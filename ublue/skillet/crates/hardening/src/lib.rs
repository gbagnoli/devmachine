use sha2::{Digest, Sha256};
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

    // Common setup for SSH
    let ssh_dir = Path::new("/etc/ssh");
    files.ensure_directory(ssh_dir, Some(0o755), Some("root"), Some("root"))?;

    // 3. Include 'ssh-hardening::server'
    apply_ssh_hardening_server(system, files)?;

    // 4. Include 'ssh-hardening::client'
    apply_ssh_hardening_client(system, files)?;

    Ok(())
}

fn converge_service_file<S, F>(
    system: &S,
    files: &F,
    path: &Path,
    content: &[u8],
    mode: u32,
    service: &str,
    ensure_active: bool,
) -> Result<(), HardeningError>
where
    S: SystemResource + ?Sized,
    F: FileResource + ?Sized,
{
    let state_dir = Path::new("/var/lib/skillet/hardening");
    files.ensure_directory(state_dir, Some(0o755), Some("root"), Some("root"))?;
    let applied_path = state_dir.join(format!("{service}.applied"));
    let revision = hex::encode(Sha256::digest(content));
    let pending = files.read_file(&applied_path)?.as_deref() != Some(revision.as_bytes());
    let changed = files.ensure_file(path, content, Some(mode), Some("root"), Some("root"))?;
    if changed || pending {
        system.service_restart(service)?;
        files.ensure_file(
            &applied_path,
            revision.as_bytes(),
            Some(0o644),
            Some("root"),
            Some("root"),
        )?;
    } else if ensure_active && !system.service_is_active(service)? {
        system.service_start(service)?;
    }
    Ok(())
}

fn apply_sysctl_hardening<S, F>(system: &S, files: &F) -> Result<(), HardeningError>
where
    S: SystemResource + ?Sized,
    F: FileResource + ?Sized,
{
    info!("Applying sysctl hardening...");
    let sysctl_dir = Path::new("/etc/sysctl.d");
    files.ensure_directory(sysctl_dir, Some(0o755), Some("root"), Some("root"))?;

    let content = include_bytes!("../files/sysctl.boxy.conf");
    let path = sysctl_dir.join("99-hardening.conf");

    converge_service_file(
        system,
        files,
        &path,
        content,
        0o644,
        "systemd-sysctl",
        false,
    )?;

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
    let content = include_bytes!("../files/sshd_config");
    let path = Path::new("/etc/ssh/sshd_config");

    converge_service_file(system, files, path, content, 0o600, "sshd", true)?;

    Ok(())
}

fn apply_ssh_hardening_client<S, F>(_system: &S, files: &F) -> Result<(), HardeningError>
where
    S: SystemResource + ?Sized,
    F: FileResource + ?Sized,
{
    info!("Applying ssh-hardening::client");
    let content = include_bytes!("../files/ssh_config");
    let path = Path::new("/etc/ssh/ssh_config");

    files.ensure_file(path, content, Some(0o644), Some("root"), Some("root"))?;

    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
