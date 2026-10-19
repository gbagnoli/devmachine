//! Caddy reverse proxy for administrative interfaces.

use serde::{Deserialize, Serialize};
use skillet_core::files::{FileError, FileMutationResource, FileReadResource, Ownership};
use skillet_core::system::{
    AccountLookupResource, AccountResource, PodmanSecretResource, ServiceResource, SystemError,
};
use skillet_podman::{
    self, MountDependency, NetworkAttachment, PodmanConfig, PodmanError, PortProtocol,
    PortPublication, ProcessIdentity, QuadletSecret, SecretTarget, Volume,
};
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;

pub mod auth;
pub mod host_proxy;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpstreamTransport {
    #[default]
    Bridge,
    HostLoopback,
}

#[cfg(test)]
mod refresh_tests;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UiService {
    pub name: String,
    pub upstream: String,
    #[serde(default)]
    pub transport: UpstreamTransport,
    pub port: u16,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub tailnet_identity: bool,
    /// Compatibility for a no-auth frontend expecting a successful token refresh.
    /// This response is only available after the identity gate authorizes it.
    #[serde(default)]
    pub anonymous_token_refresh: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UiEnvironment {
    pub ui_domain: String,
    pub acme_staging: bool,
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaddySites {
    pub version: u8,
    pub host: String,
    pub ui_domain: String,
    #[serde(default)]
    pub zone_domain: Option<String>,
    pub acme_staging: bool,
    pub machine_hostname: String,
    pub services: Vec<CaddySite>,
    #[serde(default)]
    pub tailnet_users: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homeassistant_api_token: Option<String>,
}

impl std::fmt::Debug for CaddySites {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CaddySites")
            .field("version", &self.version)
            .field("host", &self.host)
            .field("ui_domain", &self.ui_domain)
            .field("zone_domain", &self.zone_domain)
            .field("acme_staging", &self.acme_staging)
            .field("machine_hostname", &self.machine_hostname)
            .field("services", &self.services)
            .field("tailnet_users", &self.tailnet_users)
            .field(
                "homeassistant_api_token",
                &self.homeassistant_api_token.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaddySite {
    pub name: String,
    pub hostname: String,
    pub aliases: Vec<String>,
    pub upstream: String,
    #[serde(default)]
    pub transport: UpstreamTransport,
    pub port: u16,
    #[serde(default)]
    pub tailnet_identity: bool,
    /// Compatibility for a no-auth frontend expecting a successful token refresh.
    /// This response is only available after the identity gate authorizes it.
    #[serde(default)]
    pub anonymous_token_refresh: bool,
}

#[derive(Debug, Error)]
pub enum CaddyError {
    #[error("Caddy host proxy I/O: {0}")]
    Io(#[from] std::io::Error),
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
    #[error(transparent)]
    Auth(#[from] auth::AuthError),
    #[error(transparent)]
    Activation(#[from] skillet_core::activation::ActivationError),
}

impl CaddySites {
    pub fn from_host(
        host: &str,
        environment: &UiEnvironment,
        services: &[UiService],
    ) -> Result<Self, CaddyError> {
        Self::from_host_with_zone(host, environment, services, None)
    }

    /// Zone-level aliases are resolved only when the workstation supplies the
    /// authoritative Cloudflare zone; the guest verifies the resulting payload.
    pub fn from_host_with_zone(
        host: &str,
        environment: &UiEnvironment,
        services: &[UiService],
        zone_domain: Option<&str>,
    ) -> Result<Self, CaddyError> {
        validate_label(host, "host")?;
        validate_domain(&environment.ui_domain)?;
        if let Some(zone) = zone_domain {
            validate_domain_in_zone(&environment.ui_domain, zone)?;
        }
        if services.is_empty() {
            return Err(CaddyError::InvalidConfiguration(
                "host declares no UI services".to_string(),
            ));
        }
        let machine_hostname = format!("{host}.{}", environment.ui_domain);
        validate_domain(&machine_hostname)?;
        let sites = build_sites(
            host,
            &environment.ui_domain,
            zone_domain,
            &machine_hostname,
            services,
        )?;
        Ok(Self {
            version: if zone_domain.is_some() {
                5
            } else if services
                .iter()
                .any(|s| s.transport == UpstreamTransport::HostLoopback)
            {
                4
            } else {
                3
            },
            host: host.to_string(),
            ui_domain: environment.ui_domain.clone(),
            zone_domain: zone_domain.map(str::to_string),
            acme_staging: environment.acme_staging,
            machine_hostname,
            services: sites,
            tailnet_users: Vec::new(),
            homeassistant_api_token: None,
        })
    }

    pub fn parse(contents: &str, host: &str, services: &[UiService]) -> Result<Self, CaddyError> {
        let sites: Self = serde_json::from_str(contents)?;
        if !matches!(sites.version, 2..=5) {
            return Err(CaddyError::InvalidConfiguration(format!(
                "unsupported payload version {}",
                sites.version
            )));
        }
        if sites.zone_domain.is_some() && sites.version < 5 {
            return Err(CaddyError::InvalidConfiguration(
                "zone-level aliases require payload version 5".into(),
            ));
        }
        if sites.version == 5 && sites.zone_domain.is_none() {
            return Err(CaddyError::InvalidConfiguration(
                "payload version 5 requires the Cloudflare zone".into(),
            ));
        }
        let mut expected = Self::from_host_with_zone(
            host,
            &UiEnvironment {
                ui_domain: sites.ui_domain.clone(),
                acme_staging: sites.acme_staging,
            },
            services,
            sites.zone_domain.as_deref(),
        )?;
        auth::Policy::new(sites.tailnet_users.clone())?;
        if sites.version == 2 && sites.services.iter().any(|site| site.tailnet_identity) {
            return Err(CaddyError::InvalidConfiguration(
                "identity protection requires payload version 3; redeliver Caddy credentials"
                    .into(),
            ));
        }
        if sites.version < 4
            && sites
                .services
                .iter()
                .any(|s| s.transport != UpstreamTransport::Bridge)
        {
            return Err(CaddyError::InvalidConfiguration(
                "host upstreams require version 4; redeliver Caddy credentials".into(),
            ));
        }
        expected.version = sites.version;
        expected.tailnet_users.clone_from(&sites.tailnet_users);
        expected
            .homeassistant_api_token
            .clone_from(&sites.homeassistant_api_token);
        if sites != expected {
            return Err(CaddyError::InvalidConfiguration(
                "payload does not match this host's declared UI services".to_string(),
            ));
        }
        if sites.homeassistant_api_token.as_ref().is_some_and(|token| {
            !services
                .iter()
                .any(|service| service.name == "homeassistant")
                || !skillet_homeassistant::backup::valid_token(token)
        }) {
            return Err(CaddyError::InvalidConfiguration(
                "Home Assistant dashboard token is invalid or has no declared service".into(),
            ));
        }
        Ok(expected)
    }

    pub fn needs_identity_gate(&self) -> bool {
        self.services.iter().any(|site| site.tailnet_identity)
    }

    pub fn render(&self) -> String {
        use std::fmt::Write as _;

        let acme = if self.acme_staging {
            "    acme_ca https://acme-staging-v02.api.letsencrypt.org/directory\n"
        } else {
            ""
        };
        let mut config =
            format!("{{\n    admin off\n{acme}    acme_dns cloudflare {{env.CF_API_TOKEN}}\n}}\n");
        let homer_origins = self
            .services
            .iter()
            .filter(|site| site.name == "homer")
            .flat_map(|site| std::iter::once(&site.hostname).chain(&site.aliases))
            .map(|hostname| format!("https://{hostname}"))
            .collect::<Vec<_>>();
        for service in &self.services {
            for hostname in std::iter::once(&service.hostname).chain(&service.aliases) {
                let _ = writeln!(config, "\n{hostname} {{\n    tls {{\n        resolvers 1.1.1.1 1.0.0.1\n    }}\n    route {{\n        @outside_tailnet not remote_ip 100.64.0.0/10 fd7a:115c:a1e0::/48\n        respond @outside_tailnet \"Access denied by Skillet tailnet policy\" 403");
                if service.tailnet_identity {
                    let homepage_api = matches!(service.name.as_str(), "pihole" | "homeassistant")
                        && !homer_origins.is_empty();
                    if homepage_api {
                        let own_origin = format!("https://{hostname}");
                        let allowed_origins = homer_origins
                            .iter()
                            .filter(|origin| origin.as_str() != own_origin)
                            .collect::<Vec<_>>();
                        let denied = std::iter::once(format!(
                            "{{http.request.header.Origin}} != \"{own_origin}\""
                        ))
                        .chain(allowed_origins.iter().map(|origin| {
                            format!("{{http.request.header.Origin}} != \"{origin}\"")
                        }))
                        .collect::<Vec<_>>()
                        .join(" && ");
                        let mut cross_site_origins = String::new();
                        for origin in &allowed_origins {
                            let _ = write!(
                                cross_site_origins,
                                " && {{http.request.header.Origin}} != \"{origin}\""
                            );
                        }
                        let _ = writeln!(
                            config,
                            "        @cross_origin expression `{{http.request.header.Origin}} != \"\" && {denied}`\n        respond @cross_origin 403\n        @cross_site expression `{{http.request.header.Sec-Fetch-Site}} == \"cross-site\" && {{http.request.header.Origin}} != \"{own_origin}\"{cross_site_origins}`\n        respond @cross_site 403"
                        );
                        for (index, origin) in allowed_origins.iter().enumerate() {
                            let _ = writeln!(config, "        @homer_origin_{index} header Origin \"{origin}\"\n        header @homer_origin_{index} Access-Control-Allow-Origin \"{origin}\"\n        header @homer_origin_{index} Access-Control-Allow-Methods \"GET, OPTIONS\"\n        header @homer_origin_{index} Access-Control-Allow-Headers \"Authorization, Content-Type\"\n        header @homer_origin_{index} Vary Origin");
                        }
                    } else {
                        let _ = writeln!(config, "        @cross_origin expression `{{http.request.header.Origin}} != \"\" && {{http.request.header.Origin}} != \"https://{hostname}\"`\n        respond @cross_origin 403\n        @cross_site header Sec-Fetch-Site cross-site\n        respond @cross_site 403");
                    }
                    // A trailing '?' clears the inherited application query on the
                    // cloned auth request; the backend keeps its original URI.
                    config.push_str("        request_header -X-Skillet-Peer\n        request_header -X-Skillet-User\n        request_header -X-Webauth-User\n        request_header -Remote-User\n        request_header -Tailscale-User\n        forward_auth unix//run/skillet-tailnet-ui/auth.sock {\n            uri /auth?\n            header_up X-Skillet-Peer {http.request.remote}\n            copy_headers X-Skillet-User\n        }\n");
                }
                if service.tailnet_identity
                    && !homer_origins.is_empty()
                    && matches!(service.name.as_str(), "pihole" | "homeassistant")
                {
                    // Browser preflight requests have already passed the source and
                    // origin checks above and the tailnet identity broker here. Do
                    // not forward them to applications that reject CORS preflights.
                    config.push_str("        @homer_preflight method OPTIONS\n        respond @homer_preflight \"\" 204\n");
                }
                if service.tailnet_identity && service.anonymous_token_refresh {
                    // No application identity/token is minted. NO_AUTH backends ignore
                    // this marker; Caddy must authorize every subsequent request too.
                    config.push_str("        @anonymous_refresh {\n            method GET\n            path /api/refresh\n        }\n        header @anonymous_refresh Content-Type application/json\n        header @anonymous_refresh Cache-Control no-store\n        respond @anonymous_refresh `{\"token\":\"skillet-tailnet-authenticated\"}` 200\n");
                }
                if service.name == "homer" {
                    // The Homer image contains its static assets. Only its generated
                    // config is served from the host, after the identity gate.
                    config.push_str("        @homer_config path /assets/config.yml\n        root @homer_config /etc/skillet/homer\n        header @homer_config Cache-Control no-store\n        file_server @homer_config\n");
                }
                let _ = writeln!(
                    config,
                    "        reverse_proxy {}{}\n    }}\n}}",
                    match service.transport {
                        UpstreamTransport::Bridge =>
                            format!("{}:{}", service.upstream, service.port),
                        UpstreamTransport::HostLoopback =>
                            format!("unix//run/skillet-ui-host/{}.sock", service.name),
                    },
                    if matches!(service.name.as_str(), "pihole" | "homeassistant") {
                        let mut upstream_headers = String::from(
                            " {\n            header_down -Access-Control-Allow-Origin",
                        );
                        if service.name == "pihole" {
                            // Pi-hole v6 only serves the UI for its configured webserver.domain.
                            // Its API also rejects Homer-origin requests as CSRF, so present the
                            // canonical local origin to Pi-hole and add browser CORS at Caddy.
                            upstream_headers.push_str(
                                "\n            header_up Host pi.hole\n            header_up Origin http://pi.hole",
                            );
                        } else {
                            // The frontend origin has already passed the exact Homer
                            // allowlist and tailnet identity gate. Present HA with its
                            // own origin so its API accepts the dashboard request.
                            let _ = write!(
                                upstream_headers,
                                "\n            header_up Origin https://{hostname}"
                            );
                        }
                        upstream_headers.push_str("\n        }");
                        upstream_headers
                    } else {
                        String::new()
                    },
                );
            }
        }
        config
    }
}

fn build_sites(
    host: &str,
    ui_domain: &str,
    zone_domain: Option<&str>,
    machine_hostname: &str,
    services: &[UiService],
) -> Result<Vec<CaddySite>, CaddyError> {
    let mut sites = Vec::with_capacity(services.len());
    let mut all_hostnames = BTreeMap::from([(machine_hostname.to_string(), "machine")]);
    for service in services {
        validate_label(&service.name, "service name")?;
        validate_label(&service.upstream, "upstream name")?;
        if service.transport == UpstreamTransport::HostLoopback && !service.tailnet_identity {
            return Err(CaddyError::InvalidConfiguration(
                "host upstream requires identity protection".into(),
            ));
        }
        if service.anonymous_token_refresh && !service.tailnet_identity {
            return Err(CaddyError::InvalidConfiguration(
                "anonymous token refresh requires identity protection".into(),
            ));
        }
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
        let hostname = format!("{}.{}.{}", service.name, host, ui_domain);
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
            let alias_hostname = resolve_alias(alias, host, ui_domain, zone_domain)?;
            if alias_conflicts(
                &mut all_hostnames,
                &alias_hostname,
                machine_hostname,
                service,
                &aliases,
            )? {
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
            transport: service.transport,
            port: service.port,
            tailnet_identity: service.tailnet_identity,
            anonymous_token_refresh: service.anonymous_token_refresh,
        });
    }
    Ok(sites)
}

fn alias_conflicts(
    names: &mut BTreeMap<String, &'static str>,
    alias: &str,
    machine_hostname: &str,
    service: &UiService,
    existing_aliases: &[String],
) -> Result<bool, CaddyError> {
    validate_domain(alias)?;
    // The machine's A/AAAA name is also its private dashboard. Reserve it
    // from every other service and reject duplicate dashboard aliases.
    if alias == machine_hostname {
        return Ok(service.name != "homer"
            || !service.tailnet_identity
            || existing_aliases.iter().any(|name| name == alias));
    }
    Ok(names.insert(alias.to_string(), "UI alias").is_some())
}

fn resolve_alias(
    alias: &str,
    host: &str,
    ui_domain: &str,
    zone_domain: Option<&str>,
) -> Result<String, CaddyError> {
    let (relative, domain) = if let Some(relative) = alias.strip_suffix(".{zone}") {
        let zone = zone_domain.ok_or_else(|| {
            CaddyError::InvalidConfiguration(
                "zone-level UI alias requires the Cloudflare zone; redeliver Caddy credentials"
                    .into(),
            )
        })?;
        (relative, zone)
    } else {
        (alias, ui_domain)
    };
    Ok(format!("{}.{}", expand_alias(relative, host)?, domain))
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
    network_name: &str,
) -> Result<(), CaddyError>
where
    S: AccountLookupResource + AccountResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let (networks, auth_volumes) = backend_resources(system, files, sites, network_name)?;
    let caddyfile = sites.render();
    let config_dir = Path::new("/etc/skillet/caddy");
    files.ensure_directory(
        config_dir,
        Some(0o755),
        &Ownership::named(Some("root"), Some("root")),
    )?;
    files.ensure_file(
        &config_dir.join("Caddyfile"),
        caddyfile.as_bytes(),
        Some(0o644),
        &Ownership::named(Some("root"), Some("root")),
    )?;

    let mut container = BTreeMap::new();
    container.insert(
        "Container".to_string(),
        vec![
            "AutoUpdate=registry".to_string(),
            "ContainerName=caddy".to_string(),
            skillet_datadog::process("caddy", "caddy run"),
        ],
    );
    container.insert("Service".to_string(), vec!["Restart=always".to_string()]);
    container.insert(
        "Unit".to_string(),
        vec![
            "Description=Caddy private UI reverse proxy".to_string(),
            "After=network-online.target".to_string(),
        ],
    );

    if sites.needs_identity_gate() {
        container.entry("Unit".into()).or_default().extend([
            "Requires=skillet-tailnet-ui.service".into(),
            "After=skillet-tailnet-ui.service".into(),
        ]);
        container.entry("Container".into()).or_default().extend([
            "Sysctl=net.ipv4.ip_forward=0".into(),
            "Sysctl=net.ipv6.conf.all.forwarding=0".into(),
        ]);
    }
    for site in sites
        .services
        .iter()
        .filter(|s| s.transport == UpstreamTransport::HostLoopback)
    {
        container.entry("Unit".into()).or_default().extend([
            format!("Requires=skillet-ui-host-{}.service", site.name),
            format!("After=skillet-ui-host-{}.service", site.name),
        ]);
    }
    let mut config = PodmanConfig {
        name: "caddy".to_string(),
        image: "ghcr.io/caddybuilds/caddy-cloudflare:2".to_string(),
        network_attachments: networks,
        port_publications: ["::", "0.0.0.0"]
            .into_iter()
            .map(|address| PortPublication {
                host_address: match address {
                    "::" => std::net::Ipv6Addr::UNSPECIFIED.into(),
                    _ => std::net::Ipv4Addr::UNSPECIFIED.into(),
                },
                host_port: 443,
                container_port: 443,
                protocol: PortProtocol::Tcp,
            })
            .collect(),
        storage_dependency: Some(MountDependency::shared_service_data()),
        process_identity: ProcessIdentity::ImageDefault,
        namespace_mapping: None,
        volumes: state_volumes(),
        secrets: vec![QuadletSecret {
            secret_name: "cloudflare_acme_token".to_string(),
            target: SecretTarget::Environment {
                env_var_name: "CF_API_TOKEN".to_string(),
            },
        }],
        config_revisions: vec![caddyfile.into_bytes()],
        extra_config: container,
    };
    config.volumes.extend(auth_volumes);
    if sites.services.iter().any(|site| site.name == "homer") {
        config.volumes.push(Volume {
            host_path: "/etc/skillet/homer".into(),
            container_path: "/etc/skillet/homer".into(),
            // Homer and Caddy share the rendered config and Homer assets.
            // Use a shared SELinux label so either container can read it.
            options: Some("ro,z".into()),
            host_mode: None,
            host_ownership: None,
        });
    }
    // Preserve the caller's network configuration and let Podman create its
    // persistent directories on the shared data filesystem.
    for path in ["/var/lib/data/caddy/data", "/var/lib/data/caddy/config"] {
        files.ensure_directory(
            Path::new(path),
            Some(0o755),
            &Ownership::named(Some("root"), Some("root")),
        )?;
    }
    skillet_podman::container(system, files, config)?;
    Ok(())
}

fn backend_resources<S, F>(
    system: &S,
    files: &F,
    sites: &CaddySites,
    network_name: &str,
) -> Result<(Vec<NetworkAttachment>, Vec<Volume>), CaddyError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let policy = auth::Policy::new(sites.tailnet_users.clone())?;
    let mut networks = vec![NetworkAttachment::Bridge(network_name.to_string())];
    let mut auth_volumes = Vec::new();
    if sites.needs_identity_gate() {
        auth::apply(system, files, &policy)?;
        auth_volumes.push(Volume {
            host_path: "/run/skillet-tailnet-ui".into(),
            container_path: "/run/skillet-tailnet-ui".into(),
            options: Some("ro,z".into()),
            host_mode: None,
            host_ownership: None,
        });
        for site in sites
            .services
            .iter()
            .filter(|site| site.tailnet_identity && site.transport == UpstreamTransport::Bridge)
        {
            let network = private_network(&sites.host, &site.name);
            skillet_podman::ensure_network(system, files, &network)?;
            networks.push(NetworkAttachment::Bridge(network.unit_name));
        }
    }
    host_proxy::apply(system, files, sites, &mut auth_volumes)?;
    Ok((networks, auth_volumes))
}

/// A private backend whose strict firewall isolation allows only joined containers.
pub fn private_network(host: &str, service: &str) -> skillet_podman::PodmanNetwork {
    let name = format!("{host}-{service}");
    skillet_podman::PodmanNetwork {
        unit_name: name.clone(),
        options: vec![
            "Driver=bridge".into(),
            // Netavark skips firewall/isolation rules for internal networks.
            "Internal=false".into(),
            "DisableDNS=false".into(),
            "IPv6=true".into(),
            "Options=isolate=strict".into(),
            format!("NetworkName={name}"),
        ],
    }
}

fn state_volumes() -> Vec<Volume> {
    vec![
        Volume {
            host_path: "/etc/skillet/caddy".to_string(),
            container_path: "/etc/caddy".to_string(),
            options: Some("ro,Z".to_string()),
            host_mode: None,
            host_ownership: None,
        },
        Volume {
            host_path: "/var/lib/data/caddy/data".to_string(),
            container_path: "/data".to_string(),
            options: Some("Z".to_string()),
            host_mode: None,
            host_ownership: None,
        },
        Volume {
            host_path: "/var/lib/data/caddy/config".to_string(),
            container_path: "/config".to_string(),
            options: Some("Z".to_string()),
            host_mode: None,
            host_ownership: None,
        },
    ]
}
