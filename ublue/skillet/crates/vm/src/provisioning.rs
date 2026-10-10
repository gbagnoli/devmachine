//! Local artifacts owned by a disposable VM creation run.
use crate::{manifest::reject_symlinks, Error, ManifestStore, Phase, Result, RunIdentity, VmRun};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read, Write as _},
    net::TcpListener,
    os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

/// Fail early when another local listener already owns the VM's loopback SSH
/// forwarding port.
pub fn validate_ssh_port(port: u16) -> Result<()> {
    validate_ssh_port_with(port, |port| {
        TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).map(drop)
    })
}

fn validate_ssh_port_with(port: u16, bind: impl FnOnce(u16) -> std::io::Result<()>) -> Result<()> {
    if !(2200..=2299).contains(&port) {
        return Err(Error::Invalid(
            "VM SSH port must be between 2200 and 2299".into(),
        ));
    }
    bind(port)
        .map_err(|error| Error::Preparation(format!("VM SSH port {port} is unavailable: {error}")))
}

/// Resolve one supported Fedora `CoreOS` QEMU image, downloading it through the
/// existing `CoreOS` installer container only when none is already present.
pub fn resolve_coreos_image(images_dir: &Path, requested: Option<&Path>) -> Result<PathBuf> {
    if let Some(image) = requested {
        reject_symlinks(image)?;
        if image.is_file() {
            return fs::canonicalize(image).map_err(Into::into);
        }
        return Err(Error::Invalid(
            "requested VM image is not a regular file".into(),
        ));
    }
    let mut candidates = coreos_images(images_dir)?;
    if candidates.is_empty() {
        download_coreos_image(images_dir)?;
        candidates = coreos_images(images_dir)?;
    }
    match candidates.as_slice() {
        [image] => fs::canonicalize(image).map_err(Into::into),
        [] => Err(Error::Invalid(
            "CoreOS installer completed without producing a QEMU image".into(),
        )),
        _ => Err(Error::Invalid(format!(
            "multiple Fedora CoreOS QEMU images exist under {}; pass --image to choose one",
            images_dir.display()
        ))),
    }
}

fn coreos_images(images_dir: &Path) -> Result<Vec<PathBuf>> {
    reject_symlinks(images_dir)?;
    if !images_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut candidates = Vec::new();
    for entry in fs::read_dir(images_dir)? {
        let path = entry?.path();
        let matches_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                name.starts_with("fedora-coreos-") && name.ends_with("-qemu.x86_64.qcow2")
            });
        if !matches_name {
            continue;
        }
        reject_symlinks(&path)?;
        if fs::metadata(&path)?.is_file() {
            candidates.push(path);
        }
    }
    candidates.sort();
    Ok(candidates)
}

fn download_coreos_image(images_dir: &Path) -> Result<()> {
    if !images_dir.is_absolute()
        || images_dir
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(Error::Invalid(
            "image directory must be an absolute safe path".into(),
        ));
    }
    reject_symlinks(images_dir)?;
    fs::create_dir_all(images_dir)?;
    let mount_source = images_dir
        .to_str()
        .filter(|path| !path.contains(',') && !path.contains('\n'))
        .ok_or_else(|| {
            Error::Invalid("image directory is not valid for Podman mount syntax".into())
        })?;
    let mut command = Command::new("podman");
    command.args([
        "run",
        "--pull=always",
        "--rm",
        "--security-opt",
        "label=disable",
        "--volume",
    ]);
    command.arg(format!("{mount_source}:/data"));
    command.args([
        "--workdir",
        "/data",
        "quay.io/coreos/coreos-installer:release",
        "download",
        "-s",
        "stable",
        "-p",
        "qemu",
        "-f",
        "qcow2.xz",
        "--decompress",
    ]);
    let output = crate::process::capture(command, Duration::from_mins(30))?;
    if !output.status.success() {
        return Err(Error::Command {
            operation: "download Fedora CoreOS QEMU image".into(),
            code: output.status.code(),
        });
    }
    Ok(())
}

/// Return the recorded source revision used to identify a resumable run.
pub fn source_revision(workspace: &Path) -> Result<String> {
    let mut command = Command::new("git");
    command.args([OsString::from("-C"), workspace.as_os_str().to_owned()]);
    command.args(["rev-parse", "--verify", "HEAD"]);
    let output = crate::process::capture(command, Duration::from_secs(15))?;
    if !output.status.success() {
        return Err(Error::Preparation(
            "git could not identify the source revision for VM creation".into(),
        ));
    }
    let revision = String::from_utf8(output.stdout)
        .map_err(|_| Error::Invalid("git source revision is not UTF8".into()))?
        .trim()
        .to_owned();
    if revision.is_empty() || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::Invalid(
            "git returned an invalid source revision".into(),
        ));
    }
    Ok(revision)
}

/// Prepare the private SSH key and VM disk for a recorded, not-yet-defined run.
/// Repeated calls reuse a matching key and image, and repair a changed disk.
pub fn prepare_local_artifacts(
    store: &ManifestStore,
    identity: &RunIdentity,
    image: &Path,
) -> Result<String> {
    let _lock = store.lock(identity)?;
    let run = store.load(identity)?;
    store.validate(&run, identity)?;
    if run.phase != Phase::Preparing {
        return Err(Error::Invalid(
            "local VM artifacts can only be prepared for a preparing run".into(),
        ));
    }
    prepare_disk(store, &run, image)?;
    prepare_key(store, &run)
}

