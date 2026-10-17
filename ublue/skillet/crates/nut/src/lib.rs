//! Optional native, loopback-only USB UPS protection.
use sha2::{Digest as _, Sha256};
use skillet_core::{
    activation::{activate, ActivationRequest, ConsumerKind},
    files::{FileError, FileMutationResource, FileReadResource, Ownership},
    system::{ServiceResource, SystemError},
    usb::UsbResource,
};
use std::path::Path;
use thiserror::Error;

pub mod metrics;
mod monitoring;
pub use monitoring::{apply_metrics, METRICS_TIMER};

pub const MONITORED_UNITS: &[&str] = &["nut-server.service", "nut-monitor.service"];
const STATE: &str = "/var/lib/skillet/nut";
const PASSWORD: &str = "/var/lib/skillet/nut/monitor-password";

pub fn monitored_units(config: Config) -> Vec<String> {
    MONITORED_UNITS
        .iter()
        .map(|unit| (*unit).to_string())
        .chain(std::iter::once(format!(
            "nut-driver@{}.service",
            config.name
        )))
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    pub name: &'static str,
    pub vendor: u16,
    pub product: u16,
    pub description: &'static str,
}

#[derive(Debug, Error)]
pub enum NutError {
    #[error("invalid UPS declaration")]
    InvalidConfig,
    #[error("invalid retained NUT monitor credential; refusing replacement")]
    InvalidCredential,
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error(transparent)]
    Activation(#[from] skillet_core::activation::ActivationError),
}

#[derive(Debug, Eq, PartialEq)]
pub enum Outcome {
    ConfiguredWithoutDevice,
    Active,
}

impl Config {
    fn validate(self) -> Result<(), NutError> {
        if metrics::validate_name(self.name).is_err()
            || self.description.is_empty()
            || !self
                .description
                .bytes()
                .all(|b| b.is_ascii_graphic() && b != b'"' && b != b'\\' || b == b' ')
        {
            return Err(NutError::InvalidConfig);
        }
        Ok(())
    }
    pub fn ups_config(self) -> Result<String, NutError> {
        self.validate()?;
        Ok(format!("[{}]\n    driver = usbhid-ups\n    port = auto\n    vendorid = {:04x}\n    productid = {:04x}\n    desc = \"{}\"\n", self.name, self.vendor, self.product, self.description))
    }
}

