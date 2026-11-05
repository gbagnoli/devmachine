//! Converge the opt-in activation definition without requesting a reboot.
use skillet_core::{
    files::{FileError, FileMutationResource, FileReadResource, Ownership},
    system::{ServiceResource, SystemError},
};
use std::path::Path;

pub const UNIT: &str = "skillet-kexec-activate.service";
pub const PATH: &str = "/etc/systemd/system/skillet-kexec-activate.service";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error("activation executable must be an absolute path with safe unit characters")]
    InvalidExecutable,
}

pub fn render(binary: &str) -> Result<String, Error> {
    if !binary.starts_with('/')
        || !binary
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
    {
        return Err(Error::InvalidExecutable);
    }
    Ok(format!("[Unit]\nDescription=Controlled Skillet kexec activation\nDefaultDependencies=no\nAfter=local-fs.target\n[Service]\nType=exec\nPrivateMounts=yes\nExecStart={binary} kexec run\nExecStopPost={binary} kexec cleanup\nRuntimeMaxSec=120\nTimeoutStopSec=20\nRestart=no\n"))
}

pub fn apply<S, F>(system: &S, files: &F, binary: &str) -> Result<(), Error>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let content = render(binary)?;
    let owner = Ownership::named(Some("root"), Some("root"));
    files.ensure_directory(Path::new("/etc/systemd/system"), Some(0o755), &owner)?;
    let changed = files.ensure_file(Path::new(PATH), content.as_bytes(), Some(0o644), &owner)?;
    let state = Path::new("/var/lib/skillet/kexec/definition.applied");
    if changed || files.read_file(state)?.as_deref() != Some(content.as_bytes()) {
        system.daemon_reload()?;
        files.ensure_directory(Path::new("/var/lib/skillet/kexec"), Some(0o755), &owner)?;
        files.ensure_file(state, content.as_bytes(), Some(0o644), &owner)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "service/tests.rs"]
mod tests;
