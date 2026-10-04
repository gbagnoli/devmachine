use skillet_core::files::{
    FileError, FileMutationResource, FileReadResource, OwnerIdentity, Ownership, StorageResource,
};
use skillet_core::system::{
    AccountLookupResource, AccountResource, PodmanSecretResource, ServiceResource, SystemError,
};
use skillet_podman::{
    self, MountDependency, NetworkAttachment, PodmanConfig, PodmanError, ProcessIdentity, Volume,
};
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;
use tracing::info;

const DATA_PATH: &str = "/var/lib/data/unifi";

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
    S: AccountLookupResource + AccountResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    info!("Applying UniFi Network container...");
    files.require_btrfs_subvolume_mount(Path::new("/var/lib/data"), Path::new("/var"), "/data")?;
    files.ensure_btrfs_subvolume(Path::new(DATA_PATH))?;
    let mut extra_config = BTreeMap::new();
    extra_config.insert(
        "Container".to_string(),
        vec![
            "AutoUpdate=registry".to_string(),
            "ContainerName=unifi".to_string(),
            "Environment=TZ=Europe/Madrid".to_string(),
        ],
    );
    extra_config.insert("Service".to_string(), vec!["Restart=always".to_string()]);
    extra_config.insert(
        "Unit".to_string(),
        vec![
            "Description=UniFi Network application".to_string(),
            "After=network-online.target".to_string(),
            "Wants=network-online.target".to_string(),
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
            network_attachments: vec![NetworkAttachment::Host],
            port_publications: Vec::new(),
            storage_dependency: Some(MountDependency::shared_service_data()),
            process_identity: ProcessIdentity::Named {
                user: "unifi".to_string(),
                group: None,
            },
            namespace_mapping: None,
            volumes: vec![Volume {
                host_path: DATA_PATH.to_string(),
                container_path: "/unifi".to_string(),
                options: Some("Z".to_string()),
                host_mode: Some(0o750),
                host_ownership: Some(Ownership {
                    uid: Some(OwnerIdentity::Id(999)),
                    gid: Some(OwnerIdentity::Id(999)),
                }),
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
