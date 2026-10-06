//! Durable ownership records for disposable provider resources.

use crate::tailscale::DeviceRecord;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, Permissions},
    io,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;
use thiserror::Error;

const PRIVATE_FILE_MODE: u32 = 0o600;

#[derive(Debug, Error)]
pub enum ProvisioningStateError {
    #[error("provisioning state I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("provisioning state JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("refusing invalid provisioning state path: {0}")]
    InvalidPath(&'static str),
    #[error("could not atomically persist provisioning state at {path}: {source}")]
    Persist { path: PathBuf, source: io::Error },
    #[error("pending Tailscale identity does not match the recorded VM")]
    PendingIdentityMismatch,
    #[error(
        "provider ownership identity does not match the requested host, environment, and instance"
    )]
    IdentityMismatch,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProvisioningIdentity {
    pub host: String,
    pub environment: String,
    pub instance: String,
}

impl ProvisioningIdentity {
    pub fn new(
        host: impl Into<String>,
        environment: impl Into<String>,
        instance: impl Into<String>,
    ) -> Self {
        Self {
            host: host.into(),
            environment: environment.into(),
            instance: instance.into(),
        }
    }
}

/// External DNS/token ownership needed to recover or clean up a test VM.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CloudflareVmOwnership {
    /// Present in newly written journals; absent only in the legacy JSON shape.
    #[serde(default)]
    pub identity: Option<ProvisioningIdentity>,
    pub environment: String,
    pub marker: String,
    pub zone_id: String,
    pub ui_domain: String,
    pub token_name: String,
    pub token_id: Option<String>,
    pub expires_on: Option<String>,
    pub record_ids: Vec<String>,
}

/// Ownership for public DDNS records created by one disposable VM.
/// The configured names are private; this file remains mode 0600 under runs/.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DdnsVmOwnership {
    pub identity: ProvisioningIdentity,
    pub zone_id: String,
    pub marker: String,
    pub token_name: String,
    pub token_id: Option<String>,
    pub expires_on: Option<String>,
    pub record_names: Vec<String>,
    #[serde(default)]
    pub record_ids: Vec<String>,
    pub cleanup_token_name: String,
    pub cleanup_token_id: Option<String>,
}

pub fn save_ddns_ownership(
    path: &Path,
    ownership: &DdnsVmOwnership,
) -> Result<(), ProvisioningStateError> {
    write_json_atomically(path, ownership)
}

pub fn load_ddns_ownership(path: &Path) -> Result<DdnsVmOwnership, ProvisioningStateError> {
    read_json(path)
}

pub fn ddns_ownership_exists(path: &Path) -> Result<bool, ProvisioningStateError> {
    regular_file_exists(path, "DDNS ownership state must be a regular file")
}

