//! Shared static service dashboard, generated from the host's Caddy UI sites.

use serde::Serialize;
use serde_yml::Value;
use skillet_core::{
    files::{FileError, FileMutationResource, FileReadResource, Ownership},
    system::{
        AccountLookupResource, AccountResource, PodmanSecretResource, ServiceResource, SystemError,
    },
};
use skillet_podman::{MountDependency, NetworkAttachment, PodmanConfig, ProcessIdentity, Volume};
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub const IMAGE: &str = "docker.io/b4bz/homer:latest";
const TEMPLATE: &str = include_str!("template.yml");
const CONFIG_PATH: &str = "/etc/skillet/homer/assets/config.yml";
const CUSTOM_ASSET_PATH: &str = "/etc/skillet/homer/assets/custom";

pub const MEDIA: &str = "Media";
pub const APPLICATIONS: &str = "Applications";
pub const FILES: &str = "Files";
pub const HOME_INFRA: &str = "Home & Infra";
const GROUP_ORDER: [&str; 4] = [MEDIA, APPLICATIONS, FILES, HOME_INFRA];

#[derive(Clone, Eq, PartialEq)]
pub struct Link {
    pub group: &'static str,
    pub name: String,
    pub hostname: String,
    pub path: Option<&'static str>,
    pub endpoint: Option<String>,
    pub service_type: Option<&'static str>,
    pub icon: Option<&'static str>,
    pub logo: Option<&'static str>,
    pub api_version: Option<u8>,
    pub api_key: Option<String>,
}

#[derive(Debug, Error)]
pub enum HomerError {
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error(transparent)]
    Podman(#[from] skillet_podman::PodmanError),
    #[error("invalid Homer template or generated configuration: {0}")]
    Yaml(#[from] serde_yml::Error),
    #[error("Homer template root must be a mapping")]
    Template,
    #[error("invalid existing dashboard groups or unnamed card")]
    Input,
}

#[derive(Serialize)]
struct Group<'a, T> {
    name: &'a str,
    items: Vec<T>,
}

#[derive(Serialize)]
struct Item<'a> {
    name: &'a str,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint: Option<&'a str>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    service_type: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logo: Option<&'static str>,
    #[serde(rename = "apiVersion", skip_serializing_if = "Option::is_none")]
    api_version: Option<u8>,
    #[serde(rename = "apikey", skip_serializing_if = "Option::is_none")]
    api_key: Option<&'a str>,
}

/// The checked-in template owns presentation; selected UI sites own links.
pub fn render(host: &str, links: &[Link]) -> Result<String, HomerError> {
    let mut document: Value = serde_yml::from_str(TEMPLATE)?;
    let mapping = document.as_mapping_mut().ok_or(HomerError::Template)?;
    mapping.insert(Value::String("subtitle".into()), Value::String(host.into()));
    mapping.insert(
        Value::String("services".into()),
        serde_yml::to_value(groups(links))?,
    );
    Ok(serde_yml::to_string(&document)?)
}

fn groups(links: &[Link]) -> Vec<Group<'_, Item<'_>>> {
    collect_groups(links.iter().map(|link| {
        (
            link.group,
            Item {
                name: &link.name,
                url: format!("https://{}{}", link.hostname, link.path.unwrap_or("")),
                endpoint: link.endpoint.as_deref(),
                service_type: link.service_type,
                icon: link.icon,
                logo: link.logo,
                api_version: link.api_version,
                api_key: link.api_key.as_deref(),
            },
        )
    }))
}

fn collect_groups<'a, T>(entries: impl IntoIterator<Item = (&'a str, T)>) -> Vec<Group<'a, T>> {
    let mut grouped = BTreeMap::<&str, Vec<T>>::new();
    for (name, item) in entries {
        grouped.entry(name).or_default().push(item);
    }
    let mut result: Vec<_> = grouped
        .into_iter()
        .map(|(name, items)| Group { name, items })
        .collect();
    result.sort_by_key(|group| {
        (
            GROUP_ORDER
                .iter()
                .position(|name| name == &group.name)
                .unwrap_or(GROUP_ORDER.len()),
            group.name,
        )
    });
    result
}

