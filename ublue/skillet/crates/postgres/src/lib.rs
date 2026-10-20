//! Reusable isolated PostgreSQL container composition for host applications.

use skillet_core::files::{
    FileError, FileMutationResource, FileReadResource, Ownership, StorageResource,
};
use skillet_core::system::{
    AccountLookupResource, PodmanSecretResource, ServiceResource, SystemError,
};
use skillet_podman::{
    self, MountDependency, NetworkAttachment, PodmanConfig, PodmanError, ProcessIdentity,
    QuadletSecret, SecretTarget, Volume,
};
use std::path::{Path, PathBuf};
use thiserror::Error;

pub mod backup;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub const DEFAULT_IMAGE: &str = "docker.io/postgres:18";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    /// Per-application identity, for example `joplin` or `airtrail`.
    pub instance: &'static str,
    pub database: &'static str,
    pub username: &'static str,
    pub data_path: &'static str,
    pub network_name: &'static str,
    pub image: &'static str,
    /// Image-specific persistent data mount, `/var/lib/postgresql` for 18 and
    /// `/var/lib/postgresql/data` for PostgreSQL 17 and older.
    pub data_container_path: &'static str,
}

impl Config {
    #[must_use]
    pub fn container_name(self) -> String {
        format!("postgresql-{}", self.instance)
    }

    #[must_use]
    pub fn unit_name(self) -> String {
        format!("{}.service", self.container_name())
    }

    #[must_use]
    pub fn password_secret(self) -> String {
        format!("postgresql-{}_password", self.instance)
    }

    fn validate(self) -> Result<(), PostgresError> {
        for (value, kind) in [
            (self.instance, "instance"),
            (self.database, "database"),
            (self.username, "username"),
            (self.network_name, "network"),
        ] {
            if value.is_empty()
                || value.len() > 63
                || !value.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || byte == b'_'
                        || byte == b'-'
                })
            {
                return Err(PostgresError::InvalidConfiguration(kind));
            }
        }
        let data_path = Path::new(self.data_path);
        if !data_path.is_absolute()
            || data_path == Path::new("/")
            || data_path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(PostgresError::InvalidConfiguration("data path"));
        }
        if self.image.trim().is_empty() || self.image.bytes().any(|byte| byte.is_ascii_whitespace())
        {
            return Err(PostgresError::InvalidConfiguration("image"));
        }
        let container_path = Path::new(self.data_container_path);
        if !container_path.is_absolute()
            || container_path == Path::new("/")
            || container_path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(PostgresError::InvalidConfiguration("container data path"));
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PostgresError {
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("File error: {0}")]
    File(#[from] FileError),
    #[error("Podman error: {0}")]
    Podman(#[from] PodmanError),
    #[error("invalid PostgreSQL {0} configuration")]
    InvalidConfiguration(&'static str),
    #[error("PostgreSQL backup runtime I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("PostgreSQL backup command failed")]
    BackupCommand,
    #[error("PostgreSQL restore command failed")]
    RestoreCommand,
    #[error("PostgreSQL service transition failed")]
    ServiceCommand,
}

/// Converge one application-owned database. The caller selects identity,
/// storage, image and network; this module never creates a shared server or
/// publishes PostgreSQL on the host.
pub fn apply<S, F>(
    system: &S,
    files: &F,
    config: Config,
    password: &str,
) -> Result<(), PostgresError>
where
    S: AccountLookupResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    config.validate()?;
    if password.is_empty() {
        return Err(PostgresError::InvalidConfiguration("password"));
    }

    let data_path = PathBuf::from(config.data_path);
    files.require_btrfs_subvolume_mount(Path::new("/var/lib/data"), Path::new("/var"), "/data")?;
    files.ensure_btrfs_subvolume(&data_path)?;
    files.ensure_directory(
        &data_path,
        Some(0o755),
        &Ownership::named(Some("root"), Some("root")),
    )?;

    let secret_name = config.password_secret();
    system.ensure_podman_secret(&secret_name, password)?;

    let container_directives = vec![
        format!("ContainerName={}", config.container_name()),
        format!("Environment=POSTGRES_DB={}", config.database),
        format!("Environment=POSTGRES_USER={}", config.username),
        format!("Environment=POSTGRES_PASSWORD_FILE=/run/secrets/{secret_name}"),
        format!(
            "HealthCmd=pg_isready -U {} -d {}",
            config.username, config.database
        ),
        "HealthInterval=5s".to_string(),
        "HealthRetries=5".to_string(),
    ];

    let mut extra_config = std::collections::BTreeMap::new();
    extra_config.insert("Container".to_string(), container_directives);
    extra_config.insert("Service".to_string(), vec!["Restart=always".to_string()]);
    extra_config.insert(
        "Unit".to_string(),
        vec![
            format!("Description=PostgreSQL database for {}", config.instance),
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
            name: config.container_name(),
            image: config.image.to_string(),
            network_attachments: vec![NetworkAttachment::Bridge(config.network_name.to_string())],
            port_publications: Vec::new(),
            storage_dependency: Some(MountDependency::shared_service_data()),
            process_identity: ProcessIdentity::ImageDefault,
            namespace_mapping: None,
            volumes: vec![Volume {
                host_path: config.data_path.to_string(),
                container_path: config.data_container_path.to_string(),
                options: Some("z".to_string()),
                host_mode: None,
                host_ownership: None,
            }],
            secrets: vec![QuadletSecret {
                secret_name: secret_name.clone(),
                target: SecretTarget::File {
                    target_path: format!("/run/secrets/{secret_name}"),
                    mode: Some("0400".to_string()),
                    uid: None,
                    gid: None,
                },
            }],
            config_revisions: Vec::new(),
            extra_config,
        },
    )?;

    Ok(())
}
