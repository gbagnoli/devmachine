//! Caller-selected public-address DDNS using a secret-backed updater config.
pub mod config;

use skillet_core::{
    files::{FileError, FileMutationResource, FileReadResource, StorageResource},
    system::{
        AccountLookupResource, AccountResource, PodmanSecretResource, ServiceResource, SystemError,
    },
};
use skillet_podman::{
    MountDependency, NetworkAttachment, PodmanConfig, PodmanError, ProcessIdentity, QuadletSecret,
    SecretTarget,
};
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;

pub const CREDENTIAL: &str = "cloudflare_ddns_config";
/// Version 2.2.0 multi-platform index, evaluated against the shipped binary.
pub const IMAGE: &str = "docker.io/timothyjmiller/cloudflare-ddns@sha256:5f2471be9efd9f0c95f973645cc87f05d501020ded94d380a2120fbfdf812d3d";

#[derive(Debug, Error)]
pub enum DdnsError {
    #[error(transparent)]
    Config(#[from] config::ConfigError),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error(transparent)]
    Podman(#[from] PodmanError),
}

pub fn apply<S, F>(system: &S, files: &F, input: &str, network: &str) -> Result<(), DdnsError>
where
    S: AccountLookupResource + AccountResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    let payload = config::Payload::parse(input)?.render()?;
    files.require_btrfs_subvolume_mount(Path::new("/var/lib/data"), Path::new("/var"), "/data")?;
    system.ensure_podman_secret(CREDENTIAL, &payload)?;
    skillet_podman::container(
        system,
        files,
        PodmanConfig {
            name: "cloudflare-ddns".into(),
            image: IMAGE.into(),
            network_attachments: vec![NetworkAttachment::Bridge(network.into())],
            port_publications: Vec::new(),
            storage_dependency: Some(MountDependency::shared_service_data()),
            process_identity: ProcessIdentity::Numeric { uid: 0, gid: 0 },
            namespace_mapping: None,
            volumes: Vec::new(),
            secrets: vec![QuadletSecret {
                secret_name: CREDENTIAL.into(),
                target: SecretTarget::File {
                    target_path: "/config.json".into(),
                    mode: Some("0400".into()),
                    uid: Some(0),
                    gid: Some(0),
                },
            }],
            config_revisions: vec![payload.into_bytes()],
            extra_config: BTreeMap::from([
                (
                    "Container".into(),
                    vec![
                        "ContainerName=cloudflare-ddns".into(),
                        skillet_datadog::process("ddns", "cloudflare-ddns"),
                    ],
                ),
                (
                    "Unit".into(),
                    vec![
                        "After=network-online.target".into(),
                        "Wants=network-online.target".into(),
                    ],
                ),
                ("Service".into(), vec!["Restart=always".into()]),
                ("Install".into(), vec!["WantedBy=multi-user.target".into()]),
            ]),
        },
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
