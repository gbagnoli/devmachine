//! Private file browsing over a caller-declared user-file scope.

/// Moving upstream image used by both provisioning and real-container checks.
pub const IMAGE: &str = "docker.io/zer0tonin/mikochi:latest";
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
pub enum MikochiError {
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error(transparent)]
    Podman(#[from] skillet_podman::PodmanError),
    #[error("Mikochi requires an existing data owner and its primary group")]
    Owner,
    #[error("Mikochi requires a dedicated file directory below /var/lib/data")]
    Scope,
}

pub struct Config<'a> {
    pub files_path: &'a str,
    pub owner: &'a str,
    pub group: &'a str,
    pub container_uid: u32,
    pub container_gid: u32,
    pub private_network: &'a str,
}

pub fn apply<S, F>(system: &S, files: &F, config: &Config<'_>) -> Result<(), MikochiError>
where
    S: AccountLookupResource + AccountResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let path = Path::new(config.files_path);
    if !path.starts_with("/var/lib/data")
        || path == Path::new("/var/lib/data")
        || path
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(MikochiError::Scope);
    }
    let user = system
        .user_by_name(config.owner)?
        .ok_or(MikochiError::Owner)?;
    let group = system
        .group_by_name(config.group)?
        .ok_or(MikochiError::Owner)?;
    if user.primary_gid != group.gid {
        return Err(MikochiError::Owner);
    }
    files.ensure_directory(
        path,
        Some(0o755),
        &Ownership::named(Some(config.owner), Some(config.group)),
    )?;
    skillet_podman::container(
        system,
        files,
        PodmanConfig {
            name: "mikochi".into(),
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
            volumes: vec![Volume {
                host_path: config.files_path.into(),
                container_path: "/data".into(),
                options: Some("z".into()),
                host_mode: None,
                host_ownership: None,
            }],
            secrets: Vec::new(),
            config_revisions: Vec::new(),
            extra_config: BTreeMap::from([
                (
                    "Container".into(),
                    vec![
                        "ContainerName=mikochi".into(),
                        "AutoUpdate=registry".into(),
                        "Environment=NO_AUTH=true".into(),
                        "Environment=HOST=:8080".into(),
                        "Environment=DATA_DIR=/data".into(),
                        "Environment=FRONTEND_FILES=/usr/share/mikochi/static".into(),
                        "DropCapability=ALL".into(),
                        "NoNewPrivileges=true".into(),
                        skillet_datadog::process("mikochi", "mikochi"),
                    ],
                ),
                ("Service".into(), vec!["Restart=always".into()]),
                (
                    "Unit".into(),
                    vec![
                        "Description=Mikochi private Syncthing file browser".into(),
                        "After=syncthing.service".into(),
                    ],
                ),
            ]),
        },
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
