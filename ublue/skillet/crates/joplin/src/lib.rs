//! Joplin Server container composition over an isolated PostgreSQL instance.

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

pub const DEFAULT_IMAGE: &str = "docker.io/joplin/server:latest";
pub const DATABASE_PASSWORD_CREDENTIAL: &str = "joplin_postgres_password";
pub const PORT: u16 = 22_300;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    pub image: &'static str,
    pub network_name: &'static str,
    pub database_path: &'static str,
    pub postgres_data_container_path: &'static str,
}

impl Config {
    #[must_use]
    pub fn postgres(self) -> PostgresConfig {
        PostgresConfig {
            instance: "joplin",
            database: "joplin",
            username: "joplin",
            data_path: self.database_path,
            network_name: self.network_name,
            image: skillet_postgres::DEFAULT_IMAGE,
            data_container_path: self.postgres_data_container_path,
        }
    }

    fn validate(self, base_url: &str) -> Result<(), JoplinError> {
        if !base_url.starts_with("https://")
            || base_url.bytes().any(|byte| byte.is_ascii_whitespace())
            || base_url.trim_end_matches('/').len() <= "https://".len()
        {
            return Err(JoplinError::InvalidConfiguration("base URL"));
        }
        if self.network_name.is_empty() || self.image.trim().is_empty() {
            return Err(JoplinError::InvalidConfiguration("network or image"));
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum JoplinError {
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("File error: {0}")]
    File(#[from] FileError),
    #[error("Podman error: {0}")]
    Podman(#[from] PodmanError),
    #[error(transparent)]
    Postgres(#[from] PostgresError),
    #[error("invalid Joplin {0} configuration")]
    InvalidConfiguration(&'static str),
}

/// Apply Joplin and its application-dedicated PostgreSQL service.
pub fn apply<S, F>(
    system: &S,
    files: &F,
    config: Config,
    base_url: &str,
    database_password: &str,
) -> Result<(), JoplinError>
where
    S: AccountLookupResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    config.validate(base_url)?;
    let postgres = config.postgres();
    skillet_postgres::apply(system, files, postgres, database_password)?;

    let mut extra_config = BTreeMap::new();
    extra_config.insert(
        "Container".to_string(),
        vec![
            "ContainerName=joplin-server".to_string(),
            "Environment=DB_CLIENT=pg".to_string(),
            "Environment=POSTGRES_DATABASE=joplin".to_string(),
            "Environment=POSTGRES_USER=joplin".to_string(),
            format!("Environment=POSTGRES_HOST={}", postgres.container_name()),
            "Environment=POSTGRES_PORT=5432".to_string(),
            format!("Environment=APP_BASE_URL={base_url}"),
            format!("Environment=APP_PORT={PORT}"),
            "Environment=MAX_TIME_DRIFT=0".to_string(),
            format!(
                "Annotation=run.oci.condition-wait={}:healthy",
                postgres.unit_name()
            ),
        ],
    );
    extra_config.insert("Service".to_string(), vec!["Restart=always".to_string()]);
    extra_config.insert(
        "Unit".to_string(),
        vec![
            "Description=Joplin Server".to_string(),
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
            name: "joplin-server".to_string(),
            image: config.image.to_string(),
            network_attachments: vec![NetworkAttachment::Bridge(config.network_name.to_string())],
            port_publications: Vec::new(),
            storage_dependency: Some(MountDependency::shared_service_data()),
            process_identity: ProcessIdentity::ImageDefault,
            namespace_mapping: None,
            volumes: vec![Volume {
                host_path: "/etc/localtime".to_string(),
                container_path: "/etc/localtime".to_string(),
                options: Some("ro".to_string()),
                host_mode: None,
                host_ownership: None,
            }],
            secrets: vec![QuadletSecret {
                secret_name: postgres.password_secret(),
                target: SecretTarget::Environment {
                    env_var_name: "POSTGRES_PASSWORD".to_string(),
                },
            }],
            config_revisions: Vec::new(),
            extra_config,
        },
    )?;

    Ok(())
}