pub fn remove_ddns_ownership(path: &Path) -> Result<(), ProvisioningStateError> {
    remove_regular_file(path, "DDNS ownership state must be a regular file")
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct TailscaleVmOwnership {
    identity: ProvisioningIdentity,
    device: DeviceRecord,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct PendingTailscaleIdentity {
    identity: ProvisioningIdentity,
    hostname: String,
}

pub fn save_cloudflare_ownership(
    path: &Path,
    ownership: &CloudflareVmOwnership,
) -> Result<(), ProvisioningStateError> {
    write_json_atomically(path, ownership)
}

pub fn load_cloudflare_ownership(
    path: &Path,
) -> Result<CloudflareVmOwnership, ProvisioningStateError> {
    read_json(path)
}

pub fn cloudflare_ownership_exists(path: &Path) -> Result<bool, ProvisioningStateError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(ProvisioningStateError::InvalidPath(
                "Cloudflare ownership state must be a regular file",
            ))
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub fn remove_cloudflare_ownership(path: &Path) -> Result<(), ProvisioningStateError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(ProvisioningStateError::InvalidPath(
                "Cloudflare ownership state must be a regular file",
            ))
        }
        Ok(_) => {
            fs::remove_file(path)?;
            if let Some(parent) = path.parent() {
                File::open(parent)?.sync_all()?;
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Confirm that a Cloudflare journal belongs to the requested typed identity.
/// Legacy journals are still validated by their exact marker and token name at
/// the call site; new journals must match every identity field here.
pub fn validate_cloudflare_identity(
    ownership: &CloudflareVmOwnership,
    expected: &ProvisioningIdentity,
) -> Result<(), ProvisioningStateError> {
    if ownership.environment != expected.environment
        || ownership
            .identity
            .as_ref()
            .is_some_and(|identity| identity != expected)
    {
        return Err(ProvisioningStateError::IdentityMismatch);
    }
    Ok(())
}

pub fn save_tailscale_record(
    run_directory: &Path,
    identity: &ProvisioningIdentity,
    record: &DeviceRecord,
) -> Result<(), ProvisioningStateError> {
    write_json_atomically(
        &run_directory.join("tailscale.json"),
        &TailscaleVmOwnership {
            identity: identity.clone(),
            device: record.clone(),
        },
    )
}

pub fn load_tailscale_record(
    path: &Path,
    expected_identity: &ProvisioningIdentity,
    expected_hostname: &str,
) -> Result<DeviceRecord, ProvisioningStateError> {
    let bytes = read_bytes(path)?;
    if let Ok(state) = serde_json::from_slice::<TailscaleVmOwnership>(&bytes) {
        if &state.identity != expected_identity || state.device.hostname != expected_hostname {
            return Err(ProvisioningStateError::IdentityMismatch);
        }
        return Ok(state.device);
    }
    let legacy: DeviceRecord = serde_json::from_slice(&bytes)?;
    if legacy.hostname != expected_hostname {
        return Err(ProvisioningStateError::IdentityMismatch);
    }
    Ok(legacy)
}

pub fn tailscale_record_exists(run_directory: &Path) -> Result<bool, ProvisioningStateError> {
    regular_file_exists(
        &run_directory.join("tailscale.json"),
        "Tailscale ownership state",
    )
}

pub fn remove_tailscale_record(run_directory: &Path) -> Result<(), ProvisioningStateError> {
    remove_regular_file(
        &run_directory.join("tailscale.json"),
        "Tailscale ownership state",
    )
}

pub fn tailscale_pending_exists(run_directory: &Path) -> Result<bool, ProvisioningStateError> {
    regular_file_exists(
        &run_directory.join("tailscale-pending"),
        "pending Tailscale state",
    )
}

pub fn validate_tailscale_pending(
    run_directory: &Path,
    expected_identity: &ProvisioningIdentity,
    expected_hostname: &str,
) -> Result<bool, ProvisioningStateError> {
    let path = run_directory.join("tailscale-pending");
    if !regular_file_exists(&path, "pending Tailscale state")? {
        return Ok(false);
    }
    let contents = fs::read(path)?;
    if let Ok(pending) = serde_json::from_slice::<PendingTailscaleIdentity>(&contents) {
        if pending.identity != *expected_identity || pending.hostname != expected_hostname {
            return Err(ProvisioningStateError::IdentityMismatch);
        }
    } else if String::from_utf8_lossy(&contents) != expected_hostname {
        return Err(ProvisioningStateError::IdentityMismatch);
    }
    Ok(true)
}

/// Persist cleanup intent before creating a disposable Tailscale device.
/// Repeated calls must describe the same VM identity.
pub fn mark_tailscale_pending(
    run_directory: &Path,
    identity: &ProvisioningIdentity,
    hostname: &str,
) -> Result<(), ProvisioningStateError> {
    let path = run_directory.join("tailscale-pending");
    ensure_parent(&path)?;
    match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(ProvisioningStateError::InvalidPath(
                    "pending Tailscale state must be a regular file",
                ));
            }
            let contents = fs::read(&path)?;
            if let Ok(pending) = serde_json::from_slice::<PendingTailscaleIdentity>(&contents) {
                if pending.identity == *identity && pending.hostname == hostname {
                    return Ok(());
                }
                return Err(ProvisioningStateError::PendingIdentityMismatch);
            }
            if String::from_utf8_lossy(&contents) == hostname {
                // Legacy markers only contain the deterministic VM hostname.
                // The caller also validates host/environment/instance from
                // the VM manifest before acting on this retained run.
                return Ok(());
            }
            return Err(ProvisioningStateError::PendingIdentityMismatch);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let mut file = NamedTempFile::new_in(run_directory)?;
    serde_json::to_writer(
        file.as_file_mut(),
        &PendingTailscaleIdentity {
            identity: identity.clone(),
            hostname: hostname.to_string(),
        },
    )?;
    file.as_file()
        .set_permissions(Permissions::from_mode(PRIVATE_FILE_MODE))?;
    file.as_file().sync_all()?;
    match file.persist_noclobber(&path) {
        Ok(_) => {}
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(ProvisioningStateError::PendingIdentityMismatch);
            }
            let contents = fs::read(&path)?;
            let matches_new = serde_json::from_slice::<PendingTailscaleIdentity>(&contents)
                .is_ok_and(|pending| pending.identity == *identity && pending.hostname == hostname);
            let matches_legacy = String::from_utf8_lossy(&contents) == hostname;
            if !matches_new && !matches_legacy {
                return Err(ProvisioningStateError::PendingIdentityMismatch);
            }
        }
        Err(error) => {
            return Err(ProvisioningStateError::Persist {
                path,
                source: error.error,
            });
        }
    }
    File::open(run_directory)?.sync_all()?;
    Ok(())
}

