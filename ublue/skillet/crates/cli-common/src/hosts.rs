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
use skillet_podman::{PodmanNetwork, QuadletSecret, SecretTarget};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum ApplyPhase {
    Base,
    Full,
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

/// Apply the beezelbot host configuration (hardening baseline).
pub fn apply_beezelbot(
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), ApplyError> {
    apply_full_base(system, files)
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

/// Custom DNS records for the clamps Pi-hole (`ip -> fqdn`).
// TODO: replace with the real LAN IP and domain before the production
// cutover; these are still the old placeholder values.
const CLAMPS_CUSTOM_DNS_RECORDS: &[(&str, &str)] = &[("192.168.1.100", "my.custom.domain")];

fn clamps_service_network() -> PodmanNetwork {
    PodmanNetwork {
        unit_name: "clamps".to_string(),
        options: vec![
            "DisableDNS=false".to_string(),
            "Driver=bridge".to_string(),
            "Gateway=172.26.26.1".to_string(),
            "Gateway=fd59:4e23:2950:11f5::1".to_string(),
            "IPv6=true".to_string(),
            "NetworkName=clamps".to_string(),
            "Subnet=172.26.26.0/24".to_string(),
            "Subnet=fd59:4e23:2950:11f5::/64".to_string(),
        ],
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

    // 1. Ingest secret from systemd (lazy: only this host needs secrets)
    let credentials = CredentialManager::new()?;
    let secret_payload = credentials.read_secret(PIHOLE_WEB_PASSWORD_CREDENTIAL)?;

    // 2. Provision to Podman
    system.ensure_podman_secret(PIHOLE_WEB_PASSWORD_CREDENTIAL, &secret_payload)?;

    // Look up pihole user and group IDs; None if the user/group
    // doesn't exist yet (ensure_user/group will assign dynamic IDs).
    // Passing the looked-up IDs through (rather than None) keeps the
    // secret file ownership and the user record consistent when the
    // account already exists, e.g. pre-created by Ignition.
    let pihole_uid = user_lookup::lookup_uid("pihole");
    let pihole_gid = user_lookup::lookup_gid("pihole");

    // 3. Apply pihole with the secret
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
        clamps_service_network(),
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
        // beezelbot and unknown hostnames fall back to the hardening-only
        // baseline, matching the previous "(Agent Mode)" default behaviour.
        _ => apply_beezelbot(system, files),
    }
}

#[path = "hosts/fixture.rs"]
mod fixture;

pub fn apply_host_phase(
    hostname: &str,
    phase: ApplyPhase,
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), ApplyError> {
    match phase {
        ApplyPhase::Base => apply_base(system, files),
        ApplyPhase::Full => apply_host(hostname, system, files),
    }
}
