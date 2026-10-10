//! Native fleet SMTP relay configuration and volatile credential preparation.
use askama::Template;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use skillet_core::{
    activation::{self, ActivationRequest, ConsumerKind},
    files::{FileError, FileMutationResource, FileReadResource, Ownership},
    system::{ServiceResource, SystemError},
};
use std::path::Path;
use thiserror::Error;

pub const MONITORED_UNITS: &[&str] = &["postfix.service", "skillet-smtp-prepare.service"];
pub const CREDENTIAL: &str = "smtp_config";
pub const MAP_PATH: &str = "/run/postfix/skillet/sasl_passwd";
const STATE_DIR: &str = "/var/lib/skillet/smtp";

#[derive(Debug, Error)]
pub enum SmtpError {
    #[error("invalid SMTP configuration; check required fields and security policy")]
    Invalid,
    #[error("refusing to switch SMTP environment on a retained queue")]
    EnvironmentChange,
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error(transparent)]
    Activation(#[from] activation::ActivationError),
    #[error("SMTP configuration rendering failed")]
    Render,
}

// Intentionally no Debug: production input contains credentials.
#[derive(Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase", deny_unknown_fields)]
pub enum Input {
    Production {
        host: String,
        port: u16,
        tls: String,
        username: String,
        password: String,
        sender: String,
    },
    Capture {},
}
impl Input {
    pub fn parse(payload: &str) -> Result<Self, SmtpError> {
        let input: Self = serde_json::from_str(payload).map_err(|_| SmtpError::Invalid)?;
        input.validate()?;
        Ok(input)
    }
    pub fn validate(&self) -> Result<(), SmtpError> {
        if let Self::Production {
            host,
            port,
            tls,
            username,
            password,
            sender,
        } = self
        {
            if !valid_host(host)
                || *port == 0
                || tls != "starttls"
                || !valid_login(username)
                || username.contains(':')
                || !valid_login(password)
                || !valid_sender(sender)
            {
                return Err(SmtpError::Invalid);
            }
        }
        Ok(())
    }
    pub fn payload(&self) -> Result<String, SmtpError> {
        self.validate()?;
        serde_json::to_string(self).map_err(|_| SmtpError::Invalid)
    }
    fn mode(&self) -> &'static str {
        match self {
            Self::Production { .. } => "production",
            Self::Capture {} => "capture",
        }
    }
    pub fn main_config(&self) -> Result<String, SmtpError> {
        self.validate()?;
        let (hostname, relay, auth, tls, password_map, sender_map) = match self {
            Self::Production {
                host, port, sender, ..
            } => (
                sender
                    .split_once('@')
                    .ok_or(SmtpError::Invalid)?
                    .1
                    .to_string(),
                format!("[{host}]:{port}"),
                "yes",
                "secure",
                format!("texthash:{MAP_PATH}"),
                format!("static:{sender}"),
            ),
            Self::Capture {} => (
                "smtp-capture.invalid".into(),
                "[127.0.0.1]:1025".into(),
                "no",
                "none",
                String::new(),
                String::new(),
            ),
        };
        MainConfig {
            hostname,
            relay,
            auth,
            tls,
            password_map,
            sender_map,
        }
        .render()
        .map_err(|_| SmtpError::Render)
    }
}
#[derive(Template)]
#[template(path = "main.cf", escape = "none")]
struct MainConfig<'a> {
    hostname: String,
    relay: String,
    auth: &'a str,
    tls: &'a str,
    password_map: String,
    sender_map: String,
}
fn valid_login(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_graphic())
}
fn valid_host(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value.split('.').all(|s| {
            !s.is_empty()
                && s.len() <= 63
                && !s.starts_with('-')
                && !s.ends_with('-')
                && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}
pub fn valid_sender(value: &str) -> bool {
    value.split_once('@').is_some_and(|(local, host)| {
        !local.is_empty()
            && local
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b))
            && valid_host(host)
    })
}

/// Prepare only tmpfs files; never start a service from its own prerequisite.
pub fn prepare<F: FileMutationResource + FileReadResource + ?Sized>(
    files: &F,
    input: &Input,
) -> Result<(), SmtpError> {
    input.validate()?;
    if files
        .read_file(&Path::new(STATE_DIR).join("mode"))?
        .is_some_and(|mode| mode != input.mode().as_bytes())
    {
        return Err(SmtpError::EnvironmentChange);
    }
    files.ensure_directory(Path::new("/run/postfix"), None, &Ownership::default())?;
    files.ensure_directory(
        Path::new("/run/postfix/skillet"),
        Some(0o750),
        &Ownership::named(Some("root"), Some("postfix")),
    )?;
    let map = match input {
        Input::Production {
            host,
            port,
            username,
            password,
            ..
        } => format!("[{host}]:{port} {username}:{password}\n"),
        Input::Capture {} => String::new(),
    };
    files.ensure_file(
        Path::new(MAP_PATH),
        map.as_bytes(),
        Some(0o640),
        &Ownership::named(Some("root"), Some("postfix")),
    )?;
    Ok(())
}

