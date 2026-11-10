//! Validated boot-entry selection for controlled kexec activation.
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

pub mod attempt;
pub mod runtime;
pub mod service;

/// Report whether rpm-ostree has an update selected for the next boot.
/// This includes a staged deployment and a deployment finalized by an earlier
/// attempt that was deferred before kexec could run.
pub fn has_activation_target(status: &serde_json::Value) -> Result<bool, Error> {
    let deployments = status["deployments"]
        .as_array()
        .ok_or(Error::InvalidDeployment)?;
    let staged = deployments
        .iter()
        .filter(|item| item["staged"] == true)
        .count();
    if staged > 1 {
        return Err(Error::InvalidDeployment);
    }
    if staged == 1 {
        return Ok(true);
    }
    if deployments
        .iter()
        .filter(|item| item["booted"] == true)
        .count()
        != 1
    {
        return Err(Error::InvalidDeployment);
    }
    let Some(target) = deployments.first() else {
        return Ok(false);
    };
    if target["booted"] != false {
        return Ok(false);
    }
    let booted = deployments
        .iter()
        .find(|item| item["booted"] == true)
        .ok_or(Error::InvalidDeployment)?;
    let target_timestamp = target["timestamp"]
        .as_u64()
        .ok_or(Error::InvalidDeployment)?;
    let booted_timestamp = booted["timestamp"]
        .as_u64()
        .ok_or(Error::InvalidDeployment)?;
    Ok(target_timestamp > booted_timestamp)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationAction {
    Noop,
    Kexec,
    NormalReboot,
}

/// Choose one activation after staging: a monthly normal reboot owns the first
/// Sunday, while other days reboot only when an update is staged.
pub fn activation_action(
    status: &serde_json::Value,
    now: time::OffsetDateTime,
) -> Result<ActivationAction, Error> {
    let pending = has_activation_target(status)?;
    if is_monthly_reboot_day(now) {
        Ok(ActivationAction::NormalReboot)
    } else if pending {
        Ok(ActivationAction::Kexec)
    } else {
        Ok(ActivationAction::Noop)
    }
}

/// The monthly normal firmware reboot is scheduled on the first Sunday.
fn is_monthly_reboot_day(now: time::OffsetDateTime) -> bool {
    now.weekday() == time::Weekday::Sunday && now.day() <= 7
}

/// Select the current default activation target from a fresh `rpm-ostree`
/// observation. This may be newly staged or already finalized by an earlier
/// attempt. Selection must be repeated under staging exclusion before loading
/// a kernel.
pub fn activation_target(status: &serde_json::Value) -> Result<&serde_json::Value, Error> {
    if status
        .get("transaction")
        .is_none_or(|value| !value.is_null())
    {
        return Err(Error::InvalidDeployment);
    }
    let deployments = status["deployments"]
        .as_array()
        .ok_or(Error::InvalidDeployment)?;
    if deployments
        .iter()
        .filter(|item| item["booted"] == true)
        .count()
        != 1
        || deployments
            .iter()
            .filter(|item| item["staged"] == true)
            .count()
            > 1
    {
        return Err(Error::InvalidDeployment);
    }
    let staged: Vec<_> = deployments
        .iter()
        .filter(|item| item["staged"] == true)
        .collect();
    if staged.len() > 1 {
        return Err(Error::InvalidDeployment);
    }
    if let Some(target) = staged.first() {
        return Ok(target);
    }
    let target = deployments.first().ok_or(Error::InvalidDeployment)?;
    let booted = deployments
        .iter()
        .find(|item| item["booted"] == true)
        .ok_or(Error::InvalidDeployment)?;
    let target_timestamp = target["timestamp"]
        .as_u64()
        .ok_or(Error::InvalidDeployment)?;
    let booted_timestamp = booted["timestamp"]
        .as_u64()
        .ok_or(Error::InvalidDeployment)?;
    if target["booted"] != false || target_timestamp <= booted_timestamp {
        return Err(Error::InvalidDeployment);
    }
    Ok(target)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("boot entry lacks a required field or contains a duplicate")]
    InvalidEntry,
    #[error("boot entry has an unsafe or unsupported path")]
    InvalidPath,
    #[error("boot entry does not identify exactly one matching deployment")]
    InvalidDeployment,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BootEntry {
    pub kernel: PathBuf,
    pub initrd: PathBuf,
    pub options: String,
    pub deployment_selector: PathBuf,
}

/// Parse a BLS entry as data; arguments are never interpreted by a shell.
/// The deployment selector is resolved and compared by the caller against the
/// exact `OSTree` deployment identity, including its deployment serial.
pub fn parse_entry(content: &str, boot: &Path) -> Result<BootEntry, Error> {
    let mut kernel = None;
    let mut initrd = None;
    let mut options = None;
    for line in content.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let value = value.trim();
        let slot = match key {
            "linux" => &mut kernel,
            "initrd" => &mut initrd,
            "options" => &mut options,
            _ => continue,
        };
        if value.is_empty() || slot.replace(value).is_some() {
            return Err(Error::InvalidEntry);
        }
    }
    let options = options.ok_or(Error::InvalidEntry)?;
    if options.contains(['\0', '\r', '\n']) {
        return Err(Error::InvalidEntry);
    }
    let mut selectors = options
        .split_ascii_whitespace()
        .filter_map(|item| item.strip_prefix("ostree="));
    let selector = selectors.next().ok_or(Error::InvalidDeployment)?;
    if selectors.next().is_some() {
        return Err(Error::InvalidDeployment);
    }
    let selector = Path::new(selector);
    if !selector.is_absolute()
        || !selector.to_string_lossy().starts_with("/ostree/boot.")
        || selector
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(Error::InvalidDeployment);
    }
    Ok(BootEntry {
        kernel: boot_path(boot, kernel.ok_or(Error::InvalidEntry)?)?,
        initrd: boot_path(boot, initrd.ok_or(Error::InvalidEntry)?)?,
        options: options.to_owned(),
        deployment_selector: selector.to_owned(),
    })
}

/// Select exactly one entry whose resolved selector is the intended deployment.
/// The caller obtains the expected canonical path from trusted `OSTree` status.
pub fn select_entry<'a>(
    entries: impl IntoIterator<Item = &'a str>,
    boot: &Path,
    expected: &Path,
    resolve: impl Fn(&Path) -> Option<PathBuf>,
) -> Result<BootEntry, Error> {
    let mut selected = None;
    for content in entries {
        let entry = parse_entry(content, boot)?;
        let actual = resolve(&entry.deployment_selector).ok_or(Error::InvalidDeployment)?;
        if actual == expected && selected.replace(entry).is_some() {
            return Err(Error::InvalidDeployment);
        }
    }
    selected.ok_or(Error::InvalidDeployment)
}

fn boot_path(boot: &Path, value: &str) -> Result<PathBuf, Error> {
    let path = Path::new(value);
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        || value.contains(['\0', '\r', '\n'])
    {
        return Err(Error::InvalidPath);
    }
    let relative = path.strip_prefix("/").map_err(|_| Error::InvalidPath)?;
    Ok(boot.join(relative))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
