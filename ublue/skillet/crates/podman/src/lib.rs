use askama::Template;
use sha2::{Digest, Sha256};
use skillet_core::files::{FileError, FileResource};
use skillet_core::system::{SystemError, SystemResource};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use thiserror::Error;
use tracing::info;
use users::{get_user_by_name, get_user_by_uid};

#[derive(Error, Debug)]
pub enum PodmanError {
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("File error: {0}")]
    File(#[from] FileError),
    #[error("User mapping error: {0}")]
    UserMapping(String),
}

#[derive(Template)]
#[template(path = "quadlet.container.j2")]
struct QuadletTemplate {
    sections: BTreeMap<String, Vec<String>>,
}

pub struct ContainerUser {
    pub container_uid: u32,
    pub container_gid: u32,
    pub host_user: Option<HostUser>,
}

pub enum HostUser {
    Name(String),
    Uid(u32),
}

pub struct Volume {
    pub host_path: String,
    pub container_path: String,
    pub options: Option<String>,
}

pub enum SecretTarget {
    File {
        target_path: String,
        mode: Option<String>,
        uid: Option<u32>,
        gid: Option<u32>,
    },
    Environment {
        env_var_name: String,
    },
}

pub struct QuadletSecret {
    pub secret_name: String,
    pub target: SecretTarget,
}

impl QuadletSecret {
    pub fn to_directive(&self) -> String {
        match &self.target {
            SecretTarget::File {
                target_path,
                mode,
                uid,
                gid,
            } => {
                let mut s = format!("Secret={},target={}", self.secret_name, target_path);
                if let Some(m) = mode {
                    let _ = write!(s, ",mode={m}");
                }
                if let Some(u) = uid {
                    let _ = write!(s, ",uid={u}");
                }
                if let Some(g) = gid {
                    let _ = write!(s, ",gid={g}");
                }
                s
            }
            SecretTarget::Environment { env_var_name } => {
                format!(
                    "Secret={},type=env,target={}",
                    self.secret_name, env_var_name
                )
            }
        }
    }
}

pub struct PodmanConfig {
    pub name: String,
    pub image: String,
    pub user: ContainerUser,
    pub create_host_user: bool,
    pub volumes: Vec<Volume>,
    pub secrets: Vec<QuadletSecret>,
    /// Content consumed at container startup outside the Quadlet definition.
    pub config_revisions: Vec<Vec<u8>>,
    pub extra_config: BTreeMap<String, Vec<String>>,
}

pub fn container<S, F>(system: &S, files: &F, config: PodmanConfig) -> Result<bool, PodmanError>
where
    S: SystemResource + ?Sized,
    F: FileResource + ?Sized,
{
    let name = &config.name;
    info!("Ensuring podman container: {name}");

    let mut extra_config = config.extra_config;

    // 1. Resolve and ensure host user
    let host_info = resolve_host_user(system, &config.user, config.create_host_user)?;

    // 2. Calculate mappings
    if let Some((uid_host, gid_host, username)) = &host_info {
        calculate_user_mappings(
            &config.user,
            *uid_host,
            *gid_host,
            username,
            &mut extra_config,
        );
    }

    // 3. Ensure volumes and secrets
    let container_section = extra_config.entry("Container".to_string()).or_default();
    container_section.push(format!("Image={}", config.image));

    for vol in config.volumes {
        // The application owns volume metadata. Ensure only existence here so
        // repeated applies cannot alternate ownership with its resource.
        files.ensure_directory(Path::new(&vol.host_path), None, None, None)?;

        let mut vol_line = format!("Volume={}:{}", vol.host_path, vol.container_path);
        if let Some(opt) = vol.options {
            let _ = write!(vol_line, ":{opt}");
        }
        container_section.push(vol_line);
    }

    let secret_ids = config
        .secrets
        .iter()
        .map(|secret| system.podman_secret_id(&secret.secret_name))
        .collect::<Result<Vec<_>, _>>()?;
    for secret in config.secrets {
        container_section.push(secret.to_directive());
    }

    // Sort lines in each section for deterministic output
    for lines in extra_config.values_mut() {
        lines.sort();
    }

    // 4. Render and ensure Quadlet file
    let changed = render_and_ensure_quadlet(
        system,
        files,
        name,
        extra_config,
        &secret_ids,
        &config.config_revisions,
    )?;

    Ok(changed)
}

fn resolve_host_user<S: SystemResource + ?Sized>(
    system: &S,
    user: &ContainerUser,
    create: bool,
) -> Result<Option<(u32, u32, String)>, PodmanError> {
    if let Some(hu) = &user.host_user {
        let (username, uid, gid) = match hu {
            HostUser::Name(ref n) => {
                if create {
                    system.ensure_user(n, None, None)?;
                }
                let u = get_user_by_name(n).ok_or_else(|| {
                    PodmanError::UserMapping(format!("User {n} not found on host"))
                })?;
                (n.clone(), u.uid(), u.primary_group_id())
            }
            HostUser::Uid(u) => {
                let u_info = get_user_by_uid(*u).ok_or_else(|| {
                    PodmanError::UserMapping(format!("UID {u} not found on host"))
                })?;
                (
                    u_info.name().to_string_lossy().to_string(),
                    *u,
                    u_info.primary_group_id(),
                )
            }
        };
        Ok(Some((uid, gid, username)))
    } else {
        Ok(None)
    }
}

