//! Shared static service dashboard, generated from the host's Caddy UI sites.

use serde::Serialize;
use serde_yml::Value;
use skillet_core::{
    files::{FileError, FileMutationResource, FileReadResource, Ownership},
    system::{
        AccountLookupResource, AccountResource, PodmanSecretResource, ServiceResource, SystemError,
    },
};
use skillet_podman::{MountDependency, NetworkAttachment, PodmanConfig, ProcessIdentity};
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;

pub const IMAGE: &str = "docker.io/b4bz/homer:latest";
const TEMPLATE: &str = include_str!("template.yml");
const CONFIG_PATH: &str = "/etc/skillet/homer/assets/config.yml";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Link {
    pub name: String,
    pub hostname: String,
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
}

#[derive(Serialize)]
struct Group<'a> {
    name: &'static str,
    items: Vec<Item<'a>>,
}

#[derive(Serialize)]
struct Item<'a> {
    name: &'a str,
    url: String,
}

/// The checked-in template owns presentation; selected UI sites own links.
pub fn render(host: &str, links: &[Link]) -> Result<String, HomerError> {
    let mut document: Value = serde_yml::from_str(TEMPLATE)?;
    let mapping = document.as_mapping_mut().ok_or(HomerError::Template)?;
    mapping.insert(Value::String("subtitle".into()), Value::String(host.into()));
    mapping.insert(
        Value::String("services".into()),
        serde_yml::to_value([Group {
            name: "Services",
            items: links
                .iter()
                .map(|link| Item {
                    name: &link.name,
                    url: format!("https://{}", link.hostname),
                })
                .collect(),
        }])?,
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
    files.ensure_file(
        Path::new(CONFIG_PATH),
        config.as_bytes(),
        Some(0o644),
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
            volumes: Vec::new(),
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
