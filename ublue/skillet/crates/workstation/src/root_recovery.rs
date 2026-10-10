//! Portable LUKS recovery enrollment with write-ahead vault persistence.
use crate::vault::{Vault, VaultError};
use skillet_vm::transport::{GuestCommand, GuestTransport};
use std::{io::Read as _, path::Path};
use thiserror::Error;
use zeroize::Zeroizing;

#[derive(Debug, Error)]
pub enum RecoveryError {
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error(transparent)]
    Guest(#[from] skillet_vm::Error),
    #[error("root recovery: {0}")]
    Invalid(&'static str),
    #[error("recovery-key randomness is unavailable: {0}")]
    Randomness(#[from] std::io::Error),
}

pub struct Record {
    pub key: Zeroizing<String>,
    pub volume: String,
    pub verified: bool,
}

/// Cohesive persistence of a key and its enrollment/recovery identity.
pub trait RecoveryStore {
    fn load(&self, path: &str) -> Result<Option<Record>, RecoveryError>;
    fn save(&mut self, path: &str, record: &Record) -> Result<(), RecoveryError>;
}

pub struct VaultStore<'a> {
    pub vault: &'a mut Vault,
    pub key_file: Option<&'a Path>,
}
impl RecoveryStore for VaultStore<'_> {
    fn load(&self, path: &str) -> Result<Option<Record>, RecoveryError> {
        let Some(key) = self.vault.get(path)? else {
            return Ok(None);
        };
        let volume = self
            .vault
            .get_field(path, "luks-uuid")?
            .ok_or(RecoveryError::Invalid(
                "entry lacks luks-uuid; refusing to replace it",
            ))?;
        let state = self
            .vault
            .get_field(path, "recovery-state")?
            .ok_or(RecoveryError::Invalid("entry lacks recovery-state"))?;
        if !matches!(state.as_str(), "pending" | "verified") {
            return Err(RecoveryError::Invalid("invalid recovery-state"));
        }
        Ok(Some(Record {
            key: Zeroizing::new(key),
            volume,
            verified: state == "verified",
        }))
    }
    fn save(&mut self, path: &str, record: &Record) -> Result<(), RecoveryError> {
        self.vault.ensure_unchanged()?;
        if self.vault.get(path)?.is_none() {
            self.vault.insert(path, &record.key)?;
        }
        self.vault.set_field(path, "luks-uuid", &record.volume)?;
        self.vault.set_field(
            path,
            "recovery-state",
            if record.verified {
                "verified"
            } else {
                "pending"
            },
        )?;
        self.vault.save_verified(self.key_file, path, &record.key)?;
        Ok(())
    }
}

pub fn generate_key() -> Result<String, RecoveryError> {
    let mut bytes = Zeroizing::new([0u8; 64]);
    std::fs::File::open("/dev/urandom")?.read_exact(&mut *bytes)?;
    Ok(hex::encode(bytes.as_slice()))
}

/// Persist before enrollment; interrupted pending enrollment safely reuses its key.
pub fn provision(
    host: &str,
    path: &str,
    helper: &Path,
    store: &mut impl RecoveryStore,
    guest: &impl GuestTransport,
    generate: impl FnOnce() -> Result<String, RecoveryError>,
) -> Result<(), RecoveryError> {
    if text(guest, "cat", &["/etc/skillet/host"])? != host {
        return Err(RecoveryError::Invalid(
            "SSH host identity differs from selected host",
        ));
    }
    let status = text(guest, "sudo", &["-n", "cryptsetup", "status", "root"])?;
    let source = text(guest, "findmnt", &["-T", "/var", "-n", "-o", "SOURCE"])?;
    if !status.contains("LUKS2")
        || !(source == "/dev/mapper/root" || source.starts_with("/dev/mapper/root["))
    {
        return Err(RecoveryError::Invalid("requires mapper-backed LUKS2 root"));
    }
    let backing = status
        .lines()
        .find_map(|line| line.trim().strip_prefix("device:").map(str::trim))
        .ok_or(RecoveryError::Invalid("root backing device is absent"))?;
    if text(guest, "readlink", &["-f", backing])?
        != text(guest, "readlink", &["-f", "/dev/disk/by-partlabel/root"])?
    {
        return Err(RecoveryError::Invalid(
            "root partition label does not identify the active root",
        ));
    }
    let volume = text(
        guest,
        "sudo",
        &[
            "-n",
            "cryptsetup",
            "luksUUID",
            "/dev/disk/by-partlabel/root",
        ],
    )?;
    if volume.len() != 36
        || !volume.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
    {
        return Err(RecoveryError::Invalid("invalid LUKS volume identity"));
    }
    let existing = store.load(path)?;
    let created = existing.is_none();
    let mut record = match existing {
        Some(record) => record,
        None => Record {
            key: Zeroizing::new(generate()?),
            volume: volume.clone(),
            verified: false,
        },
    };
    if record.volume != volume {
        return Err(RecoveryError::Invalid(
            "vault recovery key belongs to another volume; refusing replacement",
        ));
    }
    if record.key.len() != 128
        || !record
            .key
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(RecoveryError::Invalid(
            "recovery key must be 128 lowercase hex characters",
        ));
    }
    if created || !record.verified {
        store.save(path, &record)?;
    }
    let remote = "/var/tmp/skillet-root-recovery.sh";
    guest.upload(helper, remote)?;
    if !record.verified {
        run_key(guest, remote, "enroll", &record.key)?;
    }
    run_key(guest, remote, "verify", &record.key)?;
    if !record.verified {
        record.verified = true;
        store.save(path, &record)?;
    }
    Ok(())
}

fn text(
    guest: &impl GuestTransport,
    program: &str,
    arguments: &[&str],
) -> Result<String, RecoveryError> {
    let out = guest.execute(&GuestCommand { program, arguments }, None)?;
    if !out.status.success() {
        return Err(RecoveryError::Invalid("guest preflight failed"));
    }
    String::from_utf8(out.stdout)
        .map(|s| s.trim().to_owned())
        .map_err(|_| RecoveryError::Invalid("invalid guest preflight output"))
}
fn run_key(
    guest: &impl GuestTransport,
    script: &str,
    action: &str,
    key: &str,
) -> Result<(), RecoveryError> {
    let out = guest.execute(
        &GuestCommand {
            program: "sudo",
            arguments: &["-n", "bash", script, action],
        },
        Some(key.as_bytes()),
    )?;
    if !out.status.success() {
        return Err(RecoveryError::Invalid(
            "guest enrollment/verification failed; key remains in vault for retry",
        ));
    }
    Ok(())
}
#[cfg(test)]
#[path = "root_recovery/tests.rs"]
mod tests;
