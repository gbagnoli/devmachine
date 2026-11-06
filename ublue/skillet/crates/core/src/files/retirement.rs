//! Recoverable retirement of verified legacy definitions.
use super::FileError;
use std::{fs, io, io::Read, os::unix::fs::OpenOptionsExt, path::Path};

pub(super) fn regular(path: &Path) -> Result<Option<Vec<u8>>, FileError> {
    let mut file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(FileError::Read(path.display().to_string(), error)),
    };
    if !file.metadata()?.is_file() {
        return Err(FileError::Retirement(format!(
            "not a regular file: {}",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

fn rename(source: &Path, destination: &Path) -> Result<(), FileError> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        source,
        rustix::fs::CWD,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(|error| FileError::Io(error.into()))
}

pub(super) fn retire(path: &Path, expected: &[u8], archive: &Path) -> Result<bool, FileError> {
    if path == archive || path.parent() != archive.parent() {
        return Err(FileError::Retirement(
            "archive must be a distinct sibling".into(),
        ));
    }
    let backup = regular(archive)?;
    if backup.as_deref().is_some_and(|bytes| bytes != expected) {
        return Err(FileError::Retirement(format!(
            "unknown archive: {}",
            archive.display()
        )));
    }
    let Some(bytes) = regular(path)? else {
        return Ok(false);
    };
    if bytes != expected {
        return Err(FileError::Retirement(format!(
            "unknown content: {}",
            path.display()
        )));
    }
    let parent = path
        .parent()
        .ok_or_else(|| FileError::Retirement("missing parent".into()))?;
    // Keep quarantine on every failure, including process death. A TempDir drop
    // must never delete a file changed concurrently by an administrator.
    let quarantine = if backup.is_some() {
        Some(
            tempfile::Builder::new()
                .prefix(".skillet-retired-")
                .tempdir_in(parent)?
                .keep(),
        )
    } else {
        None
    };
    let destination = quarantine
        .as_ref()
        .map_or_else(|| archive.to_path_buf(), |dir| dir.join("retired"));
    rename(path, &destination)?;
    if regular(&destination).ok().flatten().as_deref() != Some(expected) {
        let preserved = if rename(&destination, path).is_ok() {
            path
        } else {
            &destination
        };
        return Err(FileError::Retirement(format!(
            "changed during retirement; preserved at {}",
            preserved.display()
        )));
    }
    // Retain duplicates too: an administrator may edit the relocated inode
    // after our check. No verified-retirement operation deletes recovery data.
    if let Some(directory) = quarantine {
        fs::File::open(directory)?.sync_all()?;
    }
    fs::File::open(parent)?.sync_all()?;
    Ok(true)
}

#[cfg(test)]
#[path = "retirement_tests.rs"]
mod tests;