fn prepare_disk(store: &ManifestStore, run: &VmRun, image: &Path) -> Result<()> {
    reject_symlinks(image)?;
    let mut source = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(image)?;
    if !source.metadata()?.is_file() {
        return Err(Error::Invalid("VM image is not a regular file".into()));
    }
    let dir = store.run_dir(&run.identity);
    reject_symlinks(&dir)?;
    reject_symlinks(&run.disk)?;
    let (source_hash, source_size) = hash_file(&mut source)?;
    let sidecar = dir.join("fcos-image.sha256");
    let disk_matches = if run.disk.is_file() {
        let mut disk = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(&run.disk)?;
        disk.metadata()?.is_file()
            && disk.metadata()?.len() == source_size
            && hash_file(&mut disk)?.0 == source_hash
    } else {
        false
    };
    if !disk_matches {
        let mut source = OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(image)?;
        let mut staged = tempfile::NamedTempFile::new_in(&dir)?;
        staged
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o644))?;
        let mut hasher = Sha256::new();
        let mut copied = 0_u64;
        let mut buffer = [0_u8; 16 * 1024];
        loop {
            let count = source.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            staged.write_all(&buffer[..count])?;
            hasher.update(&buffer[..count]);
            copied =
                copied
                    .checked_add(u64::try_from(count).map_err(|_| {
                        Error::Invalid("VM image size exceeds supported range".into())
                    })?)
                    .ok_or_else(|| Error::Invalid("VM image size overflow".into()))?;
        }
        if copied != source_size || hex::encode(hasher.finalize()) != source_hash {
            return Err(Error::Invalid(
                "VM image changed while it was being copied".into(),
            ));
        }
        staged.as_file().sync_all()?;
        staged
            .persist(&run.disk)
            .map_err(|error| Error::Io(error.error))?;
    }
    let source_name = image
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::Invalid("VM image filename is invalid".into()))?;
    write_atomic(
        &dir,
        &sidecar,
        format!("{source_hash}  {source_name}\n").as_bytes(),
        0o600,
    )?;
    File::open(dir)?.sync_all()?;
    Ok(())
}

fn prepare_key(store: &ManifestStore, run: &VmRun) -> Result<String> {
    let key = &run.ssh.identity;
    let public = key.with_extension("pub");
    let dir = key
        .parent()
        .ok_or_else(|| Error::Invalid("SSH key path has no parent".into()))?;
    reject_symlinks(dir)?;
    if !dir.exists() {
        fs::create_dir(dir)?;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    store.validate(run, &run.identity)?;
    reject_symlinks(dir)?;
    reject_symlinks(key)?;
    reject_symlinks(&public)?;
    match (key.exists(), public.exists()) {
        (false, false) => {
            let mut command = Command::new("ssh-keygen");
            command.args(["-q", "-t", "ed25519", "-N", ""]);
            command.arg("-f").arg(key);
            let output = match crate::process::capture(command, Duration::from_secs(30)) {
                Ok(output) => output,
                Err(error) => {
                    remove_incomplete_key(key, &public)?;
                    return Err(error);
                }
            };
            if !output.status.success() {
                remove_incomplete_key(key, &public)?;
                return Err(Error::Preparation("ssh-keygen failed".into()));
            }
        }
        (true, true) => (),
        _ => {
            return Err(Error::Invalid(
                "prepared SSH key pair is incomplete; inspect the run before retrying".into(),
            ));
        }
    }
    validated_public_key(run)
}

pub(crate) fn validated_public_key(run: &VmRun) -> Result<String> {
    let key = &run.ssh.identity;
    let public = key.with_extension("pub");
    reject_symlinks(key)?;
    reject_symlinks(&public)?;
    let key_metadata = fs::metadata(key)?;
    let public_metadata = fs::metadata(&public)?;
    if !key_metadata.is_file() || !public_metadata.is_file() {
        return Err(Error::Invalid("SSH key pair must be regular files".into()));
    }
    fs::set_permissions(key, fs::Permissions::from_mode(0o600))?;
    let mut command = Command::new("ssh-keygen");
    command.arg("-y").arg("-f").arg(key);
    let output = crate::process::capture(command, Duration::from_secs(30))?;
    if !output.status.success() {
        return Err(Error::Preparation(
            "SSH private key could not be validated".into(),
        ));
    }
    let derived = String::from_utf8(output.stdout)
        .map_err(|_| Error::Invalid("ssh-keygen returned an invalid public key".into()))?;
    let saved = fs::read_to_string(public)?;
    let derived = derived.trim();
    let saved = saved.trim();
    if derived.is_empty() || derived != saved {
        return Err(Error::Invalid(
            "prepared SSH key pair is inconsistent".into(),
        ));
    }
    Ok(derived.to_owned())
}

fn remove_incomplete_key(key: &Path, public: &Path) -> Result<()> {
    reject_symlinks(key)?;
    reject_symlinks(public)?;
    if key.exists() {
        fs::remove_file(key)?;
    }
    if public.exists() {
        fs::remove_file(public)?;
    }
    Ok(())
}

fn hash_file(file: &mut File) -> Result<(String, u64)> {
    let mut hasher = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        size = size
            .checked_add(
                u64::try_from(count)
                    .map_err(|_| Error::Invalid("VM image size exceeds supported range".into()))?,
            )
            .ok_or_else(|| Error::Invalid("VM image size overflow".into()))?;
    }
    Ok((hex::encode(hasher.finalize()), size))
}

fn write_atomic(dir: &Path, path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    reject_symlinks(path)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    file.as_file()
        .set_permissions(fs::Permissions::from_mode(mode))?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| Error::Io(error.error))?;
    Ok(())
}

#[cfg(test)]
#[path = "provisioning_tests.rs"]
mod tests;
