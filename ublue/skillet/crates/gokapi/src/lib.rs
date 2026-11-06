//! Private, per-user header-authenticated administration and public native shares.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use skillet_core::{
    files::{FileError, FileMutationResource, FileReadResource, Ownership, StorageResource},
    system::{AccountLookupResource, PodmanSecretResource, ServiceResource, SystemError},
};
use skillet_podman::{MountDependency, NetworkAttachment, PodmanConfig, ProcessIdentity, Volume};
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;
pub mod backup;
pub mod initial;
pub const IMAGE: &str = "docker.io/f0rc3/gokapi:v2.2.4";
pub const PORT: u16 = 53842;
pub const ADMIN_CREDENTIAL: &str = "gokapi_admin_user";
pub const APPLY_UNIT: &str = "skillet-gokapi-apply.service";
pub const INITIAL_UNIT: &str = "skillet-gokapi-initialize.service";
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    pub state_path: &'static str,
    pub backup_path: &'static str,
    pub backup_subvolume: &'static str,
    pub owner: &'static str,
    pub group: &'static str,
}
#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub public_url: String,
    pub admin: String,
    pub users: Vec<String>,
}
impl Settings {
    pub fn validate(&self) -> Result<(), Error> {
        let origin = url::Url::parse(&self.public_url).map_err(|_| Error::Invalid)?;
        if origin.scheme() != "https"
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.query().is_some()
            || origin.fragment().is_some()
            || origin.path() != "/"
            || origin.port_or_known_default() != Some(443)
            || (self.users.is_empty() || self.users.len() > 32)
            || !self.users.contains(&self.admin)
            || self.users.iter().any(|user| !valid_user(user))
            || !valid_user(&self.admin)
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
pub fn valid_user(value: &str) -> bool {
    (3..=254).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_graphic() && !matches!(byte, b'"' | b'\'' | b'\\' | b','))
        && value == value.to_ascii_lowercase()
}
#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid Gokapi configuration or account policy")]
    Invalid,
    #[error("Gokapi native operation failed (response suppressed)")]
    Consumer,
    #[error("Gokapi state belongs to different settings; explicit reconfiguration required")]
    Binding,
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error(transparent)]
    Podman(#[from] skillet_podman::PodmanError),
    #[error("Gokapi state I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid Gokapi state document")]
    Json(#[from] serde_json::Error),
}
impl Config {
    pub fn validate(self) -> Result<(), Error> {
        let state = Path::new(self.state_path);
        let backup = Path::new(self.backup_path);
        if !Path::new(self.backup_subvolume).starts_with("/var/lib/data")
            || Path::new(self.backup_subvolume) == Path::new("/var/lib/data")
            || !state.starts_with("/var/lib/data")
            || state == Path::new("/var/lib/data")
            || !backup.starts_with(self.backup_subvolume)
            || backup == Path::new(self.backup_subvolume)
            || state.starts_with(backup)
            || backup.starts_with(state)
            || [self.state_path, self.backup_path, self.backup_subvolume]
                .iter()
                .any(|p| {
                    p.split('/').any(|part| matches!(part, ".." | "."))
                        || p.contains(['\n', '\r', ':'])
                })
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}
pub fn apply<S, F>(
    system: &S,
    files: &F,
    config: Config,
    network: &str,
    settings: &Settings,
) -> Result<(), Error>
where
    S: AccountLookupResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    config.validate()?;
    settings.validate()?;
    let user = system.user_by_name(config.owner)?.ok_or(Error::Invalid)?;
    let group = system.group_by_name(config.group)?.ok_or(Error::Invalid)?;
    if user.uid == 0 || group.gid == 0 || network.is_empty() {
        return Err(Error::Invalid);
    }
    files.require_btrfs_subvolume_mount(Path::new("/var/lib/data"), Path::new("/var"), "/data")?;
    files.require_btrfs_subvolume(Path::new(config.backup_subvolume))?;
    let owner = Ownership::named(Some(config.owner), Some(config.group));
    let mut volumes = Vec::new();
    for leaf in ["config", "data"] {
        let path = Path::new(config.state_path).join(leaf);
        files.ensure_directory(&path, Some(0o700), &owner)?;
        volumes.push(Volume {
            host_path: path.to_string_lossy().into_owned(),
            container_path: format!("/app/{leaf}"),
            options: Some("Z".into()),
            host_mode: None,
            host_ownership: None,
        });
    }
    files.ensure_directory(Path::new(config.backup_path), Some(0o700), &owner)?;
    install_units(system, files, network)?;
    let revision = hex_digest(settings)?;
    skillet_podman::container(
        system,
        files,
        PodmanConfig {
            name: "gokapi".into(),
            image: IMAGE.into(),
            network_attachments: vec![NetworkAttachment::Bridge(network.into())],
            port_publications: vec![],
            storage_dependency: Some(MountDependency::shared_service_data()),
            process_identity: ProcessIdentity::Numeric {
                uid: user.uid,
                gid: group.gid,
            },
            namespace_mapping: None,
            volumes,
            secrets: vec![],
            config_revisions: vec![revision.into_bytes()],
            extra_config: BTreeMap::from([
                (
                    "Container".into(),
                    vec![
                        "ContainerName=gokapi".into(),
                        "Environment=TZ=Etc/UTC".into(),
                        "Environment=GOKAPI_DATA_DIR=/app/data".into(),
                        "Environment=GOKAPI_CONFIG_DIR=/app/config".into(),
                        "Environment=GOKAPI_USE_CLOUDFLARE=false".into(),
                        "Environment=GOKAPI_DISABLE_DOCKER_TRUSTED_PROXY=true".into(),
                        "EnvironmentFile=/run/skillet/gokapi/proxy.env".into(),
                        "Environment=GOKAPI_GUEST_UPLOAD_BY_DEFAULT=true".into(),
                        skillet_datadog::process("gokapi", "/app/gokapi"),
                    ],
                ),
                (
                    "Unit".into(),
                    vec![
                        format!("After={INITIAL_UNIT}"),
                        format!("Requires={INITIAL_UNIT}"),
                    ],
                ),
                (
                    "Service".into(),
                    vec!["Restart=always".into(), "TimeoutStartSec=600".into()],
                ),
                ("Install".into(), vec!["WantedBy=multi-user.target".into()]),
            ]),
        },
    )?;
    system.service_enable(backup::TIMER)?;
    system.service_start(backup::TIMER)?;
    Ok(())
}
fn hex_digest(settings: &Settings) -> Result<String, Error> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(settings)?)
    ))
}
fn install_units<S: ServiceResource + ?Sized, F: FileMutationResource + ?Sized>(
    system: &S,
    files: &F,
    network: &str,
) -> Result<(), Error> {
    let root = Ownership::named(Some("root"), Some("root"));
    let mut changed = false;
    for (name, content) in [
        (INITIAL_UNIT, include_str!("initial.service")),
        (backup::UNIT, include_str!("backup.service")),
        (backup::TIMER, include_str!("backup.timer")),
    ] {
        let content = if name == INITIAL_UNIT {
            content.replace("Before=gokapi.service",&format!("Before=gokapi.service\nAfter={network}-network.service\nRequires={network}-network.service"))
        } else {
            content.into()
        };
        changed |= files.ensure_file(
            &Path::new("/etc/systemd/system").join(name),
            content.as_bytes(),
            Some(0o644),
            &root,
        )?;
    }
    if changed {
        system.daemon_reload()?;
    }
    Ok(())
}
#[cfg(test)]
mod tests;

fn valid_proxy(cidr: &str) -> bool {
    let Some((address, prefix)) = cidr.split_once('/') else {
        return false;
    };
    let (Ok(address), Ok(prefix)) = (address.parse::<std::net::IpAddr>(), prefix.parse::<u8>())
    else {
        return false;
    };
    match address {
        std::net::IpAddr::V4(ip) => ip.is_private() && (8..=32).contains(&prefix),
        std::net::IpAddr::V6(ip) => ip.is_unique_local() && (7..=128).contains(&prefix),
    }
}
