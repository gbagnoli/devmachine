use askama::Template;
use skillet_core::files::{FileError, FileResource};
use skillet_core::system::{SystemError, SystemResource};
use skillet_core::templates::ensure_templated_file;
use skillet_podman::{self, ContainerUser, HostUser, PodmanError, Volume, PodmanConfig, QuadletSecret};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use thiserror::Error;
use tracing::info;

#[derive(Error, Debug)]
pub enum PiholeError {
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

pub struct PiholeUser {
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub name: String,
    pub group_name: String,
}

pub fn apply<S, F>(
    system: &S,
    files: &F,
    user_config: PiholeUser,
    secrets: Vec<QuadletSecret>,
) -> Result<(), PiholeError>
where
    S: SystemResource + ?Sized,
    F: FileResource + ?Sized,
{
    info!("Applying pihole configuration...");
    let root = "/etc/pihole";
    let logs = "/var/log/pihole";

    // 1. Ensure user and group
    system.ensure_group(&user_config.group_name, user_config.gid)?;
    system.ensure_user(&user_config.name, user_config.uid, user_config.gid)?;

    // 2. Ensure directories
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

    // 3. Custom list template
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

    // 4. Define container
    let user = ContainerUser {
        container_uid: 1000,
        container_gid: 1000,
        host_user: Some(HostUser::Name(user_config.name)),
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
        PodmanConfig {
            name: "pihole".to_string(),
            image: "docker.io/pihole/pihole:latest".to_string(),
            user,
            create_host_user: false,
            volumes,
            secrets,
            extra_config,
        },
    )?;

    Ok(())
}
