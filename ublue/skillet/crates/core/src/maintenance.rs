//! Cooperative exclusion for guest configuration and OS activation.
use nix::fcntl::{Flock, FlockArg};
use std::{fs, io, os::unix::fs::OpenOptionsExt as _, path::Path};

pub const ROOT: &str = "/run/skillet-maintenance";

pub fn writer() -> io::Result<Flock<fs::File>> {
    lock(Path::new(ROOT), false)
}

pub fn activation() -> io::Result<Flock<fs::File>> {
    lock(Path::new(ROOT), true)
}

fn lock(root: &Path, exclusive: bool) -> io::Result<Flock<fs::File>> {
    fs::create_dir_all(root)?;
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(root.join("lock"))?;
    let mode = if exclusive {
        FlockArg::LockExclusiveNonblock
    } else {
        FlockArg::LockSharedNonblock
    };
    let guard = Flock::lock(file, mode).map_err(|(_, error)| {
        io::Error::other(format!("configuration/activation busy: {error}"))
    })?;
    if !exclusive && root.join("finalized").try_exists()? {
        return Err(io::Error::other(
            "OS activation finalized /etc; reboot before further configuration writes",
        ));
    }
    Ok(guard)
}

#[cfg(test)]
#[path = "maintenance/tests.rs"]
mod tests;
