//! `AirTrail` over an application-dedicated PostgreSQL instance.

use serde::{Deserialize, Serialize};
use skillet_core::files::{FileError, FileMutationResource, FileReadResource, StorageResource};
use skillet_core::system::{
    AccountLookupResource, PodmanSecretResource, ServiceResource, SystemError,
};
use skillet_podman::{
    self, MountDependency, NetworkAttachment, PodmanConfig, PodmanError, ProcessIdentity,
    QuadletSecret, SecretTarget, Volume,
};
use skillet_postgres::{self, Config as PostgresConfig, PostgresError};
use std::collections::BTreeMap;
use thiserror::Error;

pub mod backup;

pub const DEFAULT_IMAGE: &str = "docker.io/johly/airtrail:latest";
pub const DATABASE_PASSWORD_CREDENTIAL: &str = "airtrail_postgres_password";
pub const INTEGRATIONS_CREDENTIAL: &str = "airtrail_integrations";
pub const DATABASE_URL_SECRET: &str = "airtrail_database_url";
pub const AERODATABOX_SECRET: &str = "airtrail_aerodatabox_key";
pub const OPENAIP_SECRET: &str = "airtrail_openaip_key";
pub const PORT: u16 = 3000;
pub const BACKUP_KEEP: usize = 7;
pub const BACKUP_UNIT: &str = "skillet-airtrail-backup.service";
pub const BACKUP_TIMER: &str = "skillet-airtrail-backup.timer";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    pub image: &'static str,
    pub network_name: &'static str,
    pub database_path: &'static str,
    pub postgres_data_container_path: &'static str,
    pub uploads_path: &'static str,
    pub backup_path: &'static str,
    pub backup_subvolume: &'static str,
    pub backup_owner: &'static str,
    pub backup_group: &'static str,
}

#[must_use]
pub fn backup_config(config: Config) -> skillet_postgres::backup::Config {
    skillet_postgres::backup::Config {
        instance: "airtrail",
        database: "airtrail",
        username: "airtrail",
        container: "postgresql-airtrail",
        application_unit: "airtrail.service",
        backup_path: config.backup_path,
        backup_subvolume: config.backup_subvolume,
        owner: config.backup_owner,
        group: config.backup_group,
        keep: BACKUP_KEEP,
    }
}

impl Config {
    #[must_use]
    pub fn postgres(self) -> PostgresConfig {
        PostgresConfig {
            instance: "airtrail",
            database: "airtrail",
            username: "airtrail",
            data_path: self.database_path,
            network_name: self.network_name,
            image: skillet_postgres::DEFAULT_IMAGE,
            data_container_path: self.postgres_data_container_path,
        }
    }

