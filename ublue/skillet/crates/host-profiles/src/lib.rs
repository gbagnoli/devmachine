//! Canonical host profiles and their shared composition.

use skillet_core::{
    credentials::{CredentialError, CredentialManager},
    files::{FileError, FileResource},
    system::{SystemError, SystemResource},
};
use skillet_podman::{
    ContainerUser, PodmanConfig, PodmanNetwork, QuadletSecret, SecretTarget, Volume,
};
use std::collections::BTreeMap;
use thiserror::Error;

pub mod profile;
pub use profile::{
    declared_profiles, profile_for_host, profile_for_name, CredentialConsumer, HostId, HostProfile,
    HostService, ServiceConfig, UiServiceDeclaration,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostApplyPhase {
    Base,
    Full,
    Caddy,
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
    #[error("Podman error: {0}")]
    Podman(#[from] skillet_podman::PodmanError),
    #[error("Fixture input error: {0}")]
    FixtureInput(String),
    #[error("Unknown host profile: {0}")]
    UnknownHost(String),
}

mod user_lookup {
    use users::{get_group_by_name, get_user_by_name};

    /// Look up UID for a username, returns None if user doesn't exist
    pub fn lookup_uid(username: &str) -> Option<u32> {
        get_user_by_name(username).map(|user| user.uid())
    }

    /// Look up GID for a group name, returns None if group doesn't exist
    pub fn lookup_gid(groupname: &str) -> Option<u32> {
        get_group_by_name(groupname).map(|group| group.gid())
    }
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

/// UI services declared by a host. This is the canonical input used by Caddy
/// both on the workstation (to build delivery payloads) and on the host (to
/// validate and apply them).
pub struct HostUiConfig {
    pub network: PodmanNetwork,
    pub services: Vec<skillet_caddy::UiService>,
}

pub fn ui_config_for_host(hostname: &str) -> Option<HostUiConfig> {
    let profile = profile_for_name(hostname)?;
    let services = profile.ui_services();
    (!services.is_empty()).then(|| HostUiConfig {
        network: profile.service_network(),
        services,
    })
}

fn tailscale_config(hostname: &str, auth_key: String, state_path: &str) -> PodmanConfig {
    let extra_config = BTreeMap::from([
        (
            "Container".to_string(),
            vec![
                "ContainerName=tailscale".to_string(),
                "AddCapability=NET_ADMIN".to_string(),
                "AddCapability=NET_RAW".to_string(),
                "AddDevice=/dev/net/tun:/dev/net/tun".to_string(),
                "AutoUpdate=registry".to_string(),
                "Environment=TS_ACCEPT_DNS=false".to_string(),
                "Environment=TS_AUTH_ONCE=true".to_string(),
                format!("Environment=TS_HOSTNAME={hostname}"),
                "Environment=TS_STATE_DIR=/var/lib/tailscale".to_string(),
                "Environment=TS_USERSPACE=false".to_string(),
                "Network=host".to_string(),
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
        networks: Vec::new(),
        user: ContainerUser {
            container_uid: 0,
            container_gid: 0,
            host_user: None,
        },
        create_host_user: false,
        volumes: vec![Volume {
            host_path: state_path.to_string(),
            container_path: "/var/lib/tailscale".to_string(),
            options: Some("Z".to_string()),
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
        uid,
        gid,
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
            uid,
            gid,
            network: profile.service_network(),
        },
    )?;
    Ok(())
}

/// Apply a canonical host profile. Unknown identities are rejected.
pub fn apply_host(
    hostname: &str,
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), ApplyError> {
    let profile =
        profile_for_name(hostname).ok_or_else(|| ApplyError::UnknownHost(hostname.to_string()))?;
    apply_profile(&profile, system, files)
}

#[allow(clippy::similar_names)]
fn apply_profile(
    profile: &HostProfile,
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), ApplyError> {
    apply_base(system, files)?;
    if profile.requires_data_mount {
        files.require_btrfs_subvolume_mount(
            std::path::Path::new("/var/lib/data"),
            std::path::Path::new("/var"),
            "/data",
        )?;
    }
    for service in &profile.services {
        match &service.config {
            ServiceConfig::Pihole { custom_dns } => {
                let password =
                    CredentialManager::new()?.read_secret(PIHOLE_WEB_PASSWORD_CREDENTIAL)?;
                system.ensure_podman_secret(PIHOLE_WEB_PASSWORD_CREDENTIAL, &password)?;
                skillet_pihole::apply(
                    system,
                    files,
                    &skillet_pihole::PiholeUser {
                        uid: user_lookup::lookup_uid("pihole"),
                        gid: user_lookup::lookup_gid("pihole"),
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
                    profile.service_network(),
                )?;
            }
            ServiceConfig::Syncthing { .. } => apply_syncthing(system, files, profile, service)?,
            ServiceConfig::Unifi => skillet_unifi::apply(system, files)?,
            ServiceConfig::Tailscale { state_path } => {
                let auth_key =
                    CredentialManager::new()?.read_secret(TAILSCALE_AUTH_KEY_CREDENTIAL)?;
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
            "invalid hostname in /etc/hostname for Tailscale".to_string(),
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
) -> Result<(), ApplyError> {
    match phase {
        HostApplyPhase::Base => apply_base(system, files),
        HostApplyPhase::Full => apply_host(hostname, system, files),
        HostApplyPhase::Caddy => apply_caddy_host(hostname, system, files),
    }
}

fn apply_caddy_host(
    hostname: &str,
    system: &dyn SystemResource,
    files: &dyn FileResource,
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
        network: profile.service_network(),
        services,
    };
    if profile.requires_data_mount {
        files.require_btrfs_subvolume_mount(
            std::path::Path::new("/var/lib/data"),
            std::path::Path::new("/var"),
            "/data",
        )?;
    }
    let credentials = CredentialManager::new()?;
    let sites = skillet_caddy::CaddySites::parse(
        &credentials.read_secret(CADDY_SITES_CREDENTIAL)?,
        hostname,
        &ui_config.services,
    )?;
    let token = credentials.read_secret(CLOUDFLARE_ACME_TOKEN_CREDENTIAL)?;
    system.ensure_podman_secret(CLOUDFLARE_ACME_TOKEN_CREDENTIAL, &token)?;
    skillet_caddy::apply(system, files, &sites, ui_config.network)?;
    Ok(())
}
