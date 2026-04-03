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
    apply_sysctl_hardening(system, files)?;

    // 2. Include 'os-hardening'
    apply_os_hardening(system);

    // 3. Include 'ssh-hardening::server'
    apply_ssh_hardening_server(system, files)?;

    // 4. Include 'ssh-hardening::client'
    apply_ssh_hardening_client(system, files)?;

    Ok(())
}

fn apply_sysctl_hardening<S, F>(system: &S, files: &F) -> Result<(), HardeningError>
where
    S: SystemResource + ?Sized,
    F: FileResource + ?Sized,
{
    info!("Applying sysctl hardening...");
    let content = include_bytes!("../files/sysctl.boxy.conf");
    let path = Path::new("/etc/sysctl.d/99-hardening.conf");

    let changed = files.ensure_file(path, content, Some(0o644), Some("root"), Some("root"))?;

    if changed {
        info!("Sysctl configuration changed, restarting systemd-sysctl...");
        system.service_restart("systemd-sysctl")?;
    }

    Ok(())
}

fn apply_os_hardening<S: SystemResource + ?Sized>(_system: &S) {
    info!("(Placeholder) Applying os-hardening");
}

fn apply_ssh_hardening_server<S, F>(system: &S, files: &F) -> Result<(), HardeningError>
where
    S: SystemResource + ?Sized,
    F: FileResource + ?Sized,
{
    info!("Applying ssh-hardening::server");
    let ssh_dir = Path::new("/etc/ssh");
    files.ensure_directory(ssh_dir, Some(0o755), Some("root"), Some("root"))?;

    let content = include_bytes!("../files/sshd_config");
    let path = Path::new("/etc/ssh/sshd_config");

    let changed = files.ensure_file(path, content, Some(0o600), Some("root"), Some("root"))?;

    if changed {
        info!("SSH server configuration changed, restarting sshd...");
        system.service_restart("sshd")?;
    }

    Ok(())
}

fn apply_ssh_hardening_client<S, F>(_system: &S, files: &F) -> Result<(), HardeningError>
where
    S: SystemResource + ?Sized,
    F: FileResource + ?Sized,
{
    info!("Applying ssh-hardening::client");
    let ssh_dir = Path::new("/etc/ssh");
    files.ensure_directory(ssh_dir, Some(0o755), Some("root"), Some("root"))?;

    let content = include_bytes!("../files/ssh_config");
    let path = Path::new("/etc/ssh/ssh_config");

    files.ensure_file(path, content, Some(0o644), Some("root"), Some("root"))?;

    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
