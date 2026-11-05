//! Read-only validation of the booted `OSTree` entry on an owned test guest.
use skillet_cli_common::kexec::select_entry;
use std::{error::Error, fs, io, path::Path, process::Command};

fn main() -> Result<(), Box<dyn Error>> {
    let output = Command::new("rpm-ostree")
        .args(["status", "--json"])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("rpm-ostree status failed").into());
    }
    let status: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let deployments = status["deployments"]
        .as_array()
        .ok_or_else(|| io::Error::other("missing deployments"))?;
    let booted: Vec<_> = deployments
        .iter()
        .filter(|v| v["booted"].as_bool() == Some(true))
        .collect();
    if booted.len() != 1 {
        return Err(io::Error::other("ambiguous booted deployment").into());
    }
    let item = booted[0];
    let checksum = item["checksum"]
        .as_str()
        .ok_or_else(|| io::Error::other("missing checksum"))?;
    let osname = item["osname"]
        .as_str()
        .ok_or_else(|| io::Error::other("missing osname"))?;
    let serial = item["serial"]
        .as_u64()
        .ok_or_else(|| io::Error::other("missing serial"))?;
    if checksum.len() != 64
        || !checksum.bytes().all(|b| b.is_ascii_hexdigit())
        || !osname
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(io::Error::other("invalid deployment identity").into());
    }
    let expected = fs::canonicalize(format!(
        "/sysroot/ostree/deploy/{osname}/deploy/{checksum}.{serial}"
    ))?;
    let boot = fs::canonicalize("/boot")?;
    let mut contents = Vec::new();
    for file in fs::read_dir("/boot/loader/entries")? {
        let file = file?.path();
        if file.extension().is_none_or(|ext| ext != "conf") {
            continue;
        }
        contents.push(fs::read_to_string(file)?);
    }
    let entry = select_entry(
        contents.iter().map(String::as_str),
        Path::new("/boot"),
        &expected,
        |path| fs::canonicalize(path).ok(),
    )?;
    for artifact in [&entry.kernel, &entry.initrd] {
        let path = fs::canonicalize(artifact)?;
        if !path.starts_with(&boot) || !path.is_file() {
            return Err(io::Error::other("invalid boot artifact").into());
        }
    }
    println!("Verified the booted deployment has one matching BLS entry and valid boot artifacts");
    Ok(())
}
