use skillet_core::files::{FileError, FileMutationResource, FileReadResource, StorageResource};
use skillet_core::system::{
    AccountLookupResource, AccountResource, PodmanSecretResource, ServiceResource, SystemError,
};
use skillet_podman::{
    self, NetworkAttachment, PodmanConfig, PodmanError, PortProtocol, PortPublication,
    ProcessIdentity, Volume,
};
use std::collections::BTreeMap;
use std::path::Path;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum SyncthingError {
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("File error: {0}")]
    File(#[from] FileError),
    #[error("Podman error: {0}")]
    Podman(#[from] PodmanError),
}

pub struct SyncthingConfig {
    pub data_path: String,
    pub data_owner: String,
    pub data_group: String,
    pub uid: u32,
    pub gid: u32,
    pub network_name: String,
}

pub fn apply<S, F>(system: &S, files: &F, config: SyncthingConfig) -> Result<(), SyncthingError>
where
    S: AccountLookupResource + AccountResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    info!("Applying Syncthing configuration...");
    let data_path = Path::new(&config.data_path);
    files.require_btrfs_subvolume_mount(Path::new("/var/lib/data"), Path::new("/var"), "/data")?;
    files.ensure_btrfs_subvolume(data_path)?;
    files.ensure_directory(
        data_path,
        Some(0o755),
        Some(&config.data_owner),
        Some(&config.data_group),
    )?;

    let volumes = vec![Volume {
        host_path: config.data_path,
        container_path: "/var/syncthing".to_string(),
        options: Some("z".to_string()),
    }];

    let mut extra_config = BTreeMap::new();
    extra_config.insert("Service".to_string(), vec!["Restart=always".to_string()]);
    extra_config.insert(
        "Container".to_string(),
        vec![
            "AutoUpdate=registry".to_string(),
            "ContainerName=syncthing".to_string(),
            format!("Environment=PGID={}", config.gid),
            format!("Environment=PUID={}", config.uid),
        ],
    );
    extra_config.insert(
        "Unit".to_string(),
        vec![
            "Description=Syncthing file synchronization".to_string(),
            "After=network-online.target".to_string(),
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
            name: "syncthing".to_string(),
            image: "docker.io/syncthing/syncthing:latest".to_string(),
            network_attachments: vec![NetworkAttachment::Bridge(config.network_name)],
            port_publications: vec![
                PortPublication {
                    host_address: std::net::Ipv6Addr::UNSPECIFIED.into(),
                    host_port: 22000,
                    container_port: 22000,
                    protocol: PortProtocol::Tcp,
                },
                PortPublication {
                    host_address: std::net::Ipv4Addr::UNSPECIFIED.into(),
                    host_port: 22000,
                    container_port: 22000,
                    protocol: PortProtocol::Tcp,
                },
                PortPublication {
                    host_address: std::net::Ipv6Addr::UNSPECIFIED.into(),
                    host_port: 22000,
                    container_port: 22000,
                    protocol: PortProtocol::Udp,
                },
                PortPublication {
                    host_address: std::net::Ipv4Addr::UNSPECIFIED.into(),
                    host_port: 22000,
                    container_port: 22000,
                    protocol: PortProtocol::Udp,
                },
            ],
            process_identity: ProcessIdentity::ImageDefault,
            namespace_mapping: None,
            volumes,
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
