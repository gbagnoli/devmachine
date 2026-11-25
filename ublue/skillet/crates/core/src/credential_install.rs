//! Guest-side inspection and atomic installation of encrypted credentials.

use std::{
    fs,
    io::{self, Read as _, Write as _},
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
    let _maintenance = crate::maintenance::writer()?;
    let path = credential_path(name)?;
    if !valid_unit(unit) {
        return Err(CredentialInstallError::InvalidName);
    }
    let directory = prepare_directory()?;
    encrypt(name, &directory, None)?
        .persist(path)
        .map_err(|error| error.error)?;
    fs::File::open(&directory)?.sync_all()?;
    if start_unit {
        start(unit)?;
    }
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(transparent)]
struct CredentialBatch(Vec<(String, Vec<u8>)>);

impl Drop for CredentialBatch {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        for (_, value) in &mut self.0 {
            value.zeroize();
        }
    }
}

pub fn install_batch(unit: &str, start_unit: bool) -> Result<(), CredentialInstallError> {
    const LIMIT: u64 = 8 * 1024 * 1024;
    if !valid_unit(unit) {
        return Err(CredentialInstallError::InvalidName);
    }
    let mut input = zeroize::Zeroizing::new(Vec::new());
    io::stdin().take(LIMIT + 1).read_to_end(&mut input)?;
    if input.len() as u64 > LIMIT {
        return Err(CredentialInstallError::Command("credential batch size"));
    }
    let credentials: CredentialBatch = serde_json::from_slice(&input)
        .map_err(|_| CredentialInstallError::Command("credential batch decoding"))?;
    let mut names = std::collections::BTreeSet::new();
    if credentials.0.is_empty()
        || credentials.0.iter().any(|(name, value)| {
            !names.insert(name) || value.is_empty() || credential_path(name).is_err()
        })
    {
        return Err(CredentialInstallError::InvalidName);
    }
    let _maintenance = crate::maintenance::writer()?;
    let directory = prepare_directory()?;
    let mut updates = Vec::with_capacity(credentials.0.len());
    for (name, bytes) in &credentials.0 {
        let path = credential_path(name)?;
        if unchanged(name, &path, bytes)? {
            continue;
        }
        updates.push((path, encrypt(name, &directory, Some(bytes))?));
    }
    // Verify every encrypted file before making any replacement visible.
    for (path, encrypted) in updates {
        encrypted.persist(path).map_err(|error| error.error)?;
    }
    fs::File::open(&directory)?.sync_all()?;
    if start_unit {
        start(unit)?;
    }
    Ok(())
}

fn unchanged(
    name: &str,
    path: &std::path::Path,
    value: &[u8],
) -> Result<bool, CredentialInstallError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => return Err(CredentialInstallError::InvalidName),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    }
    let output = Command::new("systemd-creds")
        .arg("decrypt")
        .arg(format!("--name={name}"))
        .arg(path)
        .arg("-")
        .stderr(Stdio::null())
        .output()?;
    let current = zeroize::Zeroizing::new(output.stdout);
    Ok(output.status.success() && current.as_slice() == value)
}

fn prepare_directory() -> Result<PathBuf, CredentialInstallError> {
    let directory = PathBuf::from(CREDENTIAL_DIRECTORY);
    fs::create_dir_all(&directory)?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    Ok(directory)
}

fn encrypt(
    name: &str,
    directory: &std::path::Path,
    bytes: Option<&[u8]>,
) -> Result<tempfile::NamedTempFile, CredentialInstallError> {
    let encrypted = tempfile::NamedTempFile::new_in(directory)?;
    let mut child = Command::new("systemd-creds")
        .args(["encrypt", "--with-key=host"])
        .arg(format!("--name={name}"))
        .args(["-", "-"])
        .stdin(if bytes.is_some() {
            Stdio::piped()
        } else {
            Stdio::inherit()
        })
        .stdout(Stdio::from(encrypted.reopen()?))
        .spawn()?;
    let write = if let Some(bytes) = bytes {
        child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("missing credential pipe"))?
            .write_all(bytes)
    } else {
        Ok(())
    };
    let status = child.wait()?;
    write?;
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
    Ok(encrypted)
}

fn start(unit: &str) -> Result<(), CredentialInstallError> {
    let status = Command::new("systemctl").args(["start", unit]).status()?;
    if !status.success() {
        return Err(CredentialInstallError::Command("systemctl start"));
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
