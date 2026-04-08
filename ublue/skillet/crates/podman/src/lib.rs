use askama::Template;
use skillet_core::files::{FileError, FileResource};
use skillet_core::system::{SystemError, SystemResource};
use std::collections::BTreeMap;
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

pub fn container<S, F>(
    system: &S,
    files: &F,
    name: &str,
    image: &str,
    user: ContainerUser,
    create_host_user: bool,
    volumes: Vec<Volume>,
    mut extra_config: BTreeMap<String, Vec<String>>,
) -> Result<bool, PodmanError>
where
    S: SystemResource + ?Sized,
    F: FileResource + ?Sized,
{
    info!("Ensuring podman container: {name}");

    // 1. Resolve and ensure host user
    let host_uid_gid = if let Some(hu) = user.host_user {
        let (username, uid) = match hu {
            HostUser::Name(ref n) => {
                if create_host_user {
                    system.ensure_user(n, None, None)?;
                }
                let u = get_user_by_name(n).ok_or_else(|| {
                    PodmanError::UserMapping(format!("User {n} not found on host"))
                })?;
                (n.clone(), u.uid())
            }
            HostUser::Uid(u) => {
                let u_info = get_user_by_uid(u).ok_or_else(|| {
                    PodmanError::UserMapping(format!("UID {u} not found on host"))
                })?;
                (u_info.name().to_string_lossy().to_string(), u)
            }
        };
        // For simplicity, assuming gid = uid for now, but should ideally resolve gid too
        let gid = uid;
        Some((uid, gid, username))
    } else {
        None
    };

    // 2. Calculate mappings
    if let Some((h_uid, h_gid, _)) = host_uid_gid {
        let c_uid = user.container_uid;
        let c_gid = user.container_gid;

        // Formula:
        // UIDMap=0:100000:C
        // UIDMap=C:H:1
        // UIDMap=C+1:100000+C+1:65536-C-1

        let sub_base = 100000;
        let sub_size = 65536;

        let container_section = extra_config.entry("Container".to_string()).or_default();

        container_section.push(format!("User={c_uid}:{c_gid}"));

        // UIDMap
        if c_uid > 0 {
            container_section.push(format!("UIDMap=0:{sub_base}:{c_uid}"));
        }
        container_section.push(format!("UIDMap={c_uid}:{h_uid}:1"));
        let remaining = sub_size - c_uid - 1;
        if remaining > 0 {
            container_section.push(format!(
                "UIDMap={}:{}:{remaining}",
                c_uid + 1,
                sub_base + c_uid + 1
            ));
        }

        // GIDMap
        if c_gid > 0 {
            container_section.push(format!("GIDMap=0:{sub_base}:{c_gid}"));
        }
        container_section.push(format!("GIDMap={c_gid}:{h_gid}:1"));
        let remaining_g = sub_size - c_gid - 1;
        if remaining_g > 0 {
            container_section.push(format!(
                "GIDMap={}:{}:{remaining_g}",
                c_gid + 1,
                sub_base + c_gid + 1
            ));
        }
    }

    // 3. Ensure volumes
    let container_section = extra_config.entry("Container".to_string()).or_default();
    container_section.push(format!("Image={image}"));

    for vol in volumes {
        let host_path = Path::new(&vol.host_path);

        let (owner, group) = if let Some((_, _, ref name)) = host_uid_gid {
            (Some(name.as_str()), Some(name.as_str()))
        } else {
            (Some("root"), Some("root"))
        };

        files.ensure_directory(host_path, Some(0o755), owner, group)?;

        let mut vol_line = format!("Volume={}:{}", vol.host_path, vol.container_path);
        if let Some(opt) = vol.options {
            vol_line.push_str(&format!(":{}", opt));
        }
        container_section.push(vol_line);
    }

    // Sort lines in each section for deterministic output
    for lines in extra_config.values_mut() {
        lines.sort();
    }

    // 4. Render Quadlet
    let template = QuadletTemplate {
        sections: extra_config,
    };
    let content = template.render().map_err(|e| {
        FileError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Template rendering failed: {e}"),
        ))
    })?;

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

    if changed {
        info!("Quadlet changed, triggering daemon-reload");
        system.service_restart("daemon-reload")?; // Assuming service_restart handles this or we add service_reload
    }

    Ok(changed)
}
