//! Converge the opt-in activation definition without requesting a reboot.
use skillet_core::{
    files::{FileError, FileMutationResource, FileReadResource, Ownership},
    system::{ServiceResource, SystemError},
};
use std::path::Path;

pub const UNIT: &str = "skillet-kexec-activate.service";
pub const PATH: &str = "/etc/systemd/system/skillet-kexec-activate.service";
pub const TIMER: &str = "skillet-kexec-activate.timer";
pub const REBOOT_TIMER: &str = "skillet-firmware-reboot.timer";

const DAILY: &str = "[Unit]\nDescription=Daily Skillet kexec activation except first Sunday\n[Timer]\nOnCalendar=Mon..Sat *-*-01..07 05:30:00 Europe/Paris\nOnCalendar=*-*-08..31 05:30:00 Europe/Paris\nRandomizedDelaySec=0\nPersistent=false\n[Install]\nWantedBy=timers.target\n";
const MONTHLY: &str = "[Unit]\nDescription=Monthly normal firmware reboot\n[Timer]\nOnCalendar=Sun *-*-01..07 05:30:00 Europe/Paris\nRandomizedDelaySec=0\nPersistent=false\n[Install]\nWantedBy=timers.target\n";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Vendor(#[from] skillet_core::vendor_units::Error),
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error("activation executable must be an absolute path with safe unit characters")]
    InvalidExecutable,
}

pub fn render(binary: &str) -> Result<String, Error> {
    if !binary.starts_with('/')
        || !binary
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
    {
        return Err(Error::InvalidExecutable);
    }
    Ok(format!("[Unit]\nDescription=Controlled Skillet kexec activation\nDefaultDependencies=no\nAfter=local-fs.target\n[Service]\nType=exec\nPrivateMounts=yes\nExecStart={binary} kexec run\nExecStopPost={binary} kexec cleanup\nRuntimeMaxSec=120\nTimeoutStopSec=20\nRestart=no\n"))
}

pub fn image_units() -> Result<Vec<skillet_core::vendor_units::Unit>, Error> {
    units("/usr/bin/skillet")
}

fn units(binary: &str) -> Result<Vec<skillet_core::vendor_units::Unit>, Error> {
    use skillet_core::vendor_units::Unit;
    Ok(vec![
        Unit { name: UNIT, content: render(binary)? },
        Unit { name: TIMER, content: DAILY.into() },
        Unit { name: REBOOT_TIMER, content: MONTHLY.into() },
        Unit { name: "skillet-firmware-reboot.service", content: format!("[Unit]\nDescription=Monthly Skillet firmware reboot\n[Service]\nType=exec\nExecStart={binary} kexec firmware-reboot\nRuntimeMaxSec=120\nRestart=no\n") },
    ])
}

pub fn apply<S, F>(system: &S, files: &F, binary: &str) -> Result<(), Error>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let vendor = image_units()?;
    let legacy = units(binary)?;
    let receipt = skillet_core::vendor_units::LegacyReceipt {
        path: "/var/lib/skillet/kexec/definition.applied".into(),
        revisions: vec![
            legacy
                .iter()
                .flat_map(|unit| unit.content.as_bytes())
                .copied()
                .collect(),
            render(binary)?.into_bytes(),
        ],
    };
    if skillet_core::vendor_units::reconcile(
        system,
        files,
        "kexec",
        &vendor,
        &legacy,
        Some(&receipt),
    )? {
        for timer in [TIMER, REBOOT_TIMER] {
            system.service_enable(timer)?;
            system.service_start(timer)?;
        }
        return Ok(());
    }
    let owner = Ownership::named(Some("root"), Some("root"));
    files.ensure_directory(Path::new("/etc/systemd/system"), Some(0o755), &owner)?;
    let mut changed = false;
    for unit in &legacy {
        changed |= files.ensure_file(
            &Path::new("/etc/systemd/system").join(unit.name),
            unit.content.as_bytes(),
            Some(0o644),
            &owner,
        )?;
    }
    let state = Path::new("/var/lib/skillet/kexec/definition.applied");
    let receipt = legacy
        .iter()
        .map(|unit| unit.content.as_str())
        .collect::<String>();
    if changed || files.read_file(state)?.as_deref() != Some(receipt.as_bytes()) {
        system.daemon_reload()?;
        files.ensure_directory(Path::new("/var/lib/skillet/kexec"), Some(0o755), &owner)?;
        files.ensure_file(state, receipt.as_bytes(), Some(0o644), &owner)?;
    }
    for timer in [TIMER, REBOOT_TIMER] {
        if changed {
            system.service_reenable(timer)?;
        } else {
            system.service_enable(timer)?;
        }
        system.service_start(timer)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "service/tests.rs"]
mod tests;
