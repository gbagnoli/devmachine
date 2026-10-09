//! Canonical host profiles and their shared composition.

use skillet_core::{
    credentials::{CredentialError, CredentialInputs},
    files::{FileError, FileResource},
    system::{SystemError, SystemResource},
};
use skillet_podman::{PodmanConfig, ProcessIdentity, QuadletSecret, SecretTarget, Volume};
use std::collections::BTreeMap;
use thiserror::Error;

pub mod profile;
pub use profile::{
    declared_profiles, profile_for_host, profile_for_name, AcceptanceListener, AcceptanceOwner,
    AcceptanceService, CredentialConsumer, HealthProbe, HostAcceptancePlan, HostId, HostProfile,
    HostService, ListenerProtocol, ServiceConfig, UiServiceDeclaration,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostApplyPhase {
    Base,
    Full,
    Caddy,
    Ddns,
    Datadog,
    Smtp,
}

pub fn credentials_for_phase(
    hostname: &str,
    phase: HostApplyPhase,
) -> Result<Vec<&'static str>, ApplyError> {
    let profile =
        profile_for_name(hostname).ok_or_else(|| ApplyError::UnknownHost(hostname.to_string()))?;
    if phase == HostApplyPhase::Base {
        return Ok(Vec::new());
    }
    Ok(profile
        .credential_consumers()
        .into_iter()
        .filter(|consumer| match phase {
            HostApplyPhase::Full => !matches!(
                consumer.unit,
                "caddy.service"
                    | "cloudflare-ddns.service"
                    | "datadog-agent.service"
                    | "postfix.service"
            ),
            HostApplyPhase::Caddy => consumer.unit == "caddy.service",
            HostApplyPhase::Ddns => consumer.unit == "cloudflare-ddns.service",
            HostApplyPhase::Datadog => consumer.unit == "datadog-agent.service",
            HostApplyPhase::Smtp => consumer.unit == "postfix.service",
            HostApplyPhase::Base => false,
        })
        .map(|consumer| consumer.credential)
        .collect())
}

/// Explicit guest boot expectations. Consolidated with the canonical capability
/// declaration in refactoring workstream 2; the VM orchestrator has no host cases.
pub struct HostBootPolicy {
    pub signed_image: String,
    pub masked_units: Vec<&'static str>,
}

pub fn boot_policy_for_host(hostname: &str) -> Option<HostBootPolicy> {
    let profile = profile_for_name(hostname)?;
    let signed_image = profile.signed_image?;
    Some(HostBootPolicy {
        signed_image: signed_image.to_string(),
        masked_units: profile.masked_units.to_vec(),
    })
}

