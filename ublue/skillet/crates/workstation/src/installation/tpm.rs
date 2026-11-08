//! One-time physical TPM bootstrap. No host names or measured PCRs are built in.
use super::{Error, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use x509_cert::der::Decode as _;

mod enrollment;
mod eventlog;
mod runtime;
mod sealing;
pub use runtime::bootstrap;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Policy {
    pub initial: [u8; 32],
    pub target: [u8; 32],
    pub certificate_sha256: String,
}

/// Verify the current firmware log and predict shim's single added authority.
/// An already measured authority needs no additional enrollment.
pub fn predict_policy(log: &[u8], certificate: &[u8], observed: [u8; 32]) -> Result<Policy> {
    let event = authority_event(certificate)?;
    let replay = eventlog::replay(log, &event)?;
    if replay.pcr7 != observed || !replay.secure_boot {
        return Err(invalid(
            "firmware log does not match Secure Boot and the current SHA256 PCR 7",
        ));
    }
    Ok(Policy {
        initial: observed,
        target: if replay.authority_seen {
            observed
        } else {
            extend(observed, Sha256::digest(&event).into())
        },
        certificate_sha256: hex::encode(Sha256::digest(certificate)),
    })
}

fn invalid(message: &str) -> Error {
    Error::Invalid(format!("TPM bootstrap: {message}"))
}

fn extend(pcr: [u8; 32], event: [u8; 32]) -> [u8; 32] {
    Sha256::digest([pcr, event].concat()).into()
}

fn authority_event(certificate: &[u8]) -> Result<Vec<u8>> {
    x509_cert::Certificate::from_der(certificate)
        .map_err(|_| invalid("MOK must be a DER X.509 certificate"))?;
    // EFI_SHIM_LOCK_GUID, serialized in EFI little-endian GUID format.
    let shim = [
        0x50, 0xab, 0x5d, 0x60, 0x46, 0xe0, 0x00, 0x43, 0xab, 0xb6, 0x3d, 0xd8, 0x10, 0xdd, 0x8b,
        0x23,
    ];
    let name = "MokListRT";
    let length =
        u64::try_from(certificate.len()).map_err(|_| invalid("certificate is too large"))?;
    let mut data = Vec::new();
    data.extend(shim);
    data.extend(9_u64.to_le_bytes());
    data.extend((length + 16).to_le_bytes());
    for character in name.encode_utf16() {
        data.extend(character.to_le_bytes());
    }
    data.extend(shim);
    data.extend(certificate);
    Ok(data)
}

/// Only physical TPM media opts in; retained/plain/VM profiles are unchanged.
pub(super) fn configure_bootstrap(
    config: &mut yaml_serde::Value,
    certificate: &[u8],
) -> Result<()> {
    authority_event(certificate)?;
    let file = serde_json::json!({
        "path": "/etc/skillet-install/ublue.der", "mode": 420,
        "contents": {"source": format!("data:;base64,{}", base64::engine::general_purpose::STANDARD.encode(certificate))}
    });
    let files = config["storage"]["files"]
        .as_sequence_mut()
        .ok_or_else(|| invalid("physical source has no file declarations"))?;
    if files
        .iter()
        .any(|f| f["path"].as_str() == Some("/etc/skillet-install/ublue.der"))
    {
        return Err(invalid("conflicting bootstrap certificate declaration"));
    }
    files.push(yaml_serde::to_value(file)?);
    if config.get("systemd").is_none() {
        config["systemd"] = yaml_serde::to_value(serde_json::json!({"units": []}))?;
    } else if config["systemd"].get("units").is_none() {
        config["systemd"]["units"] = yaml_serde::Value::Sequence(Vec::new());
    }
    let units = config["systemd"]["units"]
        .as_sequence_mut()
        .ok_or_else(|| invalid("physical source has no unit declarations"))?;
    if units
        .iter()
        .any(|u| u["name"].as_str() == Some("skillet-tpm-bootstrap.service"))
    {
        return Err(invalid("conflicting TPM bootstrap unit"));
    }
    units.push(yaml_serde::to_value(serde_json::json!({
        "name": "skillet-tpm-bootstrap.service",
        "contents": include_str!("tpm/bootstrap.service")
    }))?);
    units.push(yaml_serde::to_value(serde_json::json!({
        "name": "ucore-bootstrap.service",
        "dropins": [{"name": "10-tpm-bootstrap.conf", "contents": "[Unit]\nRequires=skillet-tpm-bootstrap.service\nAfter=skillet-tpm-bootstrap.service\n"}]
    }))?);
    Ok(())
}

/// TPM2 PolicyPCR(SHA256:7), with a SHA256 policy session and no other commands.
fn policy_digest(pcr: [u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update([0; 32]);
    hash.update(0x0000_017f_u32.to_be_bytes()); // TPM_CC_PolicyPCR
    hash.update([0, 0, 0, 1, 0, 0x0b, 3, 0x80, 0, 0]); // TPML_PCR_SELECTION
    hash.update(Sha256::digest(pcr));
    hash.finalize().into()
}

fn header(jwe: &str) -> Result<serde_json::Value> {
    let encoded = jwe
        .split('.')
        .next()
        .ok_or_else(|| invalid("invalid sealed JWE"))?;
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|_| invalid("invalid sealed JWE header"))?;
    serde_json::from_slice(&bytes).map_err(|_| invalid("invalid sealed JWE JSON"))
}

fn validate_jwe(jwe: &str, expected: [u8; 32]) -> Result<()> {
    let value = header(jwe)?;
    let clevis = &value["clevis"];
    let config = &clevis["tpm2"];
    if clevis["pin"] != "tpm2"
        || config["pcr_bank"] != "sha256"
        || config["pcr_ids"] != "7"
        || config["hash"] != "sha256"
    {
        return Err(invalid("sealed key has a different TPM policy"));
    }
    let public = config["jwk_pub"]
        .as_str()
        .ok_or_else(|| invalid("missing TPM public object"))?;
    let bytes = URL_SAFE_NO_PAD
        .decode(public)
        .map_err(|_| invalid("invalid TPM public object"))?;
    // TPM2B_PUBLIC: size, type, nameAlg, objectAttributes, authPolicy size/data.
    if bytes.len() < 44
        || usize::from(u16::from_be_bytes([bytes[0], bytes[1]])) != bytes.len() - 2
        || bytes[2..6] != [0, 8, 0, 0x0b]
        || bytes[10..12] != [0, 32]
        || bytes[12..44] != policy_digest(expected)
    {
        return Err(invalid(
            "TPM public object does not bind the approved PCR 7 value",
        ));
    }
    let attributes = u32::from_be_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);
    // An empty-auth user authorization must not bypass the PCR policy.
    if attributes & 0x40 != 0 || attributes & 0x92 != 0x92 {
        return Err(invalid(
            "TPM public object permits an unexpected authorization path",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "tpm/tests.rs"]
mod tests;
