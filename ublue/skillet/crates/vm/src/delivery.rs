//! Captured/current binaries share one delivery and verification operation.
use crate::{
    manifest::{reject_symlinks, ArtifactHashes},
    transport::{GuestCommand, GuestTransport},
    Error, Phase, Result, VmRun,
};
use sha2::{Digest as _, Sha256};
use std::{fs::File, io::Read as _, path::Path};

pub fn deliver(
    run: &VmRun,
    transport: &impl GuestTransport,
    host_artifact: &Path,
    generic_artifact: &Path,
) -> Result<ArtifactHashes> {
    if !matches!(run.phase, Phase::Started | Phase::Ready) {
        return Err(Error::Invalid(
            "VM is not available for binary delivery; finish creation or disposal first".into(),
        ));
    }
    let hashes = ArtifactHashes {
        host: sha256(host_artifact)?,
        generic: sha256(generic_artifact)?,
    };
    let host_staging = format!("/var/tmp/skillet-{}-update", run.identity.host());
    let generic_staging = "/var/tmp/skillet-generic-update";
    let host_destination = format!("/var/usrlocal/bin/skillet-{}", run.identity.host());
    let generic_destination = "/var/usrlocal/bin/skillet";
    for (artifact, source, destination, expected) in [
        (
            host_artifact,
            &host_staging[..],
            &host_destination[..],
            &hashes.host[..],
        ),
        (
            generic_artifact,
            generic_staging,
            generic_destination,
            &hashes.generic[..],
        ),
    ] {
        if !matches_installed(transport, destination, expected)? {
            transport.upload(artifact, source)?;
            checked(
                transport,
                "sudo",
                &["-n", "install", "-m", "0755", source, destination],
            )?;
        }
    }
    for (path, expected) in [
        (&host_destination[..], &hashes.host[..]),
        (generic_destination, &hashes.generic[..]),
    ] {
        if !matches_installed(transport, path, expected)? {
            return Err(Error::Invalid(
                "guest artifact bytes or metadata differ from the selected binary".into(),
            ));
        }
    }
    checked(transport, "rm", &["-f", &host_staging, generic_staging])?;
    Ok(hashes)
}

pub(crate) fn matches_installed(
    transport: &impl GuestTransport,
    path: &str,
    expected: &str,
) -> Result<bool> {
    if test_path(transport, "-L", path)? {
        return Err(Error::Invalid("installed binary is a symlink".into()));
    }
    if !test_path(transport, "-e", path)? {
        return Ok(false);
    }
    if !test_path(transport, "-f", path)? {
        return Err(Error::Invalid(
            "installed binary is not a regular file".into(),
        ));
    }
    let hash = checked(transport, "sha256sum", &[path])?;
    let metadata = checked(transport, "stat", &["--format=%a:%u:%g", path])?;
    Ok(std::str::from_utf8(&hash)
        .ok()
        .and_then(|value| value.split_whitespace().next())
        == Some(expected)
        && metadata == b"755:0:0\n")
}

fn test_path(transport: &impl GuestTransport, condition: &str, path: &str) -> Result<bool> {
    let output = transport.execute(
        &GuestCommand {
            program: "test",
            arguments: &[condition, path],
        },
        None,
    )?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        code => Err(Error::Guest {
            operation: "inspect installed binary".into(),
            code,
        }),
    }
}

pub fn sha256(path: &Path) -> Result<String> {
    reject_symlinks(path)?;
    if !std::fs::metadata(path)?.is_file() {
        return Err(Error::Invalid("artifact is not a regular file".into()));
    }
    let mut file = File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::Invalid("artifact is not a regular file".into()));
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(hex::encode(digest.finalize()))
}

pub(crate) fn checked(
    transport: &impl GuestTransport,
    program: &str,
    arguments: &[&str],
) -> Result<Vec<u8>> {
    let output = transport.execute(&GuestCommand { program, arguments }, None)?;
    if !output.status.success() {
        return Err(Error::Guest {
            operation: program.into(),
            code: output.status.code(),
        });
    }
    Ok(output.stdout)
}

#[cfg(test)]
#[path = "delivery/tests.rs"]
mod tests;