pub fn remove_tailscale_pending(run_directory: &Path) -> Result<(), ProvisioningStateError> {
    let path = run_directory.join("tailscale-pending");
    remove_regular_file(&path, "pending Tailscale state")
}

fn regular_file_exists(path: &Path, label: &'static str) -> Result<bool, ProvisioningStateError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(ProvisioningStateError::InvalidPath(label))
        }
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn remove_regular_file(path: &Path, label: &'static str) -> Result<(), ProvisioningStateError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(ProvisioningStateError::InvalidPath(label))
        }
        Ok(_) => {
            fs::remove_file(path)?;
            if let Some(parent) = path.parent() {
                File::open(parent)?.sync_all()?;
            }
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn write_json_atomically<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), ProvisioningStateError> {
    let parent = path.parent().ok_or(ProvisioningStateError::InvalidPath(
        "metadata file has no parent",
    ))?;
    ensure_parent(path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(ProvisioningStateError::InvalidPath(
                "metadata destination must be a regular file",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut file = NamedTempFile::new_in(parent)?;
    serde_json::to_writer(file.as_file_mut(), value)?;
    file.as_file()
        .set_permissions(Permissions::from_mode(PRIVATE_FILE_MODE))?;
    file.as_file().sync_all()?;
    file.persist(path)
        .map_err(|error| ProvisioningStateError::Persist {
            path: path.to_path_buf(),
            source: error.error,
        })?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, ProvisioningStateError> {
    Ok(serde_json::from_slice(&read_bytes(path)?)?)
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, ProvisioningStateError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ProvisioningStateError::InvalidPath(
            "metadata source must be a regular file",
        ));
    }
    Ok(fs::read(path)?)
}

fn ensure_parent(path: &Path) -> Result<(), ProvisioningStateError> {
    let parent = path.parent().ok_or(ProvisioningStateError::InvalidPath(
        "metadata file has no parent",
    ))?;
    let metadata = fs::symlink_metadata(parent)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ProvisioningStateError::InvalidPath(
            "metadata parent must be a real directory",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "provisioning_state/tests.rs"]
mod tests;
