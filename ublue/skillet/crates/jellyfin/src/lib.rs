//! Rootful Jellyfin with caller-owned, read-only media libraries.
use skillet_core::{
    files::{FileError, FileMutationResource, FileReadResource, Ownership, StorageResource},
    system::{AccountLookupResource, PodmanSecretResource, ServiceResource, SystemError},
};
use skillet_podman::{
    MountDependency, NetworkAttachment, PodmanConfig, PortProtocol, PortPublication,
    ProcessIdentity, Volume,
};
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

/// Public ingress remains closed until the native wizard has created an account.
pub fn setup_completed(
    files: &(impl FileReadResource + ?Sized),
    config: Config,
) -> Result<bool, Error> {
    let path = Path::new(config.state_path).join("config/system.xml");
    let Some(bytes) = files.read_file(&path)? else {
        return Ok(false);
    };
    if bytes.len() > 1024 * 1024 {
        return Err(Error::Invalid("native configuration size"));
    }
    let mut reader = quick_xml::Reader::from_reader(bytes.as_slice());
    let mut completed = None;
    let mut root = false;
    let mut closed = false;
    loop {
        match reader
            .read_event()
            .map_err(|_| Error::Invalid("native configuration XML"))?
        {
            quick_xml::events::Event::Start(element)
                if element.name().as_ref() == b"ServerConfiguration" =>
            {
                if root {
                    return Err(Error::Invalid("duplicate native configuration root"));
                }
                root = true;
            }
            quick_xml::events::Event::Empty(element)
                if element.name().as_ref() == b"ServerConfiguration" =>
            {
                if root {
                    return Err(Error::Invalid("duplicate native configuration root"));
                }
                root = true;
                closed = true;
            }
            quick_xml::events::Event::End(element)
                if element.name().as_ref() == b"ServerConfiguration" =>
            {
                closed = true;
            }
            quick_xml::events::Event::Start(element)
                if element.name().as_ref() == b"IsStartupWizardCompleted" && root =>
            {
                if completed.is_some() {
                    return Err(Error::Invalid("duplicate wizard state"));
                }
                let value = reader
                    .read_text(element.name())
                    .map_err(|_| Error::Invalid("native wizard state"))?;
                let value = value
                    .decode()
                    .map_err(|_| Error::Invalid("native wizard encoding"))?;
                completed = Some(match value.trim() {
                    "true" => true,
                    "false" => false,
                    _ => return Err(Error::Invalid("native wizard state")),
                });
            }
            quick_xml::events::Event::Eof => {
                return if root && closed {
                    Ok(completed.unwrap_or(false))
                } else {
                    Err(Error::Invalid("native configuration root"))
                }
            }
            _ => {}
        }
    }
}

/// Preserve application-managed configuration and media contents on repeat apply.
pub fn apply<S, F>(system: &S, files: &F, config: Config, network: &str) -> Result<(), Error>
where
    S: AccountLookupResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    apply_with_proxies(system, files, config, network, &[])
}

/// Caller-selected container proxy networks are trusted on first initialization;
/// later application-managed network settings are preserved verbatim.
pub fn apply_with_proxies<S, F>(
    system: &S,
    files: &F,
    config: Config,
    network: &str,
    proxies: &[&str],
) -> Result<(), Error>
where
    S: AccountLookupResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    config.validate()?;
    if network.is_empty()
        || !network
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(Error::Invalid("network name"));
    }
    for proxy in proxies {
        let (address, prefix) = proxy
            .split_once('/')
            .ok_or(Error::Invalid("proxy subnet"))?;
        let address = address
            .parse::<std::net::IpAddr>()
            .map_err(|_| Error::Invalid("proxy subnet"))?;
        let prefix = prefix
            .parse::<u8>()
            .map_err(|_| Error::Invalid("proxy subnet"))?;
        let max = if address.is_ipv4() { 32 } else { 128 };
        if prefix == 0 || prefix > max || address.is_unspecified() || address.is_multicast() {
            return Err(Error::Invalid("proxy subnet"));
        }
    }
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
    initialize_network(files, config, network, proxies, &ownership)?;
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
            // Native account setup is available only through an owned SSH tunnel.
            port_publications: vec![PortPublication {
                host_address: std::net::Ipv4Addr::LOCALHOST.into(),
                host_port: 18096,
                container_port: PORT,
                protocol: PortProtocol::Tcp,
            }],
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

fn initialize_network<F: FileMutationResource + ?Sized>(
    files: &F,
    config: Config,
    network: &str,
    proxies: &[&str],
    ownership: &Ownership,
) -> Result<(), Error> {
    let application_config = Path::new(config.state_path).join("config");
    files.ensure_directory(&application_config, Some(0o750), ownership)?;
    let mut known_proxies = String::new();
    for proxy in proxies {
        known_proxies.push_str("<string>");
        known_proxies.push_str(proxy);
        known_proxies.push_str("</string>");
    }
    let network_config = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<NetworkConfiguration><EnableIPv4>true</EnableIPv4><EnableIPv6>true</EnableIPv6><EnableHttps>false</EnableHttps><RequireHttps>false</RequireHttps><EnableUPnP>false</EnableUPnP><EnableRemoteAccess>true</EnableRemoteAccess><KnownProxies>{known_proxies}</KnownProxies></NetworkConfiguration>\n");
    files.create_file_if_absent(
        &application_config.join("network.xml"),
        network_config.as_bytes(),
        Some(0o640),
        ownership,
    )?;
    let server_config = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ServerConfiguration><ServerName>{network}</ServerName></ServerConfiguration>\n");
    files.create_file_if_absent(
        &application_config.join("system.xml"),
        server_config.as_bytes(),
        Some(0o640),
        ownership,
    )?;
    Ok(())
}
