//! Rootful Plex, private media mounts and an explicitly advertised HTTPS origin.
use skillet_core::{
    files::{FileError, FileMutationResource, FileReadResource, Ownership, StorageResource},
    system::{AccountLookupResource, PodmanSecretResource, ServiceResource, SystemError},
};
use skillet_podman::{
    MountDependency, NetworkAttachment, PodmanConfig, PortProtocol, PortPublication,
    ProcessIdentity, Volume,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
use thiserror::Error;

pub const IMAGE: &str = "docker.io/plexinc/pms-docker:latest";
pub const PORT: u16 = 32400;
pub const APPLY_UNIT: &str = "skillet-plex-apply.service";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    pub state_path: &'static str,
    pub transcode_path: &'static str,
    pub files_path: &'static str,
    pub music_path: &'static str,
    pub data_owner: &'static str,
    pub data_group: &'static str,
}

impl Config {
    pub fn libraries(self) -> [(PathBuf, &'static str); 3] {
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
            self.transcode_path,
            self.files_path,
            self.music_path,
        ];
        for (index, path) in paths.iter().enumerate() {
            if !path.starts_with("/var/lib/data/")
                || path.contains(['\n', '\r', ':'])
                || path.split('/').any(|part| matches!(part, "." | ".."))
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
    #[error("invalid Plex {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error(transparent)]
    Podman(#[from] skillet_podman::PodmanError),
}

pub fn advertised_url(value: &str) -> Result<String, Error> {
    let url = url::Url::parse(value).map_err(|_| Error::Invalid("public HTTPS origin"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || url.port_or_known_default() != Some(443)
    {
        return Err(Error::Invalid("public HTTPS origin"));
    }
    Ok(format!("{}:443", url.origin().ascii_serialization()))
}

/// Observe only the native ownership marker; never expose the account token.
pub fn claimed(files: &(impl FileReadResource + ?Sized), config: Config) -> Result<bool, Error> {
    let path = Path::new(config.state_path)
        .join("Library/Application Support/Plex Media Server/Preferences.xml");
    let Some(bytes) = files.read_file(&path)? else {
        return Ok(false);
    };
    if bytes.len() > 1024 * 1024 {
        return Err(Error::Invalid("native preferences"));
    }
    let mut reader = quick_xml::Reader::from_reader(bytes.as_slice());
    loop {
        match reader
            .read_event()
            .map_err(|_| Error::Invalid("native preferences"))?
        {
            quick_xml::events::Event::Start(element) | quick_xml::events::Event::Empty(element)
                if element.name().as_ref() == b"Preferences" =>
            {
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|_| Error::Invalid("native preferences"))?;
                    if attribute.key.as_ref() == b"PlexOnlineToken" {
                        let value = attribute
                            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                            .map_err(|_| Error::Invalid("native preferences"))?;
                        return Ok(!value.trim().is_empty());
                    }
                }
                return Ok(false);
            }
            quick_xml::events::Event::Eof => return Err(Error::Invalid("native preferences")),
            _ => {}
        }
    }
}

pub fn apply<S, F>(
    system: &S,
    files: &F,
    config: Config,
    network: &str,
    public_url: &str,
) -> Result<(), Error>
where
    S: AccountLookupResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    config.validate()?;
    let advertise = advertised_url(public_url)?;
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
    let owner = Ownership::named(Some(config.data_owner), Some(config.data_group));
    let mut volumes = Vec::new();
    for (source, target) in [
        (config.state_path, "/config"),
        (config.transcode_path, "/transcode"),
    ] {
        files.ensure_directory(Path::new(source), Some(0o750), &owner)?;
        volumes.push(volume(source, target, "Z"));
    }
    for (source, target) in config.libraries() {
        files.ensure_directory(&source, Some(0o2775), &owner)?;
        volumes.push(volume(&source.to_string_lossy(), target, "ro,z"));
    }
    skillet_podman::container(
        system,
        files,
        PodmanConfig {
            name: "plex".into(),
            image: IMAGE.into(),
            network_attachments: vec![NetworkAttachment::Bridge(network.into())],
            // Plex's initial account claim is performed through an SSH tunnel.
            port_publications: [
                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
            ]
            .into_iter()
            .map(|host_address| PortPublication {
                host_address,
                host_port: PORT,
                container_port: PORT,
                protocol: PortProtocol::Tcp,
            })
            .collect(),
            storage_dependency: Some(MountDependency::shared_service_data()),
            process_identity: ProcessIdentity::ImageDefault,
            namespace_mapping: None,
            volumes,
            secrets: Vec::new(),
            config_revisions: Vec::new(),
            extra_config: BTreeMap::from([
                (
                    "Container".into(),
                    vec![
                        "ContainerName=plex".into(),
                        "AutoUpdate=registry".into(),
                        "Environment=TZ=Etc/UTC".into(),
                        format!("Environment=PLEX_UID={}", user.uid),
                        format!("Environment=PLEX_GID={}", group.gid),
                        "Environment=CHANGE_CONFIG_DIR_OWNERSHIP=false".into(),
                        format!("Environment=ADVERTISE_IP={advertise}"),
                        skillet_datadog::process("plex", "Plex Media Server"),
                    ],
                ),
                (
                    "Unit".into(),
                    vec![
                        "Description=Plex media server".into(),
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
