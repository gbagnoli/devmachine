//! Retire a legacy Quadlet only when its completed Skillet receipt still matches.
use crate::FileBrowserError;
use sha2::{Digest as _, Sha256};
use skillet_core::{
    files::{FileMutationResource, FileReadResource, Ownership},
    system::ServiceResource,
};
use std::path::Path;

pub(super) fn retire<S, F>(system: &S, files: &F) -> Result<(), FileBrowserError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let path = Path::new("/etc/containers/systemd/mikochi.container");
    let archive = Path::new("/etc/containers/systemd/mikochi.container.skillet-legacy");
    let current = files.read_regular_file(path)?;
    let previous = files.read_regular_file(archive)?;
    let Some(expected) = current.as_ref().or(previous.as_ref()) else {
        return Ok(());
    };
    let hash = hex::encode(Sha256::digest(expected));
    // Legacy Mikochi used no additional config revisions or Podman secret IDs.
    if files
        .read_regular_file(Path::new("/var/lib/skillet/containers/mikochi.applied"))?
        .as_deref()
        != Some(hash.as_bytes())
        || previous.as_ref().is_some_and(|bytes| bytes != expected)
        || !String::from_utf8_lossy(expected).contains("ContainerName=mikochi\n")
    {
        return Err(FileBrowserError::Legacy);
    }
    let marker = Path::new("/var/lib/skillet/containers/mikochi.retired");
    let pending = files.read_regular_file(marker)?.as_deref() != Some(hash.as_bytes());
    if current.is_some() || pending {
        if system.service_is_active("mikochi.service")? {
            system.service_stop("mikochi.service")?;
        }
        files.retire_file(path, expected, archive)?;
        system.daemon_reload()?;
        files.ensure_file(
            marker,
            hash.as_bytes(),
            Some(0o644),
            &Ownership::named(Some("root"), Some("root")),
        )?;
    }
    Ok(())
}
