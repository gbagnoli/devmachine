//! Disposable VM fixture. Only the explicit `skillet-smoke` host selects it.

use super::ApplyError;
use skillet_core::{files::FileResource, system::SystemResource};
use skillet_podman::{ContainerUser, PodmanConfig, QuadletSecret, SecretTarget, Volume};
use std::{collections::BTreeMap, path::Path};

const INPUT_DIR: &str = "/var/lib/skillet-smoke/desired";
const CONFIG_PATH: &str = "/etc/skillet-smoke/config";
const SECRET_NAME: &str = "skillet-smoke-dummy";
const ENTRYPOINT: &[u8] = b"#!/bin/sh\nset -eu\ncp /fixture/config /data/observed-config\nsha256sum /run/secrets/skillet-smoke-dummy | cut -d' ' -f1 > /data/observed-secret-sha\nwhile :; do sleep 3600; done\n";

pub(super) fn apply(
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), ApplyError> {
    let config = files
        .read_file(&Path::new(INPUT_DIR).join("config"))?
        .ok_or_else(|| ApplyError::FixtureInput("missing desired config".to_string()))?;
    let secret = files
        .read_file(&Path::new(INPUT_DIR).join("secret"))?
        .ok_or_else(|| ApplyError::FixtureInput("missing desired dummy secret".to_string()))?;
    let secret = String::from_utf8(secret)
        .map_err(|_| ApplyError::FixtureInput("dummy secret must be UTF-8".to_string()))?;

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
            user: ContainerUser {
                container_uid: 0,
                container_gid: 0,
                host_user: None,
            },
            create_host_user: false,
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
#[path = "fixture/tests.rs"]
mod tests;
