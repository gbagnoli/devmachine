//! Capability parsing for explicitly requested UEFI/TPM root installations.
use crate::{Error, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct Caps {
    os: Os,
    devices: Devices,
}
#[derive(Deserialize)]
struct Os {
    #[serde(rename = "enum", default)]
    enums: Vec<Enumeration>,
    loader: Loader,
}
#[derive(Deserialize)]
struct Loader {
    #[serde(rename = "enum", default)]
    enums: Vec<Enumeration>,
}
#[derive(Deserialize)]
struct Devices {
    tpm: Tpm,
}
#[derive(Deserialize)]
struct Tpm {
    #[serde(rename = "@supported")]
    supported: String,
    #[serde(rename = "enum", default)]
    enums: Vec<Enumeration>,
}
#[derive(Deserialize)]
struct Enumeration {
    #[serde(rename = "@name")]
    name: String,
    #[serde(rename = "value", default)]
    values: Vec<String>,
}
fn has(values: &[Enumeration], name: &str, value: &str) -> bool {
    values
        .iter()
        .any(|e| e.name == name && e.values.iter().any(|v| v == value))
}
pub fn verify_capabilities(xml: &str, version: &str) -> Result<()> {
    let caps: Caps = quick_xml::de::from_str(xml)?;
    if !has(&caps.os.enums, "firmware", "efi")
        || !has(&caps.os.loader.enums, "secure", "yes")
        || caps.devices.tpm.supported != "yes"
        || !has(&caps.devices.tpm.enums, "backendModel", "emulator")
        || !has(&caps.devices.tpm.enums, "backendVersion", "2.0")
    {
        return Err(Error::Invalid(
            "TPM profile requires q35 UEFI Secure Boot and emulated TPM2 support".into(),
        ));
    }
    let daemon = version
        .lines()
        .find_map(|line| line.strip_prefix("Running against daemon: "))
        .ok_or_else(|| {
            Error::Invalid("cannot establish libvirt daemon version for owned TPM state".into())
        })?;
    let numbers = daemon
        .trim()
        .split('.')
        .map(str::parse::<u32>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| Error::Invalid("invalid libvirt daemon version".into()))?;
    if numbers.len() < 2 || (numbers[0], numbers[1]) < (10, 10) {
        return Err(Error::Invalid(
            "owned TPM state requires libvirt daemon 10.10 or newer".into(),
        ));
    }
    Ok(())
}
#[cfg(test)]
#[path = "firmware/tests.rs"]
mod tests;
