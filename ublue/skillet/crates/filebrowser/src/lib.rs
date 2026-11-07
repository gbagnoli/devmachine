//! Private file browsing over a caller-declared user-file scope.

/// Stable upstream consumer used by provisioning and real-container checks.
pub const IMAGE: &str = "ghcr.io/gtsteffaniak/filebrowser:2.0.2-stable";
pub const DATA: &str = "/var/lib/data/filebrowser";
pub const CONFIG: &str = include_str!("config.yaml");
mod migration;
use skillet_core::{
    files::{FileError, FileMutationResource, FileReadResource, Ownership},
    system::{
        AccountLookupResource, AccountResource, PodmanSecretResource, ServiceResource, SystemError,
    },
};
use skillet_podman::{
    HostUser, MountDependency, NetworkAttachment, PodmanConfig, ProcessIdentity,
    UserNamespaceMapping, Volume,
};
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FileBrowserError {
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error(transparent)]
    Podman(#[from] skillet_podman::PodmanError),
    #[error("FileBrowser requires an existing data owner and its primary group")]
    Owner,
    #[error("FileBrowser requires a dedicated file directory below /var/lib/data")]
    Scope,
    #[error(
        "legacy Mikochi definition differs from its managed receipt; inspect it before replacement"
    )]
    Legacy,
}

pub struct Config<'a> {
    pub files_path: &'a str,
    pub owner: &'a str,
    pub group: &'a str,
    pub container_uid: u32,
    pub container_gid: u32,
    pub private_network: &'a str,
}

pub fn apply<S, F>(system: &S, files: &F, config: &Config<'_>) -> Result<(), FileBrowserError>
where
    S: AccountLookupResource + AccountResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let path = Path::new(config.files_path);
    if path.starts_with(DATA)
        || Path::new(DATA).starts_with(path)
        || !path.starts_with("/var/lib/data")
        || path == Path::new("/var/lib/data")
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(FileBrowserError::Scope);
    }
    let user = system
        .user_by_name(config.owner)?
        .ok_or(FileBrowserError::Owner)?;
    let group = system
        .group_by_name(config.group)?
        .ok_or(FileBrowserError::Owner)?;
    if user.primary_gid != group.gid {
        return Err(FileBrowserError::Owner);
    }
    migration::retire(system, files)?;
    let root = Ownership::named(Some("root"), Some("root"));
    let owner = Ownership::named(Some(config.owner), Some(config.group));
    files.ensure_directory(Path::new(DATA), Some(0o700), &owner)?;
    files.ensure_directory(Path::new("/etc/skillet/filebrowser"), Some(0o755), &root)?;
    files.ensure_file(
        Path::new("/etc/skillet/filebrowser/config.yaml"),
        CONFIG.as_bytes(),
        Some(0o644),
        &root,
    )?;
    files.ensure_directory(
        path,
        Some(0o755),
        &Ownership::named(Some(config.owner), Some(config.group)),
    )?;
    skillet_podman::container(system, files, definition(config))?;
    Ok(())
}

fn definition(config: &Config<'_>) -> PodmanConfig {
    PodmanConfig {
        name: "filebrowser".into(),
        image: IMAGE.into(),
        network_attachments: vec![NetworkAttachment::Bridge(config.private_network.into())],
        port_publications: Vec::new(),
        storage_dependency: Some(MountDependency::shared_service_data()),
        process_identity: ProcessIdentity::Numeric {
            uid: config.container_uid,
            gid: config.container_gid,
        },
        namespace_mapping: Some(UserNamespaceMapping {
            host_user: HostUser::Name(config.owner.into()),
        }),
        volumes: vec![
            Volume {
                host_path: config.files_path.into(),
                container_path: "/files".into(),
                options: Some("z".into()),
                host_mode: None,
                host_ownership: None,
            },
            Volume {
                host_path: DATA.into(),
                container_path: "/home/filebrowser/data".into(),
                options: Some("Z".into()),
                host_mode: None,
                host_ownership: None,
            },
            Volume {
                host_path: "/etc/skillet/filebrowser".into(),
                container_path: "/etc/filebrowser".into(),
                options: Some("ro,z".into()),
                host_mode: None,
                host_ownership: None,
            },
        ],
        secrets: Vec::new(),
        config_revisions: vec![CONFIG.as_bytes().to_vec()],
        extra_config: BTreeMap::from([
            (
                "Container".into(),
                vec![
                    "ContainerName=filebrowser".into(),
                    "Environment=FILEBROWSER_CONFIG=/etc/filebrowser/config.yaml".into(),
                    "HealthCmd=curl --fail --silent http://127.0.0.1:8080/health".into(),
                    "DropCapability=ALL".into(),
                    "NoNewPrivileges=true".into(),
                    skillet_datadog::process("filebrowser", "filebrowser"),
                ],
            ),
            ("Service".into(), vec!["Restart=always".into()]),
            (
                "Unit".into(),
                vec![
                    "Description=File Browser Quantum private Syncthing file browser".into(),
                    "After=syncthing.service".into(),
                ],
            ),
        ]),
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
