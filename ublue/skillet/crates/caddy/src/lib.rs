//! Caddy reverse proxy for administrative interfaces.

use serde::Deserialize;
use skillet_core::files::{FileError, FileResource};
use skillet_core::system::{SystemError, SystemResource};
use skillet_podman::{
    self, ContainerUser, PodmanConfig, PodmanError, PodmanNetwork, QuadletSecret, SecretTarget,
    Volume,
};
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaddySites {
    pub pihole: String,
    pub syncthing: String,
    #[serde(default)]
    pub acme_staging: bool,
}

#[derive(Debug, Error)]
pub enum CaddyError {
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("File error: {0}")]
    File(#[from] FileError),
    #[error("Podman error: {0}")]
    Podman(#[from] PodmanError),
    #[error("invalid Caddy site hostname: {0}")]
    InvalidHostname(String),
    #[error("invalid Caddy sites credential: {0}")]
    InvalidSites(#[from] serde_json::Error),
}

impl CaddySites {
    pub fn parse(contents: &str) -> Result<Self, CaddyError> {
        let sites: Self = serde_json::from_str(contents)?;
        validate_hostname(&sites.pihole)?;
        validate_hostname(&sites.syncthing)?;
        if sites.pihole == sites.syncthing {
            return Err(CaddyError::InvalidHostname(sites.pihole));
        }
        Ok(sites)
    }

    fn render(&self) -> String {
        let acme = if self.acme_staging {
            "    acme_ca https://acme-staging-v02.api.letsencrypt.org/directory\n"
        } else {
            ""
        };
        format!(
            "{{\n    admin off\n{acme}    acme_dns cloudflare {{env.CF_API_TOKEN}}\n}}\n\n{} {{\n    @outside_tailnet not remote_ip 100.64.0.0/10 fd7a:115c:a1e0::/48\n    respond @outside_tailnet 403\n    reverse_proxy pihole:8088\n}}\n\n{} {{\n    @outside_tailnet not remote_ip 100.64.0.0/10 fd7a:115c:a1e0::/48\n    respond @outside_tailnet 403\n    reverse_proxy syncthing:8384\n}}\n",
            self.pihole, self.syncthing
        )
    }
}

fn validate_hostname(hostname: &str) -> Result<(), CaddyError> {
    let valid = hostname.len() <= 253
        && hostname.contains('.')
        && hostname.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(CaddyError::InvalidHostname(hostname.to_string()))
    }
}

pub fn apply<S, F>(
    system: &S,
    files: &F,
    sites: &CaddySites,
    network: PodmanNetwork,
) -> Result<(), CaddyError>
where
    S: SystemResource + ?Sized,
    F: FileResource + ?Sized,
{
    let caddyfile = sites.render();
    let config_dir = Path::new("/etc/skillet/caddy");
    files.ensure_directory(config_dir, Some(0o755), Some("root"), Some("root"))?;
    files.ensure_file(
        &config_dir.join("Caddyfile"),
        caddyfile.as_bytes(),
        Some(0o644),
        Some("root"),
        Some("root"),
    )?;

    let mut container = BTreeMap::new();
    container.insert(
        "Container".to_string(),
        vec![
            "AutoUpdate=registry".to_string(),
            "ContainerName=caddy".to_string(),
            "PublishPort=[::]:443:443/tcp".to_string(),
            "PublishPort=0.0.0.0:443:443/tcp".to_string(),
        ],
    );
    container.insert("Service".to_string(), vec!["Restart=always".to_string()]);
    container.insert(
        "Unit".to_string(),
        vec![
            "Description=Caddy private UI reverse proxy".to_string(),
            "After=network-online.target".to_string(),
            "Requires=skillet-data-prepare.service".to_string(),
            "After=skillet-data-prepare.service".to_string(),
            "BindsTo=var-lib-data.mount".to_string(),
            "After=var-lib-data.mount".to_string(),
            "AssertPathIsMountPoint=/var/lib/data".to_string(),
        ],
    );

    let config = PodmanConfig {
        name: "caddy".to_string(),
        image: "ghcr.io/caddybuilds/caddy-cloudflare:2".to_string(),
        networks: vec![network],
        user: ContainerUser {
            container_uid: 0,
            container_gid: 0,
            host_user: None,
        },
        create_host_user: false,
        volumes: vec![
            Volume {
                host_path: "/etc/skillet/caddy/Caddyfile".to_string(),
                container_path: "/etc/caddy/Caddyfile".to_string(),
                options: Some("ro,Z".to_string()),
            },
            Volume {
                host_path: "/var/lib/data/caddy/data".to_string(),
                container_path: "/data".to_string(),
                options: Some("Z".to_string()),
            },
            Volume {
                host_path: "/var/lib/data/caddy/config".to_string(),
                container_path: "/config".to_string(),
                options: Some("Z".to_string()),
            },
        ],
        secrets: vec![QuadletSecret {
            secret_name: "cloudflare_acme_token".to_string(),
            target: SecretTarget::Environment {
                env_var_name: "CF_API_TOKEN".to_string(),
            },
        }],
        config_revisions: vec![caddyfile.into_bytes()],
        extra_config: container,
    };
    // Preserve the caller's network configuration and let Podman create its
    // persistent directories on the shared data filesystem.
    for path in ["/var/lib/data/caddy/data", "/var/lib/data/caddy/config"] {
        files.ensure_directory(Path::new(path), Some(0o755), Some("root"), Some("root"))?;
    }
    skillet_podman::container(system, files, config)?;
    Ok(())
}
