//! Fresh Arr bootstrap, private container composition and owned API links.
use serde::{Deserialize, Serialize};
use skillet_core::{files::FileError, system::SystemError};
use std::collections::BTreeMap;
use thiserror::Error;

pub mod bootstrap;
mod containers;
pub mod reconcile;
pub mod runtime;
pub use containers::apply;

pub const CREDENTIAL: &str = "media_credentials";
pub const APPLY_UNIT: &str = "skillet-media-apply.service";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum App {
    Prowlarr,
    Sonarr,
    Radarr,
    Lidarr,
    Putioarr,
}

impl App {
    pub const ALL: [Self; 5] = [
        Self::Prowlarr,
        Self::Sonarr,
        Self::Radarr,
        Self::Lidarr,
        Self::Putioarr,
    ];
    pub const ARR: [Self; 4] = [Self::Prowlarr, Self::Sonarr, Self::Radarr, Self::Lidarr];
    pub const LIBRARIES: [Self; 3] = [Self::Sonarr, Self::Radarr, Self::Lidarr];
    pub const fn name(self) -> &'static str {
        match self {
            Self::Prowlarr => "prowlarr",
            Self::Sonarr => "sonarr",
            Self::Radarr => "radarr",
            Self::Lidarr => "lidarr",
            Self::Putioarr => "putioarr",
        }
    }
    pub const fn implementation(self) -> &'static str {
        match self {
            Self::Prowlarr => "Prowlarr",
            Self::Sonarr => "Sonarr",
            Self::Radarr => "Radarr",
            Self::Lidarr => "Lidarr",
            Self::Putioarr => "Transmission",
        }
    }
    pub const fn port(self) -> u16 {
        match self {
            Self::Prowlarr => 9696,
            Self::Sonarr => 8989,
            Self::Radarr => 7878,
            Self::Lidarr => 8686,
            Self::Putioarr => 9091,
        }
    }
    pub const fn api(self) -> &'static str {
        match self {
            Self::Sonarr | Self::Radarr => "api/v3",
            _ => "api/v1",
        }
    }
    pub const fn root(self) -> Option<&'static str> {
        match self {
            Self::Sonarr => Some("/data/library/tv"),
            Self::Radarr => Some("/data/library/movies"),
            Self::Lidarr => Some("/music"),
            _ => None,
        }
    }
    pub const fn image(self) -> &'static str {
        match self {
            Self::Prowlarr => "lscr.io/linuxserver/prowlarr:latest",
            Self::Sonarr => "lscr.io/linuxserver/sonarr:latest",
            Self::Radarr => "lscr.io/linuxserver/radarr:latest",
            Self::Lidarr => "lscr.io/linuxserver/lidarr:latest",
            Self::Putioarr => "ghcr.io/wouterdebie/putioarr:latest",
        }
    }
    pub fn url(self) -> String {
        format!("http://{}:{}", self.name(), self.port())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    pub config_root: &'static str,
    pub files_path: &'static str,
    pub music_path: &'static str,
    pub network: &'static str,
    pub uid: u32,
    pub gid: u32,
}

impl Config {
    pub fn config_path(self, app: App) -> std::path::PathBuf {
        std::path::Path::new(self.config_root).join(app.name())
    }
    pub fn validate(self) -> Result<(), Error> {
        for value in [self.config_root, self.files_path, self.music_path] {
            if !value.starts_with("/var/lib/data/") || value.split('/').any(|v| v == "..") {
                return Err(Error::Invalid("storage layout"));
            }
        }
        if self.config_root == self.files_path
            || self.uid == 0
            || self.gid == 0
            || self.network.is_empty()
        {
            return Err(Error::Invalid("storage identity or network"));
        }
        Ok(())
    }
}

// Do not derive Debug: this payload contains provider and integration secrets.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Credentials {
    pub api_keys: BTreeMap<App, String>,
    pub putio_password: String,
    pub putio_token: String,
}

impl Credentials {
    pub fn parse(input: &str) -> Result<Self, Error> {
        let value: Self =
            serde_json::from_str(input).map_err(|_| Error::Invalid("credential payload"))?;
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.api_keys.len() != App::ARR.len()
            || App::ARR
                .iter()
                .any(|app| self.api_keys.get(app).is_none_or(|key| !valid_key(key)))
            || !valid_password(&self.putio_password)
            || self.putio_token.trim().is_empty()
            || self.putio_token.bytes().any(|b| b.is_ascii_whitespace())
        {
            return Err(Error::Invalid("credentials"));
        }
        Ok(())
    }
    pub fn key(&self, app: App) -> Result<&str, Error> {
        self.api_keys
            .get(&app)
            .map(String::as_str)
            .ok_or(Error::Invalid("missing application key"))
    }
}

pub fn valid_key(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
pub fn valid_password(value: &str) -> bool {
    (32..=256).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_alphanumeric())
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid media {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error(transparent)]
    Podman(#[from] skillet_podman::PodmanError),
    #[error("media runtime I/O failed")]
    Io(#[from] std::io::Error),
    #[error("{app} API {operation} failed (HTTP {status}); inspect the application journal")]
    Api {
        app: &'static str,
        operation: &'static str,
        status: u16,
    },
    #[error("{app} API {operation} failed; response details are suppressed because they may contain credentials")]
    Transport {
        app: &'static str,
        operation: &'static str,
    },
}

#[cfg(test)]
#[path = "tests.rs"]
mod fixtures;