pub fn apply<S, F>(system: &S, files: &F, payload: &str) -> Result<(), SmtpError>
where
    S: ServiceResource + ?Sized,
    F: FileReadResource + FileMutationResource + ?Sized,
{
    let input = Input::parse(payload)?;
    let mode_path = Path::new(STATE_DIR).join("mode");
    if files
        .read_file(&mode_path)?
        .is_some_and(|old| old != input.mode().as_bytes())
    {
        return Err(SmtpError::EnvironmentChange);
    }
    files.ensure_directory(
        Path::new(STATE_DIR),
        Some(0o700),
        &Ownership::named(Some("root"), Some("root")),
    )?;
    // Persist the environment guard before touching configuration or activating a queue.
    files.ensure_file(
        &mode_path,
        input.mode().as_bytes(),
        Some(0o600),
        &Ownership::named(Some("root"), Some("root")),
    )?;
    initialize_persistent_state(files)?;
    files.ensure_directory(Path::new("/etc/postfix"), None, &Ownership::default())?;
    files.ensure_directory(
        Path::new("/etc/systemd/system/postfix.service.d"),
        Some(0o755),
        &Ownership::named(Some("root"), Some("root")),
    )?;
    let config = input.main_config()?;
    let mut changed = files.ensure_file(
        Path::new("/etc/postfix/main.cf"),
        config.as_bytes(),
        Some(0o644),
        &Ownership::named(Some("root"), Some("root")),
    )?;
    changed |= files.ensure_file(
        Path::new("/etc/systemd/system/postfix.service.d/skillet.conf"),
        include_bytes!("postfix.conf"),
        Some(0o644),
        &Ownership::named(Some("root"), Some("root")),
    )?;
    changed |= files.ensure_file(
        Path::new("/etc/systemd/system/skillet-smtp-prepare.service"),
        include_bytes!("prepare.service"),
        Some(0o644),
        &Ownership::named(Some("root"), Some("root")),
    )?;
    if changed {
        system.daemon_reload()?;
    }
    // A fresh decrypt/map preparation is required on every credential redelivery.
    let revision = hex::encode(Sha256::digest(payload.as_bytes())).into_bytes();
    if changed
        || files
            .read_file(&Path::new(STATE_DIR).join("applied"))?
            .as_deref()
            != Some(revision.as_slice())
        || !system.service_is_active("postfix.service")?
    {
        system.service_restart("skillet-smtp-prepare.service")?;
    }
    system.service_enable("postfix.service")?;
    activation::activate(
        system,
        files,
        &ActivationRequest {
            service: "postfix.service".into(),
            state_path: Path::new(STATE_DIR).join("applied"),
            revision,
            definition_changed: changed,
            reload_daemon: false,
            consumer_kind: ConsumerKind::Persistent,
        },
    )?;
    Ok(())
}

// Image-layer /var paths need explicit initialization before confined startup.
fn initialize_persistent_state<F: FileMutationResource + ?Sized>(
    files: &F,
) -> Result<(), SmtpError> {
    let queue = Path::new("/var/spool/postfix");
    for path in [queue.to_path_buf(), queue.join("pid")] {
        files.ensure_directory(
            &path,
            Some(0o755),
            &Ownership::named(Some("root"), Some("root")),
        )?;
    }
    for name in [
        "active", "bounce", "corrupt", "defer", "deferred", "flush", "hold", "incoming", "private",
        "saved", "trace",
    ] {
        files.ensure_directory(
            &queue.join(name),
            Some(0o700),
            &Ownership::named(Some("postfix"), Some("root")),
        )?;
    }
    for (name, mode) in [("maildrop", 0o730), ("public", 0o710)] {
        files.ensure_directory(
            &queue.join(name),
            Some(mode),
            &Ownership::named(Some("postfix"), Some("postdrop")),
        )?;
    }
    files.ensure_directory(
        Path::new("/var/lib/postfix"),
        Some(0o700),
        &Ownership::named(Some("postfix"), Some("root")),
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
