//! Canonical per-host apply functions.
//!
//! Each supported host has exactly one apply function here. The per-host
//! binaries (`skillet-<host>`) and the generic `skillet apply --host <host>`
//! dispatcher both call these, so host logic can never diverge between the
//! two entry points again.

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum ApplyPhase {
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
    let masked_units = match hostname {
        "clamps" => vec!["systemd-resolved.service"],
        "beezelbot" => Vec::new(),
        _ => return None,
    };
    Some(HostBootPolicy {
        signed_image: format!("ghcr.io/gbagnoli/ucore-{hostname}"),
        masked_units,
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

/// Apply the beezelbot host configuration (hardening + Syncthing).
pub fn apply_beezelbot(
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), ApplyError> {
    apply_full_base(system, files)?;
    apply_syncthing(system, files, host_service_network("beezelbot"))
}

/// Shared host baseline used by both CLI entry points.
pub fn apply_base(system: &dyn SystemResource, files: &dyn FileResource) -> Result<(), ApplyError> {
    skillet_hardening::apply(system, files).map_err(|e| ApplyError::Hardening(e.to_string()))
}

/// Shared full-apply baseline. Base apply runs before the signed image is
/// ready, while full apply requires the persistent data mount.
pub fn apply_full_base(
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), ApplyError> {
    apply_base(system, files)?;
    files.require_btrfs_subvolume_mount(
        std::path::Path::new("/var/lib/data"),
        std::path::Path::new("/var"),
        "/data",
    )?;
    Ok(())
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
    match hostname {
        "clamps" => Some(HostUiConfig {
            network: host_service_network("clamps"),
            services: vec![
                skillet_caddy::UiService {
                    name: "pihole".to_string(),
                    upstream: "pihole".to_string(),
                    port: 8088,
                    aliases: Vec::new(),
                },
                skillet_caddy::UiService {
                    name: "syncthing".to_string(),
                    upstream: "syncthing".to_string(),
                    port: 8384,
                    aliases: vec!["sync.{host}".to_string()],
                },
            ],
        }),
        "beezelbot" => Some(HostUiConfig {
            network: host_service_network("beezelbot"),
            services: vec![skillet_caddy::UiService {
                name: "syncthing".to_string(),
                upstream: "syncthing".to_string(),
                port: 8384,
                aliases: vec!["sync.{host}".to_string()],
            }],
        }),
        _ => None,
    }
}

/// Custom DNS records for the clamps Pi-hole (`ip -> fqdn`).
// TODO: replace with the real LAN IP and domain before the production
// cutover; these are still the old placeholder values.
const CLAMPS_CUSTOM_DNS_RECORDS: &[(&str, &str)] = &[("192.168.1.100", "my.custom.domain")];

fn host_service_network(host: &str) -> PodmanNetwork {
    PodmanNetwork {
        unit_name: host.to_string(),
        options: vec![
            "DisableDNS=false".to_string(),
            "Driver=bridge".to_string(),
            "Gateway=172.26.26.1".to_string(),
            "Gateway=fd59:4e23:2950:11f5::1".to_string(),
            "IPv6=true".to_string(),
            format!("NetworkName={host}"),
            "Subnet=172.26.26.0/24".to_string(),
            "Subnet=fd59:4e23:2950:11f5::/64".to_string(),
        ],
    }
}

fn clamps_tailscale_config(hostname: &str, auth_key: String) -> PodmanConfig {
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
            host_path: "/var/lib/data/tailscale".to_string(),
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

/// Apply the clamps host configuration (hardening + Pi-hole).
///
/// The credential manager is constructed lazily here: hosts that need no
/// secrets (e.g. beezelbot) never touch `CREDENTIALS_DIRECTORY`.
// pihole uid/gid lookups are intentionally parallel
#[allow(clippy::similar_names)]
pub fn apply_clamps(
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), ApplyError> {
    // Check before Podman can create a graphroot on the fallback /var tree.
    apply_full_base(system, files)?;

    // 1. Ingest secrets from systemd (lazily: only clamps needs these values)
    let credentials = CredentialManager::new()?;
    let secret_payload = credentials.read_secret(PIHOLE_WEB_PASSWORD_CREDENTIAL)?;
    let tailscale_auth_key = credentials.read_secret(TAILSCALE_AUTH_KEY_CREDENTIAL)?;

    // 2. Provision the Podman secrets
    system.ensure_podman_secret(PIHOLE_WEB_PASSWORD_CREDENTIAL, &secret_payload)?;
    system.ensure_podman_secret(TAILSCALE_AUTH_KEY_CREDENTIAL, &tailscale_auth_key)?;

    let hostname = files
        .read_file(std::path::Path::new("/etc/hostname"))?
        .map(|contents| {
            String::from_utf8(contents).map_err(|error| ApplyError::FixtureInput(error.to_string()))
        })
        .transpose()?
        .unwrap_or_else(|| "clamps".to_string());
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

    // Tailscale uses host networking so its interface and routes belong to
    // the OS. The one-use auth key bootstraps login; state persists separately.
    skillet_podman::container(
        system,
        files,
        clamps_tailscale_config(hostname, tailscale_auth_key),
    )?;

    // 3. Look up pihole user and group IDs; None if the user/group
    // doesn't exist yet (ensure_user/group will assign dynamic IDs).
    // Passing the looked-up IDs through (rather than None) keeps the
    // secret file ownership and the user record consistent when the
    // account already exists, e.g. pre-created by Ignition.
    let pihole_uid = user_lookup::lookup_uid("pihole");
    let pihole_gid = user_lookup::lookup_gid("pihole");

    // 4. Apply pihole with the secret
    let secrets = vec![QuadletSecret {
        secret_name: PIHOLE_WEB_PASSWORD_CREDENTIAL.to_string(),
        target: SecretTarget::File {
            target_path: "/run/secrets/pihole_web_password".to_string(),
            mode: Some("0400".to_string()),
            uid: None,
            gid: None,
        },
    }];

    let custom_records: BTreeMap<String, String> = CLAMPS_CUSTOM_DNS_RECORDS
        .iter()
        .map(|(ip, fqdn)| ((*ip).to_string(), (*fqdn).to_string()))
        .collect();

    skillet_pihole::apply(
        system,
        files,
        &skillet_pihole::PiholeUser {
            uid: pihole_uid,
            gid: pihole_gid,
            name: "pihole".to_string(),
            group_name: "pihole".to_string(),
        },
        secrets,
        custom_records,
        host_service_network("clamps"),
    )?;

    apply_syncthing(system, files, host_service_network("clamps"))?;
    skillet_unifi::apply(system, files)?;
    skillet_btrbk::apply(
        system,
        files,
        &skillet_btrbk::BtrbkConfig {
            snapshot_subvolumes: vec![std::path::PathBuf::from("syncthing")],
        },
    )?;
    Ok(())
}

fn apply_syncthing(
    system: &dyn SystemResource,
    files: &dyn FileResource,
    network: PodmanNetwork,
) -> Result<(), ApplyError> {
    skillet_syncthing::apply(
        system,
        files,
        skillet_syncthing::SyncthingConfig {
            data_path: "/var/lib/data/syncthing".to_string(),
            data_owner: "giacomo".to_string(),
            data_group: "giacomo".to_string(),
            uid: 1000,
            gid: 1000,
            network,
        },
    )?;
    Ok(())
}

/// Dispatch to the canonical apply function for a hostname.
///
/// Unknown hostnames fall back to the hardening-only baseline, matching the
/// previous "(Agent Mode)" default behaviour.
pub fn apply_host(
    hostname: &str,
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), ApplyError> {
    match hostname {
        "clamps" => apply_clamps(system, files),
        "skillet-smoke" => fixture::apply(system, files),
        "beezelbot" => apply_beezelbot(system, files),
        _ => apply_full_base(system, files),
    }
}

#[path = "hosts/fixture.rs"]
mod fixture;

#[cfg(test)]
#[path = "hosts_tests.rs"]
mod tests;

pub fn apply_host_phase(
    hostname: &str,
    phase: ApplyPhase,
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), ApplyError> {
    match phase {
        ApplyPhase::Base => apply_base(system, files),
        ApplyPhase::Full => apply_host(hostname, system, files),
        ApplyPhase::Caddy => apply_caddy_host(hostname, system, files),
    }
}

fn apply_caddy_host(
    hostname: &str,
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), ApplyError> {
    let ui_config = ui_config_for_host(hostname).ok_or_else(|| {
        ApplyError::FixtureInput(format!("host {hostname} declares no UI services"))
    })?;
    files.require_btrfs_subvolume_mount(
        std::path::Path::new("/var/lib/data"),
        std::path::Path::new("/var"),
        "/data",
    )?;
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
