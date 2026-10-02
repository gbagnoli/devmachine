//! Caddy reverse proxy for administrative interfaces.

use serde::{Deserialize, Serialize};
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UiService {
    pub name: String,
    pub upstream: String,
    pub port: u16,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UiEnvironment {
    pub ui_domain: String,
    pub acme_staging: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaddySites {
    pub version: u8,
    pub host: String,
    pub ui_domain: String,
    pub acme_staging: bool,
    pub machine_hostname: String,
    pub services: Vec<CaddySite>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaddySite {
    pub name: String,
    pub hostname: String,
    pub aliases: Vec<String>,
    pub upstream: String,
    pub port: u16,
}

#[derive(Debug, Error)]
pub enum CaddyError {
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("File error: {0}")]
    File(#[from] FileError),
    #[error("Podman error: {0}")]
    Podman(#[from] PodmanError),
    #[error("invalid Caddy configuration: {0}")]
    InvalidConfiguration(String),
    #[error("invalid Caddy sites credential: {0}")]
    InvalidSites(#[from] serde_json::Error),
}

impl CaddySites {
    pub fn from_host(
        host: &str,
        environment: &UiEnvironment,
        services: &[UiService],
    ) -> Result<Self, CaddyError> {
        validate_label(host, "host")?;
        validate_domain(&environment.ui_domain)?;
        if services.is_empty() {
            return Err(CaddyError::InvalidConfiguration(
                "host declares no UI services".to_string(),
            ));
        }
        let machine_hostname = format!("{host}.{}", environment.ui_domain);
        validate_domain(&machine_hostname)?;
        let mut sites = Vec::with_capacity(services.len());
        let mut all_hostnames = BTreeMap::from([(machine_hostname.clone(), "machine")]);
        for service in services {
            validate_label(&service.name, "service name")?;
            validate_label(&service.upstream, "upstream name")?;
            if service.port == 0 {
                return Err(CaddyError::InvalidConfiguration(format!(
                    "service {} has an invalid upstream port",
                    service.name
                )));
            }
            if sites
                .iter()
                .any(|site: &CaddySite| site.name == service.name)
            {
                return Err(CaddyError::InvalidConfiguration(format!(
                    "duplicate UI service {}",
                    service.name
                )));
            }
            let hostname = format!("{}.{}.{}", service.name, host, environment.ui_domain);
            validate_domain(&hostname)?;
            if all_hostnames
                .insert(hostname.clone(), "canonical UI")
                .is_some()
            {
                return Err(CaddyError::InvalidConfiguration(
                    "duplicate UI hostname".to_string(),
                ));
            }
            let mut aliases = Vec::with_capacity(service.aliases.len());
            for alias in &service.aliases {
                let expanded = expand_alias(alias, host)?;
                let alias_hostname = format!("{expanded}.{}", environment.ui_domain);
                validate_domain(&alias_hostname)?;
                if all_hostnames
                    .insert(alias_hostname.clone(), "UI alias")
                    .is_some()
                {
                    return Err(CaddyError::InvalidConfiguration(format!(
                        "duplicate UI hostname for alias on service {}",
                        service.name
                    )));
                }
                aliases.push(alias_hostname);
            }
            sites.push(CaddySite {
                name: service.name.clone(),
                hostname,
                aliases,
                upstream: service.upstream.clone(),
                port: service.port,
            });
        }
        Ok(Self {
            version: 2,
            host: host.to_string(),
            ui_domain: environment.ui_domain.clone(),
            acme_staging: environment.acme_staging,
            machine_hostname,
            services: sites,
        })
    }

    pub fn parse(contents: &str, host: &str, services: &[UiService]) -> Result<Self, CaddyError> {
        let sites: Self = serde_json::from_str(contents)?;
        if sites.version != 2 {
            return Err(CaddyError::InvalidConfiguration(format!(
                "unsupported payload version {}",
                sites.version
            )));
        }
        let expected = Self::from_host(
            host,
            &UiEnvironment {
                ui_domain: sites.ui_domain.clone(),
                acme_staging: sites.acme_staging,
            },
            services,
        )?;
        if sites != expected {
            return Err(CaddyError::InvalidConfiguration(
                "payload does not match this host's declared UI services".to_string(),
            ));
        }
        Ok(expected)
    }

    pub fn render(&self) -> String {
        let acme = if self.acme_staging {
            "    acme_ca https://acme-staging-v02.api.letsencrypt.org/directory\n"
        } else {
            ""
        };
        let mut config =
            format!("{{\n    admin off\n{acme}    acme_dns cloudflare {{env.CF_API_TOKEN}}\n}}\n");
        for service in &self.services {
            use std::fmt::Write as _;
            for hostname in std::iter::once(&service.hostname).chain(&service.aliases) {
                let _ = writeln!(
                    config,
                    "\n{} {{\n    @outside_tailnet not remote_ip 100.64.0.0/10 fd7a:115c:a1e0::/48\n    respond @outside_tailnet 403\n    reverse_proxy {}:{}\n}}",
                    hostname, service.upstream, service.port
                );
            }
        }
        config
    }
}

fn expand_alias(alias: &str, host: &str) -> Result<String, CaddyError> {
    if alias.is_empty() || alias.len() > 253 || alias.starts_with('.') || alias.ends_with('.') {
        return Err(CaddyError::InvalidConfiguration(
            "invalid relative UI alias".to_string(),
        ));
    }
    let mut labels = Vec::new();
    for label in alias.split('.') {
        if label == "{host}" {
            labels.push(host);
        } else {
            if label.contains('{') || label.contains('}') {
                return Err(CaddyError::InvalidConfiguration(
                    "the {host} alias placeholder must be a complete DNS label".to_string(),
                ));
            }
            validate_label(label, "UI alias")?;
            labels.push(label);
        }
    }
    Ok(labels.join("."))
}

fn validate_label(label: &str, field: &str) -> Result<(), CaddyError> {
    let valid = !label.is_empty()
        && label.len() <= 63
        && label.as_bytes()[0].is_ascii_alphanumeric()
        && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
        && label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(CaddyError::InvalidConfiguration(format!(
            "invalid {field} DNS label"
        )))
    }
}

pub fn validate_domain(domain: &str) -> Result<(), CaddyError> {
    if domain.len() > 253 || domain.split('.').count() < 2 {
        return Err(CaddyError::InvalidConfiguration(
            "UI domain must be a fully qualified DNS domain".to_string(),
        ));
    }
    for label in domain.split('.') {
        validate_label(label, "UI domain")?;
    }
    Ok(())
}

/// Resolve an optional relative UI namespace beneath the authoritative zone.
pub fn resolve_ui_domain(zone: &str, prefix: Option<&str>) -> Result<String, CaddyError> {
    validate_domain(zone)?;
    let prefix = prefix.unwrap_or("ui").trim();
    for label in prefix.split('.') {
        validate_label(label, "relative UI domain prefix")?;
    }
    let domain = format!("{prefix}.{zone}");
    validate_domain(&domain)?;
    Ok(domain)
}

pub fn validate_domain_in_zone(domain: &str, zone: &str) -> Result<(), CaddyError> {
    validate_domain(domain)?;
    validate_domain(zone)?;
    if domain != zone && !domain.ends_with(&format!(".{zone}")) {
        return Err(CaddyError::InvalidConfiguration(
            "UI domain is outside the selected Cloudflare zone".to_string(),
        ));
    }
    Ok(())
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
