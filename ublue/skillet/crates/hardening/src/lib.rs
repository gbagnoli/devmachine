use skillet_core::files::{FileError, FileResource};
use skillet_core::system::{SystemError, SystemResource};
use std::path::Path;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum HardeningError {
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("File error: {0}")]
    File(#[from] FileError),
}

pub fn apply<S, F>(system: &S, files: &F) -> Result<(), HardeningError>
where
    S: SystemResource + ?Sized,
    F: FileResource + ?Sized,
{
    info!("Applying hardening...");

    // 1. Sysctl hardening
    apply_sysctl_hardening(files)?;

    // 2. Include 'os-hardening'
    apply_os_hardening(system)?;

    // 3. Include 'ssh-hardening::server'
    apply_ssh_hardening_server(system)?;

    // 4. Include 'ssh-hardening::client'
    apply_ssh_hardening_client(system)?;

    Ok(())
}

fn apply_sysctl_hardening<F: FileResource + ?Sized>(files: &F) -> Result<(), HardeningError> {
    info!("Applying sysctl hardening...");
    let content = include_bytes!("../files/sysctl.boxy.conf");
    let path = Path::new("/etc/sysctl.d/99-hardening.conf");

    files.ensure_file(path, content, Some(0o644), Some("root"), Some("root"))?;

    Ok(())
}

fn apply_os_hardening<S: SystemResource + ?Sized>(_system: &S) -> Result<(), HardeningError> {
    info!("(Placeholder) Applying os-hardening");
    Ok(())
}

fn apply_ssh_hardening_server<S: SystemResource + ?Sized>(
    _system: &S,
) -> Result<(), HardeningError> {
    info!("(Placeholder) Applying ssh-hardening::server");
    Ok(())
}

fn apply_ssh_hardening_client<S: SystemResource + ?Sized>(
    _system: &S,
) -> Result<(), HardeningError> {
    info!("(Placeholder) Applying ssh-hardening::client");
    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
