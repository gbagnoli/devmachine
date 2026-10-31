//! Produce the standard Clevis TPM2 envelope without relying on pin versions
//! that silently ignore supplied future PCR values.
use super::{
    invalid, policy_digest,
    runtime::{keyfile, tool},
    validate_jwe, Result,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use std::{fs, path::Path};
use zeroize::Zeroizing;

pub(super) fn seal(key: &[u8], target: [u8; 32]) -> Result<String> {
    // keyfile verifies /run is tmpfs before any plaintext is materialized.
    let plaintext = keyfile(key)?;
    let work = tempfile::Builder::new()
        .prefix("skillet-tpm-seal-")
        .tempdir_in("/run")?;
    let jwk = work.path().join("key.jwk");
    let jwk_bytes =
        Zeroizing::new(tool("jose", &["jwk", "gen", "-i", "{\"alg\":\"A256GCM\"}"], None)?.stdout);
    super::super::write_private(&jwk, &jwk_bytes)?;
    let policy = work.path().join("policy.bin");
    super::super::write_private(&policy, &policy_digest(target))?;
    let primary = work.path().join("primary.ctx");
    let public = work.path().join("object.pub");
    let private = work.path().join("object.priv");
    tool(
        "tpm2_createprimary",
        &[
            "-Q",
            "-C",
            "o",
            "-g",
            "sha256",
            "-G",
            "ecc",
            "-c",
            text(&primary)?,
        ],
        None,
    )?;
    tool(
        "tpm2_create",
        &[
            "-Q",
            "-C",
            text(&primary)?,
            "-g",
            "sha256",
            "-a",
            "fixedtpm|fixedparent|noda|adminwithpolicy",
            "-L",
            text(&policy)?,
            "-i",
            text(&jwk)?,
            "-u",
            text(&public)?,
            "-r",
            text(&private)?,
        ],
        None,
    )?;
    let header = serde_json::json!({"protected":{"clevis":{"pin":"tpm2","tpm2":{
        "hash":"sha256", "key":"ecc", "pcr_bank":"sha256", "pcr_ids":"7",
        "jwk_pub":URL_SAFE_NO_PAD.encode(fs::read(public)?),
        "jwk_priv":URL_SAFE_NO_PAD.encode(fs::read(private)?)
    }}}});
    let header_path = work.path().join("header.json");
    super::super::write_private(
        &header_path,
        &serde_json::to_vec(&header)
            .map_err(|_| invalid("cannot serialize the sealed-key header"))?,
    )?;
    let output = tool(
        "jose",
        &[
            "jwe",
            "enc",
            "-i",
            text(&header_path)?,
            "-k",
            text(&jwk)?,
            "-I",
            text(plaintext.path())?,
            "-c",
        ],
        None,
    )?;
    let jwe = String::from_utf8(output.stdout).map_err(|_| invalid("invalid sealed JWE output"))?;
    let jwe = jwe.trim().to_owned();
    validate_jwe(&jwe, target)?;
    Ok(jwe)
}

fn text(path: &Path) -> Result<&str> {
    path.to_str().ok_or_else(|| invalid("invalid tmpfs path"))
}
