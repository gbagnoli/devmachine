//! Guest-side inspection and atomic installation of encrypted credentials.

use std::{
    fs, io,
    os::unix::fs::PermissionsExt as _,
    path::PathBuf,
    process::{Command, Stdio},
};
use thiserror::Error;

const CREDENTIAL_DIRECTORY: &str = "/etc/credstore.encrypted/skillet";

#[derive(Debug, Error)]
pub enum CredentialInstallError {
    #[error("invalid credential name or service unit")]
    InvalidName,
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("{0} failed")]
    Command(&'static str),
}

fn credential_path(name: &str) -> Result<PathBuf, CredentialInstallError> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(CredentialInstallError::InvalidName);
    }
    Ok(PathBuf::from(CREDENTIAL_DIRECTORY).join(format!("{name}.cred")))
}

pub fn state(name: &str) -> Result<&'static str, CredentialInstallError> {
    let path = credential_path(name)?;
    match fs::symlink_metadata(path) {
        Ok(_) => return Ok("present"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let status = Command::new("podman")
        .args(["secret", "exists", name])
        .status()?;
    match status.code() {
        Some(0) => Ok("present"),
        Some(1) => Ok("absent"),
        _ => Err(CredentialInstallError::Command("podman secret exists")),
    }
}

pub fn install(name: &str, unit: &str, start_unit: bool) -> Result<(), CredentialInstallError> {
    let path = credential_path(name)?;
    if !valid_unit(unit) {
        return Err(CredentialInstallError::InvalidName);
    }
    let directory = PathBuf::from(CREDENTIAL_DIRECTORY);
    fs::create_dir_all(&directory)?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    let encrypted = tempfile::NamedTempFile::new_in(&directory)?;
    let status = Command::new("systemd-creds")
        .args(["encrypt", "--with-key=host"])
        .arg(format!("--name={name}"))
        .args(["-", "-"])
        .stdin(Stdio::inherit())
        .stdout(Stdio::from(encrypted.reopen()?))
        .status()?;
    if !status.success() {
        return Err(CredentialInstallError::Command("systemd-creds encrypt"));
    }
    encrypted.as_file().sync_all()?;
    let status = Command::new("systemd-creds")
        .arg("decrypt")
        .arg(format!("--name={name}"))
        .arg(encrypted.path())
        .arg("-")
        .stdout(Stdio::null())
        .status()?;
    if !status.success() {
        return Err(CredentialInstallError::Command("systemd-creds decrypt"));
    }
    encrypted.persist(path).map_err(|error| error.error)?;
    fs::File::open(&directory)?.sync_all()?;
    if start_unit {
        let status = Command::new("systemctl").args(["start", unit]).status()?;
        if !status.success() {
            return Err(CredentialInstallError::Command("systemctl start"));
        }
    }
    Ok(())
}

fn valid_unit(unit: &str) -> bool {
    unit.ends_with(".service")
        && unit
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && unit
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'@' | b'.'))
}

#[cfg(test)]
#[path = "credential_install/tests.rs"]
mod tests;
