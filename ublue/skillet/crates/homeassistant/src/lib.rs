//! Optional Home Assistant Container deployment; application state stays application-owned.
use skillet_core::{
    files::{FileError, FileMutationResource, FileReadResource, Ownership, StorageResource},
    system::{
        AccountLookupResource, AccountResource, PodmanSecretResource, ServiceResource, SystemError,
    },
};
use skillet_podman::{
    MountDependency, NetworkAttachment, PodmanConfig, PodmanError, ProcessIdentity, Volume,
};
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;

pub mod backup;
mod listener;
pub mod metrics;
pub const IMAGE: &str = "ghcr.io/home-assistant/home-assistant:2026.10.1";
pub const DATA: &str = "/var/lib/data/homeassistant";
pub const PORT: u16 = 8123;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    pub backup_subvolume: &'static str,
    pub files_root: &'static str,
    pub backup_files: &'static str,
    pub owner: &'static str,
    pub group: &'static str,
}

impl Config {
    fn validate(self) -> Result<(), Error> {
        let root = Path::new(self.backup_subvolume);
        if !root.is_absolute()
            || root
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(Error::Invalid);
        }
        for (child, parent) in [
            (self.files_root, self.backup_subvolume),
            (self.backup_files, self.files_root),
        ] {
            let relative = Path::new(child)
                .strip_prefix(parent)
                .map_err(|_| Error::Invalid)?;
            if relative.as_os_str().is_empty()
                || relative
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                return Err(Error::Invalid);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error(transparent)]
    Podman(#[from] PodmanError),
    #[error(transparent)]
    Activation(#[from] skillet_core::activation::ActivationError),
    #[error("Home Assistant runtime I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid Home Assistant backup configuration or response")]
    Invalid,
    #[error("Home Assistant API request failed (credentials and response omitted)")]
    Api,
}

pub fn apply<S, F>(system: &S, files: &F, config: Config) -> Result<(), Error>
where
    S: AccountLookupResource + AccountResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    files.require_btrfs_subvolume_mount(Path::new("/var/lib/data"), Path::new("/var"), "/data")?;
    config.validate()?;
    files.require_btrfs_subvolume(Path::new(config.backup_subvolume))?;
    files.ensure_btrfs_subvolume(Path::new(DATA))?;
    let root = Ownership::named(Some("root"), Some("root"));
    let config_path = Path::new(DATA).join("config");
    files.ensure_directory(&config_path, Some(0o700), &root)?;
    let initializer = include_bytes!("initialize.py");
    let init_directory = Path::new("/etc/skillet/homeassistant-initialize");
    files.ensure_directory(init_directory, Some(0o755), &root)?;
    files.ensure_file(
        &init_directory.join("initialize.py"),
        initializer,
        Some(0o644),
        &root,
    )?;
    listener::apply(system, files)?;
    let extra_config = BTreeMap::from([
        (
            "Container".into(),
            vec![
                "ContainerName=homeassistant".into(),
                "StopTimeout=60".into(),
                "Environment=TZ=UTC".into(),
                skillet_datadog::process("homeassistant", "homeassistant"),
            ],
        ),
        (
            "Service".into(),
            vec![
                "Restart=on-failure".into(),
                "TimeoutStartSec=900".into(),
                format!("ExecStartPre=/usr/bin/podman run --rm --network=none --entrypoint=python3 --volume={DATA}/config:/config:Z --volume=/etc/skillet/homeassistant-initialize:/skillet:ro,Z {IMAGE} /skillet/initialize.py"),
                "TimeoutStopSec=90".into(),
            ],
        ),
        (
            "Unit".into(),
            vec![
                "Description=Home Assistant Container".into(),
                "After=network-online.target skillet-homeassistant-listener.service".into(),
                "Requires=skillet-homeassistant-listener.service".into(),
                "Wants=network-online.target".into(),
            ],
        ),
        ("Install".into(), vec!["WantedBy=multi-user.target".into()]),
    ]);
    skillet_podman::container(
        system,
        files,
        PodmanConfig {
            name: "homeassistant".into(),
            image: IMAGE.into(),
            network_attachments: vec![NetworkAttachment::Host],
            port_publications: vec![],
            storage_dependency: Some(MountDependency::shared_service_data()),
            process_identity: ProcessIdentity::ImageDefault,
            namespace_mapping: None,
            volumes: vec![Volume {
                host_path: config_path.display().to_string(),
                container_path: "/config".into(),
                options: Some("Z".into()),
                host_mode: None,
                host_ownership: None,
            }],
            secrets: vec![],
            config_revisions: vec![initializer.to_vec()],
            extra_config,
        },
    )?;
    backup::install(system, files, config)?;
    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
