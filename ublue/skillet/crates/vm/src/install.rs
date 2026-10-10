//! Explicit root installation policy, independent of host and VM identity.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_yml::Value;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootProfile {
    #[default]
    Unencrypted,
    Tpm,
}

/// Transform a fresh install definition, never a mounted or populated disk.
pub fn configure_root(config: &mut Value, profile: RootProfile) -> Result<()> {
    if profile == RootProfile::Unencrypted {
        return Ok(());
    }
    let storage = config
        .get_mut("storage")
        .and_then(Value::as_mapping_mut)
        .ok_or_else(|| Error::Invalid("TPM installation requires storage configuration".into()))?;
    if storage.contains_key(Value::String("luks".into())) {
        return Err(Error::Invalid(
            "TPM profile conflicts with existing LUKS configuration".into(),
        ));
    }
    let filesystems = storage
        .get_mut(Value::String("filesystems".into()))
        .and_then(Value::as_sequence_mut)
        .ok_or_else(|| {
            Error::Invalid("TPM installation requires an explicit root filesystem".into())
        })?;
    let raw_root_count = filesystems
        .iter()
        .filter(|entry| {
            entry.get("device").and_then(Value::as_str) == Some("/dev/disk/by-partlabel/root")
        })
        .count();
    if raw_root_count != 1
        || filesystems
            .iter()
            .any(|entry| entry.get("device").and_then(Value::as_str) == Some("/dev/mapper/root"))
    {
        return Err(Error::Invalid(
            "conflicting root filesystem formatting declarations".into(),
        ));
    }
    let mut roots = filesystems
        .iter_mut()
        .filter(|entry| entry.get("label").and_then(Value::as_str) == Some("root"));
    let root = roots
        .next()
        .ok_or_else(|| Error::Invalid("root filesystem is absent".into()))?;
    if root.get("device").and_then(Value::as_str) != Some("/dev/disk/by-partlabel/root")
        || root.get("format").and_then(Value::as_str) != Some("btrfs")
    {
        return Err(Error::Invalid(
            "TPM profile requires the declared Btrfs root partition".into(),
        ));
    }
    let map = root
        .as_mapping_mut()
        .ok_or_else(|| Error::Invalid("invalid root filesystem".into()))?;
    map.insert(
        Value::String("device".into()),
        Value::String("/dev/mapper/root".into()),
    );
    if roots.next().is_some() {
        return Err(Error::Invalid("multiple root filesystems declared".into()));
    }
    let luks = serde_yml::to_value(serde_json::json!([{
        "name": "root", "label": "luks-root",
        "device": "/dev/disk/by-partlabel/root", "wipe_volume": true,
        "clevis": {"custom": {
            "needs_network": false, "pin": "tpm2",
            "config": serde_json::json!({"pcr_bank": "sha256", "pcr_ids": "7"}).to_string()
        }}
    }]))
    .map_err(|e| Error::Invalid(format!("invalid built-in TPM profile: {e}")))?;
    storage.insert(Value::String("luks".into()), luks);
    Ok(())
}

/// Butane requires file modes to be numeric. `serde_yml` parses YAML values such
/// as `0644` as strings, so convert octal mode strings before writing staged
/// configs back out.
pub fn normalize_butane_modes(value: &mut Value) -> Result<()> {
    match value {
        Value::Mapping(mapping) => {
            for (key, value) in mapping.iter_mut() {
                if key.as_str() == Some("mode") {
                    if let Some(mode) = value.as_str() {
                        let mode = u64::from_str_radix(mode, 8).map_err(|_| {
                            Error::Invalid(format!("invalid octal Butane file mode: {mode}"))
                        })?;
                        *value = Value::Number(mode.into());
                    }
                } else {
                    normalize_butane_modes(value)?;
                }
            }
        }
        Value::Sequence(values) => {
            for value in values {
                normalize_butane_modes(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn nvram_path(run: &crate::VmRun) -> Result<std::path::PathBuf> {
    Ok(run
        .disk
        .parent()
        .ok_or_else(|| Error::Invalid("disk has no parent".into()))?
        .join("nvram.fd"))
}
pub fn tpm_path(run: &crate::VmRun) -> Result<std::path::PathBuf> {
    Ok(run
        .disk
        .parent()
        .ok_or_else(|| Error::Invalid("disk has no parent".into()))?
        .join("tpm"))
}

#[cfg(test)]
#[path = "install/tests.rs"]
mod tests;

/// Select the whole disk for a fresh physical installation. VM templates remain unchanged.
pub fn configure_disk(config: &mut Value, disk: &str) -> Result<()> {
    validate_physical_disk(disk)?;
    let disks = config
        .get_mut("storage")
        .and_then(|v| v.get_mut("disks"))
        .and_then(Value::as_sequence_mut)
        .ok_or_else(|| {
            Error::Invalid("installation requires an explicit disk declaration".into())
        })?;
    if disks.len() != 1 || disks[0].get("device").and_then(Value::as_str) != Some("/dev/vda") {
        return Err(Error::Invalid(
            "expected one VM template disk; refusing ambiguous disk selection".into(),
        ));
    }
    disks[0]["device"] = Value::String(disk.into());
    Ok(())
}

/// Require a stable whole-disk identifier, not an ambient workstation block device.
pub fn validate_physical_disk(disk: &str) -> Result<()> {
    let Some(name) = disk.strip_prefix("/dev/disk/by-id/") else {
        return Err(Error::Invalid(
            "physical disk must use /dev/disk/by-id/<whole-disk-id>".into(),
        ));
    };
    if name.is_empty()
        || name.contains("-part")
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
        || name == "."
        || name == ".."
    {
        return Err(Error::Invalid("invalid whole-disk identifier".into()));
    }
    Ok(())
}
