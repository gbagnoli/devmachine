//! Disposable integration-test fixture; it is not a host profile or apply path.

use skillet_core::{
    files::{FileError, FileResource},
    system::{SystemError, SystemResource},
};
use skillet_podman::{
    PodmanConfig, PodmanError, ProcessIdentity, QuadletSecret, SecretTarget, Volume,
};
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;

const INPUT_DIR: &str = "/var/lib/skillet-smoke/desired";
const CONFIG_PATH: &str = "/etc/skillet-smoke/config";
const SECRET_NAME: &str = "skillet-smoke-dummy";
const ENTRYPOINT: &[u8] = b"#!/bin/sh\nset -eu\ncp /fixture/config /data/observed-config\nsha256sum /run/secrets/skillet-smoke-dummy | cut -d' ' -f1 > /data/observed-secret-sha\nwhile :; do sleep 3600; done\n";

#[derive(Debug, Error)]
pub(super) enum FixtureError {
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("File error: {0}")]
    File(#[from] FileError),
    #[error("Podman error: {0}")]
    Podman(#[from] PodmanError),
    #[error("Fixture input error: {0}")]
    Input(String),
}

pub(super) fn apply(
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), FixtureError> {
    let config = files
        .read_file(&Path::new(INPUT_DIR).join("config"))?
        .ok_or_else(|| FixtureError::Input("missing desired config".to_string()))?;
    let secret = files
        .read_file(&Path::new(INPUT_DIR).join("secret"))?
        .ok_or_else(|| FixtureError::Input("missing desired dummy secret".to_string()))?;
    let secret = String::from_utf8(secret)
        .map_err(|_| FixtureError::Input("dummy secret must be UTF-8".to_string()))?;

    files.ensure_directory(
        Path::new("/etc/skillet-smoke"),
        Some(0o755),
        Some("root"),
        Some("root"),
    )?;
    files.ensure_directory(
        Path::new("/var/lib/skillet-smoke/data"),
        Some(0o755),
        Some("root"),
        Some("root"),
    )?;
    files.ensure_file(
        Path::new(CONFIG_PATH),
        &config,
        Some(0o644),
        Some("root"),
        Some("root"),
    )?;
    files.ensure_file(
        Path::new("/etc/skillet-smoke/entrypoint.sh"),
        ENTRYPOINT,
        Some(0o644),
        Some("root"),
        Some("root"),
    )?;
    system.ensure_podman_secret(SECRET_NAME, &secret)?;

    let mut extra_config = BTreeMap::new();
    extra_config.insert(
        "Container".to_string(),
        vec![
            "ContainerName=skillet-smoke-fixture".to_string(),
            "Exec=/bin/sh /fixture/entrypoint.sh".to_string(),
        ],
    );
    extra_config.insert(
        "Service".to_string(),
        vec![
            "ExecStartPre=/usr/bin/test -e /var/lib/skillet-smoke/allow-start".to_string(),
            "Restart=no".to_string(),
        ],
    );
    extra_config.insert(
        "Install".to_string(),
        vec!["WantedBy=multi-user.target".to_string()],
    );

    skillet_podman::container(
        system,
        files,
        PodmanConfig {
            name: "skillet-smoke-fixture".to_string(),
            image: "docker.io/library/alpine:3.20".to_string(),
            network_attachments: Vec::new(),
            port_publications: Vec::new(),
            process_identity: ProcessIdentity::ImageDefault,
            namespace_mapping: None,
            volumes: vec![
                Volume {
                    host_path: "/etc/skillet-smoke".to_string(),
                    container_path: "/fixture".to_string(),
                    options: Some("ro,Z".to_string()),
                },
                Volume {
                    host_path: "/var/lib/skillet-smoke/data".to_string(),
                    container_path: "/data".to_string(),
                    options: Some("Z".to_string()),
                },
            ],
            secrets: vec![QuadletSecret {
                secret_name: SECRET_NAME.to_string(),
                target: SecretTarget::File {
                    target_path: "/run/secrets/skillet-smoke-dummy".to_string(),
                    mode: Some("0400".to_string()),
                    uid: None,
                    gid: None,
                },
            }],
            config_revisions: vec![config],
            extra_config,
        },
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "test_fixture_tests.rs"]
mod tests;
