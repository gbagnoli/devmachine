//! Rootful Jellyfin with caller-owned, read-only media libraries.
use skillet_core::{
    files::{FileError, FileMutationResource, FileReadResource, Ownership, StorageResource},
    system::{AccountLookupResource, PodmanSecretResource, ServiceResource, SystemError},
};
use skillet_podman::{MountDependency, NetworkAttachment, PodmanConfig, ProcessIdentity, Volume};
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;

pub const IMAGE: &str = "docker.io/jellyfin/jellyfin:latest";
pub const PORT: u16 = 8096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    pub state_path: &'static str,
    pub cache_path: &'static str,
    pub files_path: &'static str,
    pub music_path: &'static str,
    pub data_owner: &'static str,
    pub data_group: &'static str,
}

impl Config {
    /// Consumer paths are stable while the caller selects host storage.
    pub fn libraries(self) -> [(std::path::PathBuf, &'static str); 3] {
        [
            (Path::new(self.files_path).join("library/tv"), "/media/tv"),
            (
                Path::new(self.files_path).join("library/movies"),
                "/media/movies",
            ),
            (self.music_path.into(), "/media/music"),
        ]
    }

    fn validate(self) -> Result<(), Error> {
        let paths = [
            self.state_path,
            self.cache_path,
            self.files_path,
            self.music_path,
        ];
        for (index, path) in paths.iter().enumerate() {
            if !path.starts_with("/var/lib/data/")
                || path.split('/').any(|part| part == ".." || part == ".")
                || path.contains(['\n', '\r', ':'])
                || paths.iter().enumerate().any(|(other, candidate)| {
                    other != index
                        && (Path::new(path).starts_with(candidate)
                            || Path::new(candidate).starts_with(path))
                })
            {
                return Err(Error::Invalid("overlapping or invalid storage paths"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid Jellyfin {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error(transparent)]
    Podman(#[from] skillet_podman::PodmanError),
}

/// Preserve application-managed configuration and media contents on repeat apply.
pub fn apply<S, F>(system: &S, files: &F, config: Config, network: &str) -> Result<(), Error>
where
    S: AccountLookupResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    config.validate()?;
    let user = system
        .user_by_name(config.data_owner)?
        .ok_or(Error::Invalid("data owner"))?;
    let group = system
        .group_by_name(config.data_group)?
        .ok_or(Error::Invalid("data group"))?;
    if user.uid == 0 || group.gid == 0 || network.is_empty() {
        return Err(Error::Invalid("data identity or network"));
    }
    files.require_btrfs_subvolume_mount(Path::new("/var/lib/data"), Path::new("/var"), "/data")?;
    let ownership = Ownership {
        uid: Some(skillet_core::files::OwnerIdentity::Id(user.uid)),
        gid: Some(skillet_core::files::OwnerIdentity::Id(group.gid)),
    };
    let mut volumes = Vec::new();
    for (source, target) in [
        (config.state_path, "/config"),
        (config.cache_path, "/cache"),
    ] {
        files.ensure_directory(Path::new(source), Some(0o750), &ownership)?;
        volumes.push(volume(source, target, "Z"));
    }
    for (source, target) in config.libraries() {
        files.ensure_directory(&source, Some(0o2775), &ownership)?;
        volumes.push(volume(&source.to_string_lossy(), target, "ro,z"));
    }
    skillet_podman::container(
        system,
        files,
        PodmanConfig {
            name: "jellyfin".into(),
            image: IMAGE.into(),
            network_attachments: vec![NetworkAttachment::Bridge(network.into())],
            port_publications: Vec::new(),
            storage_dependency: Some(MountDependency::shared_service_data()),
            process_identity: ProcessIdentity::Numeric {
                uid: user.uid,
                gid: group.gid,
            },
            namespace_mapping: None,
            volumes,
            secrets: Vec::new(),
            config_revisions: Vec::new(),
            extra_config: BTreeMap::from([
                (
                    "Container".into(),
                    vec![
                        "ContainerName=jellyfin".into(),
                        "AutoUpdate=registry".into(),
                        skillet_datadog::process("jellyfin", "jellyfin"),
                    ],
                ),
                (
                    "Unit".into(),
                    vec![
                        "Description=Jellyfin media server".into(),
                        "After=network-online.target".into(),
                    ],
                ),
                (
                    "Service".into(),
                    vec!["Restart=always".into(), "TimeoutStartSec=900".into()],
                ),
                ("Install".into(), vec!["WantedBy=multi-user.target".into()]),
            ]),
        },
    )?;
    Ok(())
}

fn volume(source: &str, target: &str, options: &str) -> Volume {
    Volume {
        host_path: source.into(),
        container_path: target.into(),
        options: Some(options.into()),
        host_mode: None,
        host_ownership: None,
    }
}

#[cfg(test)]
mod tests;