// uid/gid subid ranges are intentionally parallel; the one-letter
// difference is the whole point
#[allow(clippy::similar_names)]
fn calculate_user_mappings(
    user: &ContainerUser,
    uid_host: u32,
    gid_host: u32,
    username: &str,
    extra_config: &mut BTreeMap<String, Vec<String>>,
) {
    let uid_container = user.container_uid;
    let gid_container = user.container_gid;

    let (sub_uid_base, sub_uid_size) =
        discover_subid_range("/etc/subuid", username).unwrap_or((100_000, 65_536));
    let (sub_gid_base, sub_gid_size) =
        discover_subid_range("/etc/subgid", username).unwrap_or((100_000, 65_536));

    let container_section = extra_config.entry("Container".to_string()).or_default();
    container_section.push(format!("User={uid_container}:{gid_container}"));

    // UIDMap
    if uid_container > 0 {
        container_section.push(format!("UIDMap=0:{sub_uid_base}:{uid_container}"));
    }
    container_section.push(format!("UIDMap={uid_container}:{uid_host}:1"));
    // Remaining subordinate UIDs after the container UID; saturating so a
    // too-small subuid range degrades to no remainder mapping instead of
    // panicking on underflow.
    let rem_u = sub_uid_size.saturating_sub(uid_container).saturating_sub(1);
    if rem_u > 0 {
        container_section.push(format!(
            "UIDMap={}:{}:{rem_u}",
            uid_container + 1,
            sub_uid_base + uid_container + 1
        ));
    }

    // GIDMap
    if gid_container > 0 {
        container_section.push(format!("GIDMap=0:{sub_gid_base}:{gid_container}"));
    }
    container_section.push(format!("GIDMap={gid_container}:{gid_host}:1"));
    // Same underflow protection as the UID remainder above.
    let rem_g = sub_gid_size.saturating_sub(gid_container).saturating_sub(1);
    if rem_g > 0 {
        container_section.push(format!(
            "GIDMap={}:{}:{rem_g}",
            gid_container + 1,
            sub_gid_base + gid_container + 1
        ));
    }
}

fn discover_subid_range(path: &str, username: &str) -> Option<(u32, u32)> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file = File::open(path).ok()?;
    let reader = BufReader::new(file);

    for line in reader.lines().map_while(Result::ok) {
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() == 3 && parts[0] == username {
            let start = parts[1].parse().ok()?;
            let size = parts[2].parse().ok()?;
            return Some((start, size));
        }
    }
    None
}

fn render_and_ensure_quadlet<S, F>(
    system: &S,
    files: &F,
    name: &str,
    sections: BTreeMap<String, Vec<String>>,
    secret_ids: &[String],
    config_revisions: &[Vec<u8>],
) -> Result<bool, PodmanError>
where
    S: SystemResource + ?Sized,
    F: FileResource + ?Sized,
{
    let template = QuadletTemplate { sections };
    let content = template.render().map_err(|e| {
        FileError::Io(std::io::Error::other(format!(
            "Template rendering failed: {e}"
        )))
    })?;

    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    for revision in config_revisions {
        hasher.update((revision.len() as u64).to_le_bytes());
        hasher.update(revision);
    }
    for id in secret_ids {
        hasher.update([0]);
        hasher.update(id.as_bytes());
    }
    let revision = hex::encode(hasher.finalize());
    let state_dir = Path::new("/var/lib/skillet/containers");
    files.ensure_directory(state_dir, Some(0o755), Some("root"), Some("root"))?;
    let applied_path = state_dir.join(format!("{name}.applied"));
    let applied = files.read_file(&applied_path)?;
    let pending = applied.as_deref() != Some(revision.as_bytes());

    let quadlet_dir = Path::new("/etc/containers/systemd");
    files.ensure_directory(quadlet_dir, Some(0o755), Some("root"), Some("root"))?;

    let quadlet_path = quadlet_dir.join(format!("{name}.container"));
    let changed = files.ensure_file(
        &quadlet_path,
        content.as_bytes(),
        Some(0o644),
        Some("root"),
        Some("root"),
    )?;

    if changed || pending {
        info!("Quadlet activation pending, triggering daemon-reload");
        system.daemon_reload()?;
        info!("Restarting {name} to consume the desired definition and secrets");
        system.service_restart(name)?;
        files.ensure_file(
            &applied_path,
            revision.as_bytes(),
            Some(0o644),
            Some("root"),
            Some("root"),
        )?;
    } else if !system.service_is_active(name)? {
        info!("Starting inactive {name}");
        system.service_start(name)?;
    }

    Ok(changed)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
