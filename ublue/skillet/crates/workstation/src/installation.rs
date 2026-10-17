//! Physical install media preparation. Never opens or writes a host block device.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_yml::Value;
use sha2::{Digest as _, Sha256};
use skillet_vm::install::RootProfile;
use std::{
    fs,
    io::Write as _,
    os::unix::fs::PermissionsExt as _,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid installation request: {0}")]
    Invalid(String),
    #[error("installation artifact I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("installation YAML: {0}")]
    Yaml(#[from] serde_yml::Error),
    #[error("installation tooling: {0}")]
    Tool(#[from] skillet_vm::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

pub struct Request<'a> {
    pub host: &'a str,
    pub disk: &'a str,
    pub root_profile: RootProfile,
    pub iso: &'a Path,
    pub output: &'a Path,
    pub public_key: &'a Path,
    pub password_hash: &'a Path,
    pub mok_certificate: Option<&'a Path>,
    pub butane: &'a Path,
    pub workspace: &'a Path,
}

/// Prepare an explicitly new private bundle. Existing bundles are never overwritten.
pub fn prepare(request: &Request<'_>) -> Result<PathBuf> {
    validate_request(request)?;
    let key = fs::read_to_string(request.public_key)?;
    let hash = fs::read_to_string(request.password_hash)?;
    let source = request.butane.join(format!("{}.bu", request.host));
    let mut config: Value = serde_yml::from_str(&fs::read_to_string(source)?)?;
    skillet_vm::install::normalize_butane_modes(&mut config)?;
    specialize(&mut config, request.disk, request.root_profile)?;
    let mut passwd: Value = serde_yml::from_str(&fs::read_to_string(
        request.butane.join("includes/passwd.bu"),
    )?)?;
    configure_access(&mut passwd, &key, &hash)?;
    skillet_vm::install::normalize_butane_modes(&mut passwd)?;
    let iso = fs::canonicalize(request.iso)?;
    if !iso.is_file() {
        return Err(Error::Invalid("ISO must be a regular file".into()));
    }
    // Complete validation before building or publishing any artifacts.
    let package = format!("skillet-{}", request.host);
    let binaries = skillet_vm::artifacts::build(&skillet_vm::artifacts::BuildRequest {
        workspace: request.workspace,
        packages: &["skillet", &package],
        target: "x86_64-unknown-linux-musl",
        profile: "release",
    })?;
    let parent = request
        .output
        .parent()
        .ok_or_else(|| Error::Invalid("output needs a parent directory".into()))?;
    fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new()
        .prefix(".skillet-install-")
        .tempdir_in(parent)?;
    fs::set_permissions(staging.path(), fs::Permissions::from_mode(0o700))?;
    let source_dir = stage_source(request, staging.path(), &config, &passwd, &binaries)?;
    let mut compiler = Command::new(request.butane.join("bin/butane"));
    compiler
        .args(["-s", "-o"])
        .arg(staging.path())
        .arg(source_dir.join(format!("{}.bu", request.host)));
    run(compiler, "Butane compilation")?;
    let ignition_path = staging.path().join(format!("{}.ign", request.host));
    fs::set_permissions(&ignition_path, fs::Permissions::from_mode(0o600))?;
    let ignition = fs::read(ignition_path)?;
    let helper = fs::read(request.butane.join("bin/install-hardware"))?;
    let certificate = request.mok_certificate.map(fs::read).transpose()?;
    let live = live_ignition(
        request.disk,
        request.root_profile,
        &ignition,
        &helper,
        key.trim(),
        certificate.as_deref(),
    );
    write_private(
        &staging.path().join("live.ign"),
        &serde_json::to_vec_pretty(&live).map_err(skillet_vm::Error::from)?,
    )?;
    customize_iso(staging.path(), &iso)?;
    // No-clobber directory publication, including concurrent preparations.
    fs::create_dir(request.output)?;
    fs::set_permissions(request.output, fs::Permissions::from_mode(0o700))?;
    for entry in fs::read_dir(staging.path())? {
        let entry = entry?;
        fs::rename(entry.path(), request.output.join(entry.file_name()))?;
    }
    Ok(request.output.join("install.iso"))
}

fn run(command: Command, operation: &str) -> Result<()> {
    let output = skillet_vm::capture_command(command, Duration::from_mins(30))?;
    if !output.status.success() {
        let mut diagnostic = tempfile::Builder::new()
            .prefix("skillet-install-diagnostic-")
            .suffix(".log")
            .tempfile()?;
        diagnostic.write_all(&output.stdout)?;
        diagnostic.write_all(&output.stderr)?;
        let (_, path) = diagnostic.keep().map_err(|error| error.error)?;
        return Err(Error::Invalid(format!(
            "{operation} failed (exit {:?}); inspect private diagnostic {}. Partial staging was removed",
            output.status.code(), path.display()
        )));
    }
    Ok(())
}

fn write_private(path: &Path, data: &[u8]) -> Result<()> {
    fs::write(path, data)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn specialize(config: &mut Value, disk: &str, profile: RootProfile) -> Result<()> {
    skillet_vm::install::configure_disk(config, disk)?;
    skillet_vm::install::configure_root(config, profile)?;
    Ok(())
}

fn configure_access(passwd: &mut Value, public_key: &str, hash: &str) -> Result<()> {
    let key = public_key.trim();
    let hash = hash.trim();
    let mut parts = key.split_whitespace();
    if parts.next() != Some("ssh-ed25519")
        || parts.next().is_none_or(|part| {
            STANDARD.decode(part).map_or(true, |decoded| {
                decoded.len() != 51 || !decoded.starts_with(b"\0\0\0\x0bssh-ed25519\0\0\0\x20")
            })
        })
        || key.contains(['\n', '\r'])
    {
        return Err(Error::Invalid(
            "provide one OpenSSH Ed25519 public key".into(),
        ));
    }
    if !valid_password_hash(hash) {
        return Err(Error::Invalid(
            "provide a private crypt-format bootstrap password hash file".into(),
        ));
    }
    let users = passwd
        .get_mut("passwd")
        .and_then(|v| v.get_mut("users"))
        .and_then(Value::as_sequence_mut)
        .ok_or_else(|| Error::Invalid("missing production user declarations".into()))?;
    let mut found = false;
    for user in users {
        user["ssh_authorized_keys"] = serde_yml::to_value([key])?;
        if user["name"].as_str() == Some("giacomo") {
            user["password_hash"] = Value::String(hash.into());
            found = true;
        }
    }
    if !found {
        return Err(Error::Invalid("missing provisioning account".into()));
    }
    Ok(())
}

fn live_ignition(
    disk: &str,
    profile: RootProfile,
    ignition: &[u8],
    helper: &[u8],
    public_key: &str,
    certificate: Option<&[u8]>,
) -> serde_json::Value {
    let mut files = vec![
        live_file(
            "/etc/skillet-install/ignition.sha256",
            0o600,
            hex::encode(Sha256::digest(ignition)).as_bytes(),
        ),
        live_file("/etc/skillet-install/disk", 0o600, disk.as_bytes()),
        live_file(
            "/etc/skillet-install/profile",
            0o600,
            if profile == RootProfile::Tpm {
                b"tpm"
            } else {
                b"unencrypted"
            },
        ),
        live_file("/var/usrlocal/bin/skillet-install", 0o700, helper),
    ];
    if let Some(cert) = certificate {
        files.push(live_file("/etc/skillet-install/ublue.der", 0o600, cert));
    }
    serde_json::json!({"ignition": {"version": "3.5.0"}, "storage": {"files": files}, "passwd": {"users": [{"name": "core", "sshAuthorizedKeys": [public_key]}]}})
}
fn live_file(path: &str, mode: u32, contents: &[u8]) -> serde_json::Value {
    serde_json::json!({"path": path, "mode": mode, "contents": {"source": format!("data:;base64,{}", STANDARD.encode(contents))}})
}

#[cfg(test)]
#[path = "installation/tests.rs"]
mod tests;

fn reject_symlinks(path: &Path) -> Result<()> {
    for component in path.ancestors() {
        match fs::symlink_metadata(component) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(Error::Invalid(
                    "symlink in installation output or source includes".into(),
                ))
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn stage_source(
    request: &Request<'_>,
    staging: &Path,
    config: &Value,
    passwd: &Value,
    binaries: &std::collections::BTreeMap<String, PathBuf>,
) -> Result<PathBuf> {
    let package = format!("skillet-{}", request.host);
    let source_dir = staging.join("source");
    fs::create_dir_all(source_dir.join("includes"))?;
    fs::create_dir(source_dir.join("files"))?;
    for entry in fs::read_dir(request.butane.join("includes"))? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|ext| ext == "bu") {
            reject_symlinks(&entry.path())?;
            fs::copy(
                entry.path(),
                source_dir.join("includes").join(entry.file_name()),
            )?;
        }
    }
    write_private(
        &source_dir.join(format!("{}.bu", request.host)),
        serde_yml::to_string(&config)?.as_bytes(),
    )?;
    write_private(
        &source_dir.join("includes/passwd.bu"),
        serde_yml::to_string(&passwd)?.as_bytes(),
    )?;
    for name in ["skillet", package.as_str()] {
        let binary = binaries
            .get(name)
            .ok_or_else(|| Error::Invalid("missing build artifact".into()))?;
        let target = source_dir.join("files").join(name);
        fs::copy(binary, &target)?;
        fs::set_permissions(target, fs::Permissions::from_mode(0o755))?;
    }
    Ok(source_dir)
}

fn customize_iso(staging: &Path, iso: &Path) -> Result<()> {
    let mut customize = Command::new("podman");
    customize
        .args(["run", "--rm", "--security-opt", "label=disable", "--volume"])
        .arg(format!("{}:/work", staging.display()))
        .arg("--volume")
        .arg(format!("{}:/input.iso:ro", iso.display()))
        .args([
            "quay.io/coreos/coreos-installer:release",
            "iso",
            "customize",
            "--live-ignition",
            "/work/live.ign",
            "--output",
            "/work/install.iso",
            "/input.iso",
        ]);
    run(customize, "CoreOS ISO customization")?;
    fs::set_permissions(
        staging.join("install.iso"),
        fs::Permissions::from_mode(0o600),
    )?;
    Ok(())
}

fn validate_request(request: &Request<'_>) -> Result<()> {
    skillet_vm::install::validate_physical_disk(request.disk)?;
    if !request
        .host
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || request.host.is_empty()
        || skillet_hosts::profile_for_name(request.host).is_none()
        || !request
            .workspace
            .join("crates/hosts")
            .join(request.host)
            .join("Cargo.toml")
            .is_file()
    {
        return Err(Error::Invalid("unknown host profile".into()));
    }
    if request.root_profile == RootProfile::Tpm && request.mok_certificate.is_none() {
        return Err(Error::Invalid(
            "TPM media requires --mok-certificate; enroll it before installation".into(),
        ));
    }
    if !request.output.is_absolute()
        || request
            .output
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(Error::Invalid(
            "output must be an absolute path without parent traversal".into(),
        ));
    }
    let workspace = fs::canonicalize(request.workspace)?;
    let repository = workspace
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| Error::Invalid("workspace must belong to a repository".into()))?;
    if request.output.starts_with(repository) {
        return Err(Error::Invalid(
            "private install bundle must be outside the repository".into(),
        ));
    }
    reject_symlinks(request.output)?;
    if request.output.exists() {
        return Err(Error::Invalid(
            "output already exists; choose a new bundle directory".into(),
        ));
    }
    Ok(())
}

fn valid_password_hash(hash: &str) -> bool {
    fn crypt_text(value: &str) -> bool {
        !value.is_empty()
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"./".contains(&b))
    }
    let fields: Vec<_> = hash.split('$').collect();
    match fields.as_slice() {
        ["", algorithm @ ("5" | "6"), salt, digest] => {
            crypt_text(salt)
                && salt.len() <= 16
                && crypt_text(digest)
                && digest.len() == if *algorithm == "6" { 86 } else { 43 }
        }
        ["", algorithm @ ("5" | "6"), rounds, salt, digest] => {
            rounds
                .strip_prefix("rounds=")
                .is_some_and(|r| !r.is_empty() && r.bytes().all(|b| b.is_ascii_digit()))
                && valid_password_hash(&format!("${algorithm}${salt}${digest}"))
        }
        ["", "y", parameters, salt, digest] => {
            crypt_text(parameters) && crypt_text(salt) && crypt_text(digest) && digest.len() == 43
        }
        _ => false,
    }
}