/// Regroup existing cards without regenerating URLs, widgets or credentials.
/// The caller supplies the same presentation metadata used by normal apply.
pub fn regroup(
    contents: &str,
    classify: impl Fn(&str) -> &'static str,
) -> Result<String, HomerError> {
    let mut document: Value = serde_yml::from_str(contents).map_err(|_| HomerError::Input)?;
    let mut entries = Vec::new();
    for group in document["services"]
        .as_sequence()
        .ok_or(HomerError::Input)?
    {
        for item in group["items"].as_sequence().ok_or(HomerError::Input)? {
            let name = item["name"].as_str().ok_or(HomerError::Input)?;
            entries.push((classify(name), item.clone()));
        }
    }
    let mapping = document.as_mapping_mut().ok_or(HomerError::Input)?;
    mapping.insert(
        Value::String("services".into()),
        serde_yml::to_value(collect_groups(entries))?,
    );
    Ok(serde_yml::to_string(&document)?)
}

pub fn apply<S, F>(
    system: &S,
    files: &F,
    host: &str,
    private_network: &str,
    links: &[Link],
) -> Result<(), HomerError>
where
    S: AccountLookupResource + AccountResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let config = render(host, links)?;
    let root = Ownership::named(Some("root"), Some("root"));
    files.ensure_directory(Path::new("/etc/skillet/homer"), Some(0o755), &root)?;
    files.ensure_directory(Path::new("/etc/skillet/homer/assets"), Some(0o755), &root)?;
    files.ensure_directory(Path::new(CUSTOM_ASSET_PATH), Some(0o755), &root)?;
    for (name, bytes) in [
        (
            "homeassistant.svg",
            include_bytes!("../assets/homeassistant.svg").as_slice(),
        ),
        (
            "mikochi.png",
            include_bytes!("../assets/mikochi.png").as_slice(),
        ),
        (
            "pihole.svg",
            include_bytes!("../assets/pihole.svg").as_slice(),
        ),
        (
            "syncthing.svg",
            include_bytes!("../assets/syncthing.svg").as_slice(),
        ),
    ] {
        files.ensure_file(
            &Path::new(CUSTOM_ASSET_PATH).join(name),
            bytes,
            Some(0o644),
            &root,
        )?;
    }
    files.ensure_file(
        Path::new(CONFIG_PATH),
        config.as_bytes(),
        Some(
            if links
                .iter()
                .any(|link| link.api_key.as_deref().is_some_and(|key| !key.is_empty()))
            {
                0o600
            } else {
                0o644
            },
        ),
        &root,
    )?;
    skillet_podman::container(
        system,
        files,
        PodmanConfig {
            name: "homer".into(),
            image: IMAGE.into(),
            network_attachments: vec![NetworkAttachment::Bridge(private_network.into())],
            port_publications: Vec::new(),
            storage_dependency: Some(MountDependency::shared_service_data()),
            process_identity: ProcessIdentity::ImageDefault,
            namespace_mapping: None,
            volumes: vec![Volume {
                host_path: CUSTOM_ASSET_PATH.into(),
                container_path: "/www/assets/custom".into(),
                // Homer and Caddy both read this same directory. Shared SELinux
                // labeling is required because the containers have different MCS
                // labels; private `Z` labeling lets the second container break access.
                options: Some("ro,z".into()),
                host_mode: Some(0o755),
                host_ownership: Some(root.clone()),
            }],
            secrets: Vec::new(),
            config_revisions: Vec::new(),
            extra_config: BTreeMap::from([
                (
                    "Container".into(),
                    vec![
                        "ContainerName=homer".into(),
                        "AutoUpdate=registry".into(),
                        "Environment=INIT_ASSETS=0".into(),
                        "DropCapability=ALL".into(),
                        "NoNewPrivileges=true".into(),
                        skillet_datadog::process("homer", "lighttpd"),
                    ],
                ),
                ("Service".into(), vec!["Restart=always".into()]),
                (
                    "Unit".into(),
                    vec!["Description=Private Homer dashboard".into()],
                ),
            ]),
        },
    )?;
    Ok(())
}
