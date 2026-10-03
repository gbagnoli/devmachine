use askama::Template;
use skillet_core::files::{FileError, FileMutationResource, FileReadResource, StorageResource};
use skillet_core::system::{
    AccountLookupResource, AccountResource, PodmanSecretResource, ServiceResource, SystemError,
};
use skillet_podman::{
    self, NetworkAttachment, PodmanConfig, PodmanError, PortProtocol, PortPublication,
    ProcessIdentity, QuadletSecret, Volume,
};
use std::collections::BTreeMap;
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
    custom: BTreeMap<String, String>,
}

pub struct PiholeUser {
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub name: String,
    pub group_name: String,
}

/// Host-specific Pi-hole configuration: custom DNS records (`ip -> fqdn`)
/// rendered into `custom.list`. Kept out of the shared crate so each host
/// supplies its own LAN topology.
pub fn apply<S, F>(
    system: &S,
    files: &F,
    user_config: &PiholeUser,
    secrets: Vec<QuadletSecret>,
    custom_records: BTreeMap<String, String>,
    network_name: String,
) -> Result<(), PiholeError>
where
    S: AccountLookupResource + AccountResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    info!("Applying pihole configuration...");
    let root = "/var/lib/data/pihole";
    let etc = "/var/lib/data/pihole/etc";
    let logs = "/var/lib/data/pihole/log";

    files.require_btrfs_subvolume_mount(Path::new("/var/lib/data"), Path::new("/var"), "/data")?;
    files.ensure_btrfs_subvolume(Path::new(root))?;

    // 1. Ensure user and group
    system.ensure_group(&user_config.group_name, user_config.gid)?;
    system.ensure_user(&user_config.name, user_config.uid, user_config.gid)?;

    // 2. Ensure directories
    files.ensure_directory(Path::new(root), Some(0o755), Some("root"), Some("root"))?;
    files.ensure_directory(Path::new(etc), Some(0o755), None, None)?;
    files.ensure_directory(&Path::new(root).join("dnsmasq.d"), Some(0o755), None, None)?;
    files.ensure_directory(Path::new(logs), Some(0o755), None, None)?;

    // 3. Custom list template (records supplied by the host)
    let template = CustomListTemplate {
        custom: custom_records,
    };
    let custom_list = template.render().map_err(|error| {
        FileError::Io(std::io::Error::other(format!(
            "Template rendering failed: {error}"
        )))
    })?;
    files.ensure_file(
        &Path::new(etc).join("custom.list"),
        custom_list.as_bytes(),
        Some(0o640),
        None,
        None,
    )?;

    // 4. Define container
    // SELinux relabeling (:z, shared) so the container can access these
    // host paths on enforcing systems such as uCore.
    let volumes = vec![
        Volume {
            host_path: etc.to_string(),
            container_path: "/etc/pihole".to_string(),
            options: Some("z".to_string()),
        },
        Volume {
            host_path: format!("{root}/dnsmasq.d"),
            container_path: "/etc/dnsmasq.d".to_string(),
            options: Some("z".to_string()),
        },
        Volume {
            host_path: logs.to_string(),
            container_path: "/var/log/pihole".to_string(),
            options: Some("z".to_string()),
        },
    ];

    let mut extra_config = BTreeMap::new();
    extra_config.insert("Service".to_string(), vec!["Restart=always".to_string()]);
    extra_config.insert(
        "Container".to_string(),
        vec![
            "AutoUpdate=registry".to_string(),
            "ContainerName=pihole".to_string(),
            "Environment=FTLCONF_dns_listeningMode=ALL".to_string(),
            "Environment=FTLCONF_webserver_port=8088o,[::]:8088o".to_string(),
            "Environment=TZ=Europe/Madrid".to_string(),
            "Environment=WEBPASSWORD_FILE=/run/secrets/pihole_web_password".to_string(),
        ],
    );
    extra_config.insert(
        "Unit".to_string(),
        vec![
            "Description=Pi. Hole".to_string(),
            "After=network-online.target".to_string(),
            "Requires=skillet-data-prepare.service".to_string(),
            "After=skillet-data-prepare.service".to_string(),
            "BindsTo=var-lib-data.mount".to_string(),
            "After=var-lib-data.mount".to_string(),
            "AssertPathIsMountPoint=/var/lib/data".to_string(),
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
            network_attachments: vec![NetworkAttachment::Bridge(network_name)],
            port_publications: dns_port_publications(),
            process_identity: ProcessIdentity::ImageDefault,
            namespace_mapping: None,
            volumes,
            secrets,
            config_revisions: vec![custom_list.into_bytes()],
            extra_config,
        },
    )?;

    Ok(())
}

fn dns_port_publications() -> Vec<PortPublication> {
    [
        std::net::Ipv6Addr::UNSPECIFIED.into(),
        std::net::Ipv4Addr::UNSPECIFIED.into(),
    ]
    .into_iter()
    .flat_map(|host_address| {
        [PortProtocol::Tcp, PortProtocol::Udp].map(move |protocol| PortPublication {
            host_address,
            host_port: 53,
            container_port: 53,
            protocol,
        })
    })
    .collect()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
