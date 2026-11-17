//! Provider-free, paused `SABnzbd` with private browser access and keyed APIs.
use skillet_core::{
    files::{FileError, FileMutationResource, FileReadResource, Ownership, StorageResource},
    system::{AccountLookupResource, PodmanSecretResource, ServiceResource, SystemError},
};
use skillet_podman::{MountDependency, NetworkAttachment, PodmanConfig, ProcessIdentity, Volume};
use std::fmt::Write as _;
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;
pub const IMAGE: &str = "lscr.io/linuxserver/sabnzbd:latest";
pub const CREDENTIAL: &str = "sabnzbd_api_key";
pub const PORT: u16 = 8080;

pub struct Config<'a> {
    pub state: &'a Path,
    pub downloads: &'a Path,
    pub owner: &'a str,
    pub group: &'a str,
    pub network: &'a str,
    pub ui_network: &'a str,
    pub hostnames: &'a [&'a str],
}
#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid SABnzbd {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error(transparent)]
    Podman(#[from] skillet_podman::PodmanError),
}

/// Seed once, retaining later application settings and refusing key drift.
pub fn initial_configuration(key: &str, hostnames: &[&str]) -> Result<String, Error> {
    if key.len() != 32 || !key.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Invalid("API key"));
    }
    if hostnames.is_empty()
        || hostnames.iter().any(|name| {
            name.is_empty()
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".-".contains(&b))
        })
    {
        return Err(Error::Invalid("UI hostnames"));
    }
    let mut ini = format!(
        include_str!("initial.ini"),
        key = key,
        hostnames = hostnames.join(", ")
    );
    for category in ["sonarr", "radarr", "lidarr"] {
        let _ = write!(ini, "[[{category}]]\nname = {category}\ndir = {category}\npriority = 0\npp = 3\nscript = None\n");
    }
    Ok(ini)
}

fn retained_key(bytes: &[u8]) -> Result<&str, Error> {
    if bytes.len() > 1024 * 1024 {
        return Err(Error::Invalid("native config size"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Error::Invalid("native config encoding"))?;
    let mut misc = false;
    let mut found = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            misc = line == "[misc]";
        }
        if misc {
            if let Some((name, value)) = line.split_once('=') {
                if name.trim() == "api_key" {
                    if found.is_some() {
                        return Err(Error::Invalid("duplicate native API key"));
                    }
                    found = Some(value.trim().trim_matches(['\'', '"']));
                }
            }
        }
    }
    found.ok_or(Error::Invalid(
        "missing retained API key; restore original credential",
    ))
}

pub fn apply<S, F>(system: &S, files: &F, config: &Config<'_>, key: &str) -> Result<(), Error>
where
    S: AccountLookupResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    let initial = initial_configuration(key, config.hostnames)?;
    files.require_btrfs_subvolume_mount(Path::new("/var/lib/data"), Path::new("/var"), "/data")?;
    if config.state.starts_with(config.downloads) || config.downloads.starts_with(config.state) {
        return Err(Error::Invalid("overlapping storage paths"));
    }
    for path in [config.state, config.downloads] {
        if path == Path::new("/var/lib/data")
            || !path.starts_with("/var/lib/data")
            || path
                .components()
                .any(|p| matches!(p, std::path::Component::ParentDir))
        {
            return Err(Error::Invalid("storage path"));
        }
    }
    let uid = system
        .user_by_name(config.owner)?
        .ok_or(Error::Invalid("data owner"))?
        .uid;
    let gid = system
        .group_by_name(config.group)?
        .ok_or(Error::Invalid("data group"))?
        .gid;
    if uid == 0 || gid == 0 || config.ui_network.is_empty() || config.network == config.ui_network {
        return Err(Error::Invalid("identity or private network"));
    }
    let ownership = Ownership::named(Some(config.owner), Some(config.group));
    files.ensure_directory(config.state, Some(0o700), &ownership)?;
    for suffix in [
        "",
        "incomplete",
        "complete",
        "complete/sonarr",
        "complete/radarr",
        "complete/lidarr",
    ] {
        files.ensure_directory(&config.downloads.join(suffix), Some(0o2770), &ownership)?;
    }
    let path = config.state.join("sabnzbd.ini");
    files.create_file_if_absent(&path, initial.as_bytes(), Some(0o600), &ownership)?;
    let current = files
        .read_file(&path)?
        .ok_or(Error::Invalid("native config publication"))?;
    if retained_key(&current)? != key {
        return Err(Error::Invalid(
            "retained API key differs; restore original vault entry",
        ));
    }
    skillet_podman::container(system, files, container_config(config, uid, gid))?;
    Ok(())
}

fn container_config(config: &Config<'_>, uid: u32, gid: u32) -> PodmanConfig {
    PodmanConfig {
        name: "sabnzbd".into(),
        image: IMAGE.into(),
        network_attachments: vec![
            NetworkAttachment::Bridge(config.network.into()),
            NetworkAttachment::Bridge(config.ui_network.into()),
        ],
        port_publications: Vec::new(),
        storage_dependency: Some(MountDependency::shared_service_data()),
        process_identity: ProcessIdentity::ImageDefault,
        namespace_mapping: None,
        volumes: vec![
            Volume {
                host_path: config.state.to_string_lossy().into_owned(),
                container_path: "/config".into(),
                options: Some("Z".into()),
                host_mode: None,
                host_ownership: None,
            },
            Volume {
                host_path: config.downloads.to_string_lossy().into_owned(),
                container_path: "/data/downloads/usenet".into(),
                options: Some("z".into()),
                host_mode: None,
                host_ownership: None,
            },
        ],
        secrets: Vec::new(),
        config_revisions: Vec::new(),
        extra_config: BTreeMap::from([
            (
                "Container".into(),
                vec![
                    "ContainerName=sabnzbd".into(),
                    skillet_datadog::process("sabnzbd", "SABnzbd.py"),
                    "AutoUpdate=registry".into(),
                    format!("Environment=PUID={uid}"),
                    format!("Environment=PGID={gid}"),
                    "Environment=UMASK=007".into(),
                    "Environment=TZ=Etc/UTC".into(),
                ],
            ),
            (
                "Service".into(),
                vec!["Restart=always".into(), "TimeoutStartSec=900".into()],
            ),
            ("Install".into(), vec!["WantedBy=multi-user.target".into()]),
        ]),
    }
}

#[cfg(test)]
mod tests;