pub fn apply<S, F>(system: &S, files: &F, config: Config) -> Result<Outcome, NutError>
where
    S: ServiceResource + UsbResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    config.validate()?;
    let root = Ownership::named(Some("root"), Some("root"));
    let nut = Ownership::named(Some("root"), Some("nut"));
    files.ensure_directory(Path::new(STATE), Some(0o700), &root)?;
    let password = monitor_password(files)?;
    files.ensure_directory(Path::new("/etc/ups"), Some(0o750), &nut)?;
    let ups = config.ups_config()?;
    let server = "LISTEN 127.0.0.1 3493\nLISTEN ::1 3493\n";
    let users = format!("[skillet-monitor]\n    password = {password}\n    upsmon primary\n");
    let monitor = format!("MONITOR {}@127.0.0.1 1 skillet-monitor {password} primary\nMINSUPPLIES 1\nSHUTDOWNCMD \"/usr/bin/systemctl poweroff\"\nPOWERDOWNFLAG /run/nut/killpower\nRUN_AS_USER nut\nPOLLFREQ 5\nPOLLFREQALERT 5\nDEADTIME 15\nFINALDELAY 5\nNOTIFYFLAG ONLINE SYSLOG+WALL\nNOTIFYFLAG ONBATT SYSLOG+WALL\nNOTIFYFLAG LOWBATT SYSLOG+WALL\nNOTIFYFLAG COMMBAD SYSLOG\nNOTIFYFLAG COMMOK SYSLOG\nNOTIFYFLAG SHUTDOWN SYSLOG+WALL\n", config.name);
    let mut changed = false;
    for (name, content) in [
        ("nut.conf", "MODE=standalone\n"),
        ("ups.conf", &ups),
        ("upsd.conf", server),
        ("upsd.users", &users),
        ("upsmon.conf", &monitor),
    ] {
        changed |= files.ensure_file(
            &Path::new("/etc/ups").join(name),
            content.as_bytes(),
            Some(0o640),
            &nut,
        )?;
    }
    let driver = format!("nut-driver@{}.service", config.name);
    let units = unit_dropins(config, &driver);
    for (unit, dropin) in &units {
        let dir = Path::new("/etc/systemd/system").join(format!("{unit}.d"));
        files.ensure_directory(&dir, Some(0o755), &root)?;
        changed |= files.ensure_file(
            &dir.join("skillet.conf"),
            dropin.as_bytes(),
            Some(0o644),
            &root,
        )?;
    }
    files.ensure_directory(Path::new("/etc/udev/rules.d"), Some(0o755), &root)?;
    let rule = format!("SUBSYSTEM==\"usb\", ENV{{DEVTYPE}}==\"usb_device\", ATTR{{idVendor}}==\"{:04x}\", ATTR{{idProduct}}==\"{:04x}\", SYMLINK+=\"skillet-ups-{}\", TAG+=\"systemd\", ENV{{SYSTEMD_WANTS}}+=\"nut-monitor.service\"\n", config.vendor, config.product, config.name);
    let rule_changed = files.ensure_file(
        Path::new("/etc/udev/rules.d/70-skillet-ups.rules"),
        rule.as_bytes(),
        Some(0o644),
        &root,
    )?;
    changed |= rule_changed;
    if changed {
        system.daemon_reload()?;
    }
    for unit in [
        &driver,
        "nut-driver.target",
        "nut-server.service",
        "nut-monitor.service",
        "nut.target",
    ] {
        system.service_enable(unit)?;
    }
    // Keep refresh pending until it succeeds, even if a previous apply wrote the rule.
    let rule_revision = Sha256::digest(rule.as_bytes()).to_vec();
    let rule_state = Path::new(STATE).join("udev.applied");
    if files.read_file(&rule_state)?.as_deref() != Some(rule_revision.as_slice()) || rule_changed {
        system.refresh_usb_rules(config.vendor, config.product)?;
        files.ensure_file(&rule_state, &rule_revision, Some(0o600), &root)?;
    }
    if !system.usb_device_present(config.vendor, config.product)? {
        // Never stop an existing monitor after unplug: its last on-battery state
        // may require a protective shutdown on loss of communication.
        tracing::info!(
            ups = config.name,
            "NUT configuration converged; matching USB UPS is not attached"
        );
        return Ok(Outcome::ConfiguredWithoutDevice);
    }
    let mut hash = Sha256::new();
    for value in [&ups, server, &users, &monitor, &rule] {
        hash.update(value);
    }
    for (unit, content) in &units {
        hash.update(unit);
        hash.update(content);
    }
    let revision = hash.finalize().to_vec();
    for unit in [&driver, "nut-server.service", "nut-monitor.service"] {
        activate(
            system,
            files,
            &ActivationRequest {
                service: unit.to_string(),
                state_path: Path::new(STATE).join(format!("{unit}.applied")),
                revision: revision.clone(),
                definition_changed: changed,
                reload_daemon: true,
                consumer_kind: ConsumerKind::Persistent,
            },
        )?;
    }
    Ok(Outcome::Active)
}

fn monitor_password<F: FileMutationResource + FileReadResource + ?Sized>(
    files: &F,
) -> Result<String, NutError> {
    let root = Ownership::named(Some("root"), Some("root"));
    let password = match files.read_file(Path::new(PASSWORD))? {
        Some(bytes) => String::from_utf8(bytes).map_err(|_| NutError::InvalidCredential)?,
        None => uuid::Uuid::new_v4().simple().to_string(),
    };
    if password.len() != 32 || !password.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(NutError::InvalidCredential);
    }
    // Persist first; interrupted configuration resumes with the same credential.
    files.ensure_file(Path::new(PASSWORD), password.as_bytes(), Some(0o600), &root)?;
    Ok(password)
}

fn unit_dropins(config: Config, driver: &str) -> Vec<(String, String)> {
    let condition = format!("ConditionPathExists=/dev/skillet-ups-{}\n", config.name);
    [(driver, "Wants=nut-udev-settle.service\nAfter=nut-udev-settle.service\n".into()),
     ("nut-server.service", format!("Requires={driver}\nAfter={driver}\n")),
     ("nut-monitor.service", "Requires=nut-server.service\nAfter=nut-server.service\n".into())]
        .into_iter().map(|(unit, dependencies)| (unit.to_string(),
            format!("[Unit]\n{condition}{dependencies}\n[Service]\nExecStartPre=/usr/sbin/restorecon -RF /etc/ups\n")))
        .collect()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
