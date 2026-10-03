use skillet_core::files::{FileError, FileResource};
use skillet_core::system::{SystemError, SystemResource};
use skillet_podman::{self, ContainerUser, PodmanConfig, PodmanError, Volume};
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;
use tracing::info;

const DATA_PATH: &str = "/var/lib/data/unifi";
const CONTAINER_UID: u32 = 999;
const CONTAINER_GID: u32 = 999;

#[derive(Debug, Error)]
pub enum UnifiError {
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("File error: {0}")]
    File(#[from] FileError),
    #[error("Podman error: {0}")]
    Podman(#[from] PodmanError),
}

/// Configure the rootful `UniFi Network` container and its persistent state.
///
/// The rootful Podman service uses host networking so `UniFi` can discover and
/// adopt devices on the LAN. The image's `unifi` user writes as UID/GID 999;
/// match the volume root to those numeric identities without recursively
/// changing existing application data or requiring matching host accounts.
pub fn apply<S, F>(system: &S, files: &F) -> Result<(), UnifiError>
where
    S: SystemResource + ?Sized,
    F: FileResource + ?Sized,
{
    info!("Applying UniFi Network container...");
    files.require_btrfs_subvolume_mount(Path::new("/var/lib/data"), Path::new("/var"), "/data")?;
    files.ensure_btrfs_subvolume(Path::new(DATA_PATH))?;
    files.ensure_directory_with_owner_ids(
        Path::new(DATA_PATH),
        Some(0o750),
        CONTAINER_UID,
        CONTAINER_GID,
    )?;

    let mut extra_config = BTreeMap::new();
    extra_config.insert(
        "Container".to_string(),
        vec![
            "AutoUpdate=registry".to_string(),
            "ContainerName=unifi".to_string(),
            "Environment=TZ=Europe/Madrid".to_string(),
            "Network=host".to_string(),
            "User=unifi".to_string(),
        ],
    );
    extra_config.insert("Service".to_string(), vec!["Restart=always".to_string()]);
    extra_config.insert(
        "Unit".to_string(),
        vec![
            "Description=UniFi Network application".to_string(),
            "After=network-online.target".to_string(),
            "Wants=network-online.target".to_string(),
            "Requires=skillet-data-prepare.service".to_string(),
            "After=skillet-data-prepare.service".to_string(),
            "BindsTo=var-lib-data.mount".to_string(),
            "After=var-lib-data.mount".to_string(),
            "AssertPathIsMountPoint=/var/lib/data".to_string(),
        ],
    );
    extra_config.insert(
        "Install".to_string(),
        vec!["WantedBy=multi-user.target default.target".to_string()],
    );

    skillet_podman::container(
        system,
        files,
        PodmanConfig {
            name: "unifi".to_string(),
            image: "docker.io/jacobalberty/unifi:latest".to_string(),
            networks: Vec::new(),
            user: ContainerUser {
                container_uid: 0,
                container_gid: 0,
                host_user: None,
            },
            create_host_user: false,
            volumes: vec![Volume {
                host_path: DATA_PATH.to_string(),
                container_path: "/unifi".to_string(),
                options: Some("Z".to_string()),
            }],
            secrets: Vec::new(),
            config_revisions: Vec::new(),
            extra_config,
        },
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
