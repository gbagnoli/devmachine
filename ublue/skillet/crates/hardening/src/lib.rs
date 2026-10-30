use sha2::{Digest, Sha256};
use skillet_core::activation::{self, ActivationRequest, ConsumerKind};
use skillet_core::files::{FileError, FileMutationResource, FileReadResource, Ownership};
use skillet_core::system::{ServiceResource, SystemError};
use std::path::Path;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum HardeningError {
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("File error: {0}")]
    File(#[from] FileError),
    #[error("Activation error: {0}")]
    Activation(#[from] skillet_core::activation::ActivationError),
}

pub fn apply<S, F>(system: &S, files: &F) -> Result<(), HardeningError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    info!("Applying hardening...");

    // 1. Sysctl hardening
    apply_sysctl_hardening(system, files)?;

    // 2. Include 'os-hardening'
    apply_os_hardening(system);

    // Common setup for SSH
    let ssh_dir = Path::new("/etc/ssh");
    files.ensure_directory(
        ssh_dir,
        Some(0o755),
        &Ownership::named(Some("root"), Some("root")),
    )?;

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
) -> Result<(), HardeningError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let state_dir = Path::new("/var/lib/skillet/hardening");
    files.ensure_directory(
        state_dir,
        Some(0o755),
        &Ownership::named(Some("root"), Some("root")),
    )?;
    let applied_path = state_dir.join(format!("{service}.applied"));
    let revision = hex::encode(Sha256::digest(content));
    let changed = files.ensure_file(
        path,
        content,
        Some(mode),
        &Ownership::named(Some("root"), Some("root")),
    )?;
    activation::activate(
        system,
        files,
        &ActivationRequest {
            service: service.to_string(),
            state_path: applied_path,
            revision: revision.into_bytes(),
            definition_changed: changed,
            reload_daemon: false,
            consumer_kind: ConsumerKind::Persistent,
        },
    )?;
    Ok(())
}

fn apply_sysctl_hardening<S, F>(system: &S, files: &F) -> Result<(), HardeningError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    info!("Applying sysctl hardening...");
    let sysctl_dir = Path::new("/etc/sysctl.d");
    files.ensure_directory(
        sysctl_dir,
        Some(0o755),
        &Ownership::named(Some("root"), Some("root")),
    )?;

    let content = include_bytes!("../files/sysctl.boxy.conf");
    let path = sysctl_dir.join("99-hardening.conf");

    converge_service_file(system, files, &path, content, 0o644, "systemd-sysctl")?;

    Ok(())
}

fn apply_os_hardening<S: ServiceResource + ?Sized>(_system: &S) {
    info!("(Placeholder) Applying os-hardening");
}

fn apply_ssh_hardening_server<S, F>(system: &S, files: &F) -> Result<(), HardeningError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    info!("Applying ssh-hardening::server");
    let content = include_bytes!("../files/sshd_config");
    let path = Path::new("/etc/ssh/sshd_config");

    converge_service_file(system, files, path, content, 0o600, "sshd")?;

    Ok(())
}

fn apply_ssh_hardening_client<S, F>(_system: &S, files: &F) -> Result<(), HardeningError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    info!("Applying ssh-hardening::client");
    let content = include_bytes!("../files/ssh_config");
    let path = Path::new("/etc/ssh/ssh_config");

    files.ensure_file(
        path,
        content,
        Some(0o644),
        &Ownership::named(Some("root"), Some("root")),
    )?;

    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
