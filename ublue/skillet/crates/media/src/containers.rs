use crate::{bootstrap, App, Config, Credentials, Error, CREDENTIAL};
use skillet_core::{
    files::{FileResource, Ownership},
    system::SystemResource,
};
use skillet_podman::{
    MountDependency, NetworkAttachment, PodmanConfig, PodmanNetwork, ProcessIdentity,
    QuadletSecret, SecretTarget, Volume,
};
use std::{collections::BTreeMap, path::Path};

impl Config {
    pub(crate) fn ownership(self) -> Ownership {
        Ownership::named(Some(self.data_owner), Some(self.data_group))
    }
}

/// The caller supplies only networks protected by its private UI declaration.
pub fn apply(
    system: &dyn SystemResource,
    files: &dyn FileResource,
    config: Config,
    ui_networks: &BTreeMap<App, String>,
    credentials: &Credentials,
    host: &str,
) -> Result<(), Error> {
    config.validate()?;
    credentials.validate()?;
    let identity = config.identity(system)?;
    if App::ARR.iter().any(|app| !ui_networks.contains_key(app)) {
        return Err(Error::Invalid("private UI networks"));
    }
    files.require_btrfs_subvolume_mount(Path::new("/var/lib/data"), Path::new("/var"), "/data")?;
    skillet_podman::ensure_network(
        system,
        files,
        &PodmanNetwork {
            unit_name: config.network.into(),
            options: vec![
                "Driver=bridge".into(),
                "DisableDNS=false".into(),
                "IPv6=true".into(),
                "Options=isolate=strict".into(),
                format!("NetworkName={}", config.network),
            ],
        },
    )?;
    for path in [
        config.files_path.to_string(),
        format!("{}/downloads", config.files_path),
        format!("{}/library", config.files_path),
        format!("{}/library/movies", config.files_path),
        format!("{}/library/tv", config.files_path),
        config.music_path.into(),
    ] {
        files.ensure_directory(Path::new(&path), Some(0o2775), &config.ownership())?;
    }
    for app in App::ALL {
        install_container(
            system,
            files,
            config,
            app,
            ui_networks.get(&app),
            credentials,
            identity,
        )?;
    }
    install_reconciliation(system, files, host)?;
    system.service_start("skillet-media-reconcile.service")?;
    Ok(())
}

fn install_container(
    system: &dyn SystemResource,
    files: &dyn FileResource,
    config: Config,
    app: App,
    ui_network: Option<&String>,
    credentials: &Credentials,
    (uid, gid): (u32, u32),
) -> Result<(), Error> {
    let mut networks = vec![NetworkAttachment::Bridge(config.network.into())];
    if let Some(network) = ui_network {
        networks.push(NetworkAttachment::Bridge(network.clone()));
    }
    let mut container = vec![
        format!("ContainerName={}", app.name()),
        "AutoUpdate=registry".into(),
        "Environment=TZ=Etc/UTC".into(),
    ];
    let mut volumes = Vec::new();
    let mut secrets = Vec::new();
    let identity = if app == App::Putioarr {
        let payload = bootstrap::putio_config(credentials, uid)?;
        system.ensure_podman_secret("media_putioarr_config", &payload)?;
        container.extend([
            "Entrypoint=/usr/bin/putioarr".into(),
            "Exec=run -c /run/secrets/putioarr.toml".into(),
        ]);
        secrets.push(QuadletSecret {
            secret_name: "media_putioarr_config".into(),
            target: SecretTarget::File {
                target_path: "/run/secrets/putioarr.toml".into(),
                mode: Some("0400".into()),
                uid: Some(uid),
                gid: Some(gid),
            },
        });
        ProcessIdentity::Numeric { uid, gid }
    } else {
        bootstrap::ensure(files, config, app, credentials.key(app)?)?;
        container.extend([
            format!("Environment=PUID={uid}"),
            format!("Environment=PGID={gid}"),
            "Environment=UMASK=002".into(),
        ]);
        volumes.push(volume(
            config.config_path(app).to_string_lossy().into_owned(),
            "/config",
        ));
        ProcessIdentity::ImageDefault
    };
    if app != App::Prowlarr {
        volumes.push(volume(config.files_path.into(), "/data"));
    }
    if app == App::Lidarr {
        volumes.push(volume(config.music_path.into(), "/music"));
    }
    let extra_config = BTreeMap::from([
        ("Container".into(), container),
        (
            "Unit".into(),
            vec![
                format!("Description=Media stack {}", app.name()),
                "After=network-online.target".into(),
            ],
        ),
        (
            "Service".into(),
            vec!["Restart=always".into(), "TimeoutStartSec=900".into()],
        ),
        ("Install".into(), vec!["WantedBy=multi-user.target".into()]),
    ]);
    skillet_podman::container(
        system,
        files,
        PodmanConfig {
            name: app.name().into(),
            image: app.image().into(),
            network_attachments: networks,
            port_publications: Vec::new(),
            storage_dependency: Some(MountDependency::shared_service_data()),
            process_identity: identity,
            namespace_mapping: None,
            volumes,
            secrets,
            config_revisions: Vec::new(),
            extra_config,
        },
    )?;
    Ok(())
}

fn volume(host_path: String, target: &str) -> Volume {
    Volume {
        host_path,
        container_path: target.into(),
        options: Some("z".into()),
        host_mode: None,
        host_ownership: None,
    }
}

fn install_reconciliation(
    system: &dyn SystemResource,
    files: &dyn FileResource,
    host: &str,
) -> Result<(), Error> {
    if host.is_empty()
        || !host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(Error::Invalid("host"));
    }
    let units = App::ALL
        .map(|app| format!("{}.service", app.name()))
        .join(" ");
    let unit = format!("[Unit]\nDescription=Reconcile the declared Arr application links\nRequires={units}\nAfter={units}\nRequiresMountsFor=/var/lib/data\n[Service]\nType=oneshot\nTimeoutStartSec=5min\nLoadCredentialEncrypted={CREDENTIAL}:/etc/credstore.encrypted/skillet/{CREDENTIAL}.cred\nExecStart=/var/usrlocal/bin/skillet-{host} media reconcile\nUMask=0077\n[Install]\nWantedBy=multi-user.target\n");
    let path = Path::new("/etc/systemd/system/skillet-media-reconcile.service");
    if files.ensure_file(
        path,
        unit.as_bytes(),
        Some(0o644),
        &Ownership::named(Some("root"), Some("root")),
    )? {
        system.daemon_reload()?;
    }
    system.service_enable("skillet-media-reconcile.service")?;
    Ok(())
}