#[derive(Error, Debug)]
pub enum ApplyError {
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("File error: {0}")]
    File(#[from] FileError),
    #[error("Credential error: {0}")]
    Credential(#[from] CredentialError),
    #[error("Hardening apply error: {0}")]
    Hardening(String),
    #[error("Pihole apply error: {0}")]
    Pihole(#[from] skillet_pihole::PiholeError),
    #[error("UniFi apply error: {0}")]
    Unifi(#[from] skillet_unifi::UnifiError),
    #[error("Syncthing apply error: {0}")]
    Syncthing(#[from] skillet_syncthing::SyncthingError),
    #[error("Btrbk apply error: {0}")]
    Btrbk(#[from] skillet_btrbk::BtrbkError),
    #[error("Caddy apply error: {0}")]
    Caddy(#[from] skillet_caddy::CaddyError),
    #[error("DDNS apply error: {0}")]
    Ddns(#[from] skillet_ddns::DdnsError),
    #[error("Datadog apply error: {0}")]
    Datadog(#[from] skillet_datadog::DatadogError),
    #[error("SMTP apply error: {0}")]
    Smtp(#[from] skillet_smtp::SmtpError),
    #[error("Podman error: {0}")]
    Podman(#[from] skillet_podman::PodmanError),
    #[error("Fixture input error: {0}")]
    FixtureInput(String),
    #[error("Unknown host profile: {0}")]
    UnknownHost(String),
}

/// Shared host baseline used by both CLI entry points.
pub fn apply_base(system: &dyn SystemResource, files: &dyn FileResource) -> Result<(), ApplyError> {
    skillet_hardening::apply(system, files).map_err(|e| ApplyError::Hardening(e.to_string()))
}

/// Name of the systemd credential (and podman secret) holding the Pi-hole
/// web UI password. Full `clamps` apply reads it from the systemd credential
/// directory supplied to that invocation.
pub const PIHOLE_WEB_PASSWORD_CREDENTIAL: &str = "pihole_web_password";
/// Name of the systemd credential (and Podman secret) holding the one-use
/// Tailscale enrollment key used when the node has not joined yet.
pub const TAILSCALE_AUTH_KEY_CREDENTIAL: &str = "tailscale_auth_key";
/// Credential containing private Caddy hostnames and the ACME environment.
pub const CADDY_SITES_CREDENTIAL: &str = "caddy_sites";
/// Cloudflare token used only by Caddy's DNS-01 challenge provider.
pub const CLOUDFLARE_ACME_TOKEN_CREDENTIAL: &str = "cloudflare_acme_token";
pub const CLOUDFLARE_DDNS_CONFIG_CREDENTIAL: &str = skillet_ddns::CREDENTIAL;

/// UI services declared by a host. This is the canonical input used by Caddy
/// both on the workstation (to build delivery payloads) and on the host (to
/// validate and apply them).
pub struct HostUiConfig {
    pub network_name: String,
    pub services: Vec<skillet_caddy::UiService>,
}

pub fn ui_config_for_host(hostname: &str) -> Option<HostUiConfig> {
    let profile = profile_for_name(hostname)?;
    let services = profile.ui_services();
    (!services.is_empty()).then(|| HostUiConfig {
        network_name: profile.id.as_str().to_string(),
        services,
    })
}

fn tailscale_config(hostname: &str, auth_key: String, state_path: &str) -> PodmanConfig {
    let extra_config = BTreeMap::from([
        (
            "Container".to_string(),
            vec![
                "ContainerName=tailscale".to_string(),
                skillet_datadog::process("tailscale", "tailscaled"),
                "AddCapability=NET_ADMIN".to_string(),
                "AddCapability=NET_RAW".to_string(),
                "AddDevice=/dev/net/tun:/dev/net/tun".to_string(),
                "AutoUpdate=registry".to_string(),
                "Environment=TS_ACCEPT_DNS=false".to_string(),
                "Environment=TS_AUTH_ONCE=true".to_string(),
                format!("Environment=TS_HOSTNAME={hostname}"),
                "Environment=TS_STATE_DIR=/var/lib/tailscale".to_string(),
                "Environment=TS_USERSPACE=false".to_string(),
            ],
        ),
        (
            "Unit".to_string(),
            vec!["After=network-online.target".to_string()],
        ),
    ]);
    PodmanConfig {
        name: "tailscale".to_string(),
        image: "docker.io/tailscale/tailscale:stable".to_string(),
        network_attachments: vec![skillet_podman::NetworkAttachment::Host],
        port_publications: Vec::new(),
        storage_dependency: Some(skillet_podman::MountDependency::shared_service_data()),
        process_identity: ProcessIdentity::ImageDefault,
        namespace_mapping: None,
        volumes: vec![Volume {
            host_path: state_path.to_string(),
            container_path: "/var/lib/tailscale".to_string(),
            options: Some("Z".to_string()),
            host_mode: None,
            host_ownership: None,
        }],
        secrets: vec![QuadletSecret {
            secret_name: TAILSCALE_AUTH_KEY_CREDENTIAL.to_string(),
            target: SecretTarget::Environment {
                env_var_name: "TS_AUTHKEY".to_string(),
            },
        }],
        config_revisions: vec![auth_key.into_bytes()],
        extra_config,
    }
}

/// Apply a service from a host declaration using only its declared config.
fn apply_syncthing(
    system: &dyn SystemResource,
    files: &dyn FileResource,
    profile: &HostProfile,
    service: &HostService,
) -> Result<(), ApplyError> {
    let ServiceConfig::Syncthing {
        data_path,
        data_owner,
        data_group,
        container_uid,
        container_gid,
    } = service.config
    else {
        return Err(ApplyError::FixtureInput(
            "syncthing apply received an incompatible service declaration".to_string(),
        ));
    };
    skillet_syncthing::apply(
        system,
        files,
        skillet_syncthing::SyncthingConfig {
            data_path: data_path.to_string(),
            data_owner: data_owner.to_string(),
            data_group: data_group.to_string(),
            container_uid,
            container_gid,
            network_name: profile.id.as_str().to_string(),
        },
    )?;
    Ok(())
}

/// Apply a canonical host profile. Unknown identities are rejected.
pub fn apply_host(
    hostname: &str,
    system: &dyn SystemResource,
    files: &dyn FileResource,
    credentials: &CredentialInputs,
) -> Result<(), ApplyError> {
    let profile =
        profile_for_name(hostname).ok_or_else(|| ApplyError::UnknownHost(hostname.to_string()))?;
    apply_profile(&profile, system, files, credentials)
}

#[allow(clippy::similar_names)]
fn apply_profile(
    profile: &HostProfile,
    system: &dyn SystemResource,
    files: &dyn FileResource,
    credentials: &CredentialInputs,
) -> Result<(), ApplyError> {
    apply_base(system, files)?;
    if profile.requires_data_mount {
        files.require_btrfs_subvolume_mount(
            std::path::Path::new("/var/lib/data"),
            std::path::Path::new("/var"),
            "/data",
        )?;
    }
    if profile.requires_pihole_dns_listener_policy() {
        skillet_podman::ensure_dns_listener_port(files, 54)?;
    }
    if profile.requires_service_network() {
        skillet_podman::ensure_network(system, files, &profile.service_network())?;
    }
    for service in &profile.services {
        match &service.config {
            // Independently credential-gated optional services have their own phase.
            ServiceConfig::Ddns | ServiceConfig::Datadog { .. } | ServiceConfig::Smtp => {}
            ServiceConfig::Pihole { custom_dns } => {
                let password = credentials
                    .require(PIHOLE_WEB_PASSWORD_CREDENTIAL)?
                    .to_string();
                system.ensure_podman_secret(PIHOLE_WEB_PASSWORD_CREDENTIAL, &password)?;
                let user = system.user_by_name("pihole")?;
                let group = system.group_by_name("pihole")?;
                skillet_pihole::apply(
                    system,
                    files,
                    &skillet_pihole::PiholeUser {
                        uid: user.map(|identity| identity.uid),
                        gid: group.map(|identity| identity.gid),
                        name: "pihole".to_string(),
                        group_name: "pihole".to_string(),
                    },
                    vec![QuadletSecret {
                        secret_name: PIHOLE_WEB_PASSWORD_CREDENTIAL.to_string(),
                        target: SecretTarget::File {
                            target_path: "/run/secrets/pihole_web_password".to_string(),
                            mode: Some("0400".to_string()),
                            uid: None,
                            gid: None,
                        },
                    }],
                    custom_dns
                        .iter()
                        .map(|(ip, domain)| ((*ip).to_string(), (*domain).to_string()))
                        .collect::<BTreeMap<_, _>>(),
                    profile.id.as_str().to_string(),
                )?;
            }
            ServiceConfig::Syncthing { .. } => apply_syncthing(system, files, profile, service)?,
            ServiceConfig::Unifi => skillet_unifi::apply(system, files)?,
            ServiceConfig::Tailscale { state_path } => {
                let auth_key = credentials
                    .require(TAILSCALE_AUTH_KEY_CREDENTIAL)?
                    .to_string();
                system.ensure_podman_secret(TAILSCALE_AUTH_KEY_CREDENTIAL, &auth_key)?;
                let hostname = runtime_hostname(files, profile.id.as_str())?;
                skillet_podman::container(
                    system,
                    files,
                    tailscale_config(&hostname, auth_key, state_path),
                )?;
            }
            ServiceConfig::Btrbk { .. } => {
                let config = profile.btrbk_config().ok_or_else(|| {
                    ApplyError::FixtureInput("btrbk service lacks snapshot configuration".into())
                })?;
                skillet_btrbk::apply(system, files, &config)?;
            }
        }
    }
    Ok(())
}

fn runtime_hostname(files: &dyn FileResource, fallback: &str) -> Result<String, ApplyError> {
    let hostname = files
        .read_file(std::path::Path::new("/etc/hostname"))?
        .map(|contents| {
            String::from_utf8(contents).map_err(|error| ApplyError::FixtureInput(error.to_string()))
        })
        .transpose()?
        .unwrap_or_else(|| fallback.to_string());
    let hostname = hostname.trim();
    if hostname.is_empty()
        || hostname.len() > 63
        || !hostname
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(ApplyError::FixtureInput(
            "invalid runtime hostname in /etc/hostname".to_string(),
        ));
    }
    Ok(hostname.to_string())
}

#[cfg(test)]
#[path = "hosts_tests.rs"]
mod tests;

pub fn apply_host_phase(
    hostname: &str,
    phase: HostApplyPhase,
    system: &dyn SystemResource,
    files: &dyn FileResource,
    credentials: &CredentialInputs,
) -> Result<(), ApplyError> {
    match phase {
        HostApplyPhase::Base => apply_base(system, files),
        HostApplyPhase::Full => apply_host(hostname, system, files, credentials),
        HostApplyPhase::Caddy => apply_caddy_host(hostname, system, files, credentials),
        HostApplyPhase::Ddns => apply_ddns_host(hostname, system, files, credentials),
        HostApplyPhase::Datadog => apply_datadog_host(hostname, system, files, credentials),
        HostApplyPhase::Smtp => {
            profile_for_name(hostname)
                .filter(|profile| profile.supports_service("smtp"))
                .ok_or_else(|| ApplyError::FixtureInput("host does not declare SMTP".into()))?;
            skillet_smtp::apply(
                system,
                files,
                credentials.require(skillet_smtp::CREDENTIAL)?,
            )?;
            Ok(())
        }
    }
}

fn apply_ddns_host(
    hostname: &str,
    system: &dyn SystemResource,
    files: &dyn FileResource,
    credentials: &CredentialInputs,
) -> Result<(), ApplyError> {
    let profile =
        profile_for_name(hostname).ok_or_else(|| ApplyError::UnknownHost(hostname.to_string()))?;
    if !profile.supports_service("ddns") {
        return Err(ApplyError::FixtureInput(
            "host does not declare DDNS".into(),
        ));
    }
    let payload = credentials.require(CLOUDFLARE_DDNS_CONFIG_CREDENTIAL)?;
    // Reject malformed secret input before any network or container mutation.
    skillet_ddns::config::Payload::parse(payload).map_err(skillet_ddns::DdnsError::from)?;
    files.require_btrfs_subvolume_mount(
        std::path::Path::new("/var/lib/data"),
        std::path::Path::new("/var"),
        "/data",
    )?;
    skillet_podman::ensure_network(system, files, &profile.service_network())?;
    skillet_ddns::apply(system, files, payload, profile.id.as_str())?;
    Ok(())
}

fn apply_datadog_host(
    hostname: &str,
    system: &dyn SystemResource,
    files: &dyn FileResource,
    credentials: &CredentialInputs,
) -> Result<(), ApplyError> {
    let profile =
        profile_for_name(hostname).ok_or_else(|| ApplyError::UnknownHost(hostname.into()))?;
    let network_monitoring = profile
        .services
        .iter()
        .find_map(|service| match service.config {
            ServiceConfig::Datadog {
                network_monitoring, ..
            } => Some(network_monitoring),
            _ => None,
        })
        .ok_or_else(|| ApplyError::FixtureInput("host does not declare Datadog".into()))?;
    let payload = credentials.require(skillet_datadog::CREDENTIAL)?;
    let hostname = runtime_hostname(files, profile.id.as_str())?;
    let mut monitored_units = profile
        .acceptance_plan_with_ddns(true)
        .services
        .into_iter()
        .map(|service| service.unit)
        .collect::<Vec<_>>();
    if profile.supports_service("smtp") {
        monitored_units.extend(
            skillet_smtp::MONITORED_UNITS
                .iter()
                .map(|unit| (*unit).to_string()),
        );
    }
    skillet_datadog::apply(
        system,
        files,
        payload,
        &skillet_datadog::RuntimeConfig {
            hostname: &hostname,
            network_monitoring,
            monitored_units: &monitored_units,
        },
    )?;
    Ok(())
}

fn apply_caddy_host(
    hostname: &str,
    system: &dyn SystemResource,
    files: &dyn FileResource,
    credentials: &CredentialInputs,
) -> Result<(), ApplyError> {
    let profile =
        profile_for_name(hostname).ok_or_else(|| ApplyError::UnknownHost(hostname.to_string()))?;
    let services = profile.ui_services();
    if services.is_empty() {
        return Err(ApplyError::FixtureInput(format!(
            "host {hostname} declares no UI services"
        )));
    }
    let ui_config = HostUiConfig {
        network_name: profile.id.as_str().to_string(),
        services,
    };
    if profile.requires_data_mount {
        files.require_btrfs_subvolume_mount(
            std::path::Path::new("/var/lib/data"),
            std::path::Path::new("/var"),
            "/data",
        )?;
    }
    skillet_podman::ensure_network(system, files, &profile.service_network())?;
    let sites = skillet_caddy::CaddySites::parse(
        credentials.require(CADDY_SITES_CREDENTIAL)?,
        hostname,
        &ui_config.services,
    )?;
    let token = credentials
        .require(CLOUDFLARE_ACME_TOKEN_CREDENTIAL)?
        .to_string();
    system.ensure_podman_secret(CLOUDFLARE_ACME_TOKEN_CREDENTIAL, &token)?;
    skillet_caddy::apply(system, files, &sites, &ui_config.network_name)?;
    Ok(())
}
