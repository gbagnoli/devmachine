use crate::{backend::VmBackend, Error, ManifestStore, Phase, Result, RunIdentity, VmRun};
use nix::fcntl::{Flock, FlockArg};
use std::{
    fs::{self, File, OpenOptions},
    os::unix::fs::OpenOptionsExt as _,
};

/// Dispose external resources, the UUID-verified domain, then local artifacts.
/// The guest need not exist; runtime errors and cleanup failures retain state.
pub fn destroy(
    store: &ManifestStore,
    identity: &RunIdentity,
    backend: &impl VmBackend,
    mut external_cleanup: impl FnMut(&VmRun) -> Result<()>,
) -> Result<()> {
    store.load(identity)?;
    let dir = store.run_dir(identity);
    let lock_path = dir.join(".vm.lock");
    crate::manifest::reject_symlinks(&lock_path)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(lock_path)?;
    let _lock = Flock::lock(file, FlockArg::LockExclusiveNonblock).map_err(|(_, error)| {
        if error == nix::errno::Errno::EWOULDBLOCK {
            Error::Busy
        } else {
            Error::Io(std::io::Error::from_raw_os_error(error as i32))
        }
    })?;
    let run = store.load(identity)?;
    if let Some(domain) = backend.inspect(&run)? {
        domain.validate_owned(&run)?;
    }
    let mut run = store.import(identity)?;
    if !matches!(
        run.phase,
        Phase::ExternalCleanupComplete | Phase::DomainRemoved
    ) {
        external_cleanup(&run)?;
        run.phase = Phase::ExternalCleanupComplete;
        store.save(&run)?;
    }
    // Recheck after potentially long provider calls. UUID addressing prevents a
    // name reused by another domain from being destroyed by a stale observation.
    if let Some(domain) = backend.inspect(&run)? {
        domain.validate_owned(&run)?;
        if domain.state != "shut off" {
            backend.stop(&run)?;
        }
        if let Some(domain) = backend.inspect(&run)? {
            domain.validate_owned(&run)?;
            backend.undefine(&run)?;
        }
    }
    if backend.inspect(&run)?.is_some() {
        return Err(Error::Invalid("domain still exists after disposal".into()));
    }
    run.phase = Phase::DomainRemoved;
    store.save(&run)?;
    // Validate once more before deleting only the owned run directory.
    store.load(identity)?;
    fs::remove_dir_all(&dir)?;
    if let Some(parent) = dir.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "lifecycle/tests.rs"]
mod tests;
