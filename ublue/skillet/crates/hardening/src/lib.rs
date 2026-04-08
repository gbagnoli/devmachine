use askama::Template;
use skillet_core::files::{FileError, FileResource};
use skillet_core::system::{SystemError, SystemResource};
use skillet_core::templates::ensure_templated_file;
use skillet_podman::{self, ContainerUser, HostUser, PodmanError, Volume};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum HardeningError {
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("File error: {0}")]
    File(#[from] FileError),
    #[error("Podman error: {0}")]
    Podman(#[from] PodmanError),
}

#[derive(Template)]
#[template(path = "pihole/custom.list.j2")]
struct CustomListTemplate {
    custom: HashMap<String, String>,
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

    // 5. Include 'pihole'
    apply_pihole(system, files)?;

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

    let changed = files.ensure_file(&path, content, Some(0o644), Some("root"), Some("root"))?;

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
    let content = include_bytes!("../files/ssh_config");
    let path = Path::new("/etc/ssh/ssh_config");

    files.ensure_file(path, content, Some(0o644), Some("root"), Some("root"))?;

    Ok(())
}

fn apply_pihole<S, F>(system: &S, files: &F) -> Result<(), HardeningError>
where
    S: SystemResource + ?Sized,
    F: FileResource + ?Sized,
{
    info!("Applying pihole hardening...");
    let root = "/etc/pihole";
    let logs = "/var/log/pihole";

    // 1. Ensure directories
    files.ensure_directory(Path::new(root), Some(0o755), Some("root"), Some("root"))?;
    files.ensure_directory(
        &Path::new(root).join("conf"),
        Some(0o755),
        Some("root"),
        Some("root"),
    )?;
    files.ensure_directory(
        &Path::new(root).join("dnsmasq.d"),
        Some(0o755),
        Some("root"),
        Some("root"),
    )?;
    files.ensure_directory(Path::new(logs), Some(0o755), Some("root"), Some("root"))?;

    // 2. Custom list template
    let mut custom = HashMap::new();
    custom.insert("192.168.1.100".to_string(), "my.custom.domain".to_string());

    let template = CustomListTemplate { custom };
    ensure_templated_file(
        files,
        &Path::new(root).join("conf/custom.list"),
        &template,
        Some(0o640),
        Some("root"),
        Some("root"),
    )?;

    // 3. Define container
    let user = ContainerUser {
        container_uid: 0, // pihole usually runs as root in container
        container_gid: 0,
        host_user: Some(HostUser::Name("root".to_string())),
    };

    let volumes = vec![
        Volume {
            host_path: format!("{root}/conf"),
            container_path: "/etc/pihole".to_string(),
            options: None,
        },
        Volume {
            host_path: format!("{root}/dnsmasq.d"),
            container_path: "/etc/dnsmasq.d".to_string(),
            options: None,
        },
        Volume {
            host_path: logs.to_string(),
            container_path: "/var/log/pihole".to_string(),
            options: None,
        },
    ];

    let mut extra_config = BTreeMap::new();
    extra_config.insert(
        "Service".to_string(),
        vec!["Restart=always".to_string()],
    );
    extra_config.insert(
        "Unit".to_string(),
        vec![
            "Description=Pi. Hole".to_string(),
            "After=network-online.target".to_string(),
        ],
    );
    extra_config.insert(
        "Install".to_string(),
        vec!["WantedBy=multi-user.target default.target".to_string()],
    );

    skillet_podman::container(
        system,
        files,
        skillet_podman::PodmanConfig {
            name: "pihole".to_string(),
            image: "docker.io/pihole/pihole:latest".to_string(),
            user,
            create_host_user: false,
            volumes,
            extra_config,
        },
    )?;

    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
