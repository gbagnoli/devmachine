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
use skillet_podman::{QuadletSecret, SecretTarget};
use thiserror::Error;

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
    skillet_hardening::apply(system, files).map_err(|e| ApplyError::Hardening(e.to_string()))
}

/// Apply the clamps host configuration (hardening + Pi-hole).
///
/// The credential manager is constructed lazily here: hosts that need no
/// secrets (e.g. beezelbot) never touch CREDENTIALS_DIRECTORY.
// pihole uid/gid lookups are intentionally parallel
#[allow(clippy::similar_names)]
pub fn apply_clamps(
    system: &dyn SystemResource,
    files: &dyn FileResource,
) -> Result<(), ApplyError> {
    skillet_hardening::apply(system, files).map_err(|e| ApplyError::Hardening(e.to_string()))?;

    // 1. Ingest secret from systemd (lazy: only this host needs secrets)
    let credentials = CredentialManager::new()?;
    let secret_payload = credentials.read_secret("test_secret")?;

    // 2. Provision to Podman
    system.ensure_podman_secret("pihole_web_password", &secret_payload)?;

    // Look up pihole user and group IDs; None if the user/group
    // doesn't exist yet (ensure_user/group will assign dynamic IDs).
    // Passing the looked-up IDs through (rather than None) keeps the
    // secret file ownership and the user record consistent when the
    // account already exists, e.g. pre-created by Ignition.
    let pihole_uid = user_lookup::lookup_uid("pihole");
    let pihole_gid = user_lookup::lookup_gid("pihole");

    // 3. Apply pihole with the secret
    let secrets = vec![QuadletSecret {
        secret_name: "pihole_web_password".to_string(),
        target: SecretTarget::File {
            target_path: "/etc/pihole/webpassword".to_string(),
            mode: Some("0400".to_string()),
            uid: pihole_uid,
            gid: pihole_gid,
        },
    }];

    skillet_pihole::apply(
        system,
        files,
        skillet_pihole::PiholeUser {
            uid: pihole_uid,
            gid: pihole_gid,
            name: "pihole".to_string(),
            group_name: "pihole".to_string(),
        },
        secrets,
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
        "beezelbot" => apply_beezelbot(system, files),
        "clamps" => apply_clamps(system, files),
        _ => apply_beezelbot(system, files),
    }
}