    fn validate(self, origin: &str) -> Result<(), AirtrailError> {
        if !origin.starts_with("https://")
            || origin.bytes().any(|byte| byte.is_ascii_whitespace())
            || origin.trim_end_matches('/').len() <= "https://".len()
        {
            return Err(AirtrailError::InvalidConfiguration("origin"));
        }
        if self.image.trim().is_empty() || self.network_name.is_empty() {
            return Err(AirtrailError::InvalidConfiguration("image or network"));
        }
        for (value, name) in [
            (self.database_path, "database path"),
            (self.uploads_path, "uploads path"),
            (self.backup_path, "backup path"),
            (self.backup_subvolume, "backup subvolume"),
        ] {
            let path = std::path::Path::new(value);
            if !path.is_absolute()
                || path == std::path::Path::new("/")
                || path
                    .components()
                    .any(|part| matches!(part, std::path::Component::ParentDir))
            {
                return Err(AirtrailError::InvalidConfiguration(name));
            }
        }
        if self.database_path == self.uploads_path
            || !self.database_path.starts_with("/var/lib/data/")
            || !self.uploads_path.starts_with("/var/lib/data/")
            || !self.backup_subvolume.starts_with("/var/lib/data/")
            || !std::path::Path::new(self.backup_path).starts_with(self.backup_subvolume)
            || self.backup_path == self.backup_subvolume
            || self.backup_owner.is_empty()
            || self.backup_group.is_empty()
        {
            return Err(AirtrailError::InvalidConfiguration("storage layout"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Integrations {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aerodatabox: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub openaip: Option<String>,
}

impl Integrations {
    pub fn parse(payload: &str) -> Result<Self, AirtrailError> {
        let parsed: Self = serde_json::from_str(payload)
            .map_err(|_| AirtrailError::InvalidConfiguration("integration credentials"))?;
        for value in [parsed.aerodatabox.as_deref(), parsed.openaip.as_deref()]
            .into_iter()
            .flatten()
        {
            if value.is_empty() || value.bytes().any(|byte| byte.is_ascii_whitespace()) {
                return Err(AirtrailError::InvalidConfiguration(
                    "integration credential",
                ));
            }
        }
        Ok(parsed)
    }
}

#[derive(Debug, Error)]
pub enum AirtrailError {
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("File error: {0}")]
    File(#[from] FileError),
    #[error("Podman error: {0}")]
    Podman(#[from] PodmanError),
    #[error(transparent)]
    Postgres(#[from] PostgresError),
    #[error(transparent)]
    Backup(#[from] backup::Error),
    #[error("invalid AirTrail {0} configuration")]
    InvalidConfiguration(&'static str),
}

/// Apply `AirTrail` with native app authentication behind private Caddy HTTPS.
pub fn apply<S, F>(
    system: &S,
    files: &F,
    host: &str,
    config: Config,
    origin: &str,
    database_password: &str,
    integrations: &Integrations,
) -> Result<(), AirtrailError>
where
    S: AccountLookupResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    config.validate(origin)?;
    if database_password.len() < 32
        || !database_password
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err(AirtrailError::InvalidConfiguration("database password"));
    }
    files.require_btrfs_subvolume_mount(
        std::path::Path::new("/var/lib/data"),
        std::path::Path::new("/var"),
        "/data",
    )?;
    files.require_btrfs_subvolume(std::path::Path::new(config.backup_subvolume))?;

    let postgres = config.postgres();
    skillet_postgres::apply(system, files, postgres, database_password)?;
    let uploads = std::path::Path::new(config.uploads_path);
    files.ensure_directory(
        uploads,
        Some(0o755),
        &skillet_core::files::Ownership::named(Some("root"), Some("root")),
    )?;
    install_backup(system, files, host, config)?;

    let secrets = install_container_secrets(system, &postgres, database_password, integrations)?;

    let container = vec![
        "ContainerName=airtrail".to_string(),
        format!("Environment=ORIGIN={origin}"),
        "Environment=ADDRESS_HEADER=X-Forwarded-For".to_string(),
        "Environment=UPLOAD_LOCATION=/app/uploads".to_string(),
        format!("Environment=PORT={PORT}"),
        format!(
            "Annotation=run.oci.condition-wait={}:healthy",
            postgres.unit_name()
        ),
    ];
    let mut extra_config = BTreeMap::new();
    extra_config.insert("Container".to_string(), container);
    extra_config.insert("Service".to_string(), vec!["Restart=always".to_string()]);
    extra_config.insert(
        "Unit".to_string(),
        vec![
            "Description=AirTrail Server".to_string(),
            format!("After={}", postgres.unit_name()),
            format!("Requires={}", postgres.unit_name()),
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
            name: "airtrail".to_string(),
            image: config.image.to_string(),
            network_attachments: vec![NetworkAttachment::Bridge(config.network_name.to_string())],
            port_publications: Vec::new(),
            storage_dependency: Some(MountDependency::shared_service_data()),
            process_identity: ProcessIdentity::ImageDefault,
            namespace_mapping: None,
            volumes: vec![Volume {
                host_path: config.uploads_path.to_string(),
                container_path: "/app/uploads".to_string(),
                options: Some("z".to_string()),
                host_mode: None,
                host_ownership: None,
            }],
            secrets,
            config_revisions: Vec::new(),
            extra_config,
        },
    )?;
    Ok(())
}

fn install_container_secrets<S: PodmanSecretResource + ?Sized>(
    system: &S,
    postgres: &PostgresConfig,
    database_password: &str,
    integrations: &Integrations,
) -> Result<Vec<QuadletSecret>, AirtrailError> {
    let db_url = format!(
        "postgres://airtrail:{database_password}@{}:5432/airtrail",
        postgres.container_name()
    );
    system.ensure_podman_secret(DATABASE_URL_SECRET, &db_url)?;
    let mut secrets = vec![QuadletSecret {
        secret_name: DATABASE_URL_SECRET.to_string(),
        target: SecretTarget::Environment {
            env_var_name: "DB_URL".to_string(),
        },
    }];
    for (value, secret_name, env_name) in [
        (
            integrations.aerodatabox.as_deref(),
            AERODATABOX_SECRET,
            "INTEGRATIONS_AERO_DATA_BOX_KEY",
        ),
        (
            integrations.openaip.as_deref(),
            OPENAIP_SECRET,
            "INTEGRATIONS_OPEN_AIP_KEY",
        ),
    ] {
        if let Some(value) = value {
            system.ensure_podman_secret(secret_name, value)?;
            secrets.push(QuadletSecret {
                secret_name: secret_name.to_string(),
                target: SecretTarget::Environment {
                    env_var_name: env_name.to_string(),
                },
            });
        }
    }
    Ok(secrets)
}

pub fn install_backup<S, F>(
    system: &S,
    files: &F,
    host: &str,
    config: Config,
) -> Result<(), AirtrailError>
where
    S: AccountLookupResource + ServiceResource + ?Sized,
    F: FileMutationResource + ?Sized,
{
    config.validate("https://airtrail.invalid")?;
    if !valid_host(host) {
        return Err(AirtrailError::InvalidConfiguration("host"));
    }
    if system.user_by_name(config.backup_owner)?.is_none()
        || system.group_by_name(config.backup_group)?.is_none()
    {
        return Err(AirtrailError::InvalidConfiguration("backup owner"));
    }
    skillet_postgres::backup::prepare_directory(files, &backup_config(config))?;
    let root = skillet_core::files::Ownership::named(Some("root"), Some("root"));
    let unit_dir = std::path::Path::new("/etc/systemd/system");
    files.ensure_directory(unit_dir, Some(0o755), &root)?;
    let binary = format!("/var/usrlocal/bin/skillet-{host}");
    let backup_service = format!(
        "[Unit]\nDescription=Create a consistent AirTrail database and uploads backup\nRequiresMountsFor=/var/lib/data\nRequires=postgresql-airtrail.service\nAfter=postgresql-airtrail.service\nConditionPathIsMountPoint=/var/lib/data\n[Service]\nType=oneshot\nTimeoutStartSec=2h\nExecStart={binary} airtrail backup\nUMask=0027\nNoNewPrivileges=yes\nProtectHome=yes\nPrivateTmp=yes\n"
    );
    let timer = "[Unit]\nDescription=Daily AirTrail database and uploads backup\n[Timer]\nOnCalendar=daily\nRandomizedDelaySec=4h\nPersistent=true\n[Install]\nWantedBy=timers.target\n";
    let service_changed = files.ensure_file(
        &unit_dir.join(BACKUP_UNIT),
        backup_service.as_bytes(),
        Some(0o644),
        &root,
    )?;
    let timer_changed = files.ensure_file(
        &unit_dir.join(BACKUP_TIMER),
        timer.as_bytes(),
        Some(0o644),
        &root,
    )?;
    if service_changed || timer_changed {
        system.daemon_reload()?;
    }
    system.service_enable(BACKUP_TIMER)?;
    system.service_start(BACKUP_TIMER)?;
    Ok(())
}

fn valid_host(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 63
        && host
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
