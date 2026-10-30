//! Stopped-service paired archives, published only after complete tar creation.
use crate::{Config, Error, Settings, IMAGE};
use serde_json::json;
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::Read,
    os::unix::fs::{chown, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
pub const UNIT: &str = "skillet-gokapi-backup.service";
pub const TIMER: &str = "skillet-gokapi-backup.timer";
const KEEP: usize = 7;

pub fn create(config: Config, uid: u32, gid: u32) -> Result<PathBuf, Error> {
    config.validate()?;
    let lock = crate::initial::lock()?;
    let active = active()?;
    if active {
        service("stop")?;
    }
    let result = crate::initial::no_bootstrap(config)
        .and_then(|()| create_stopped(config, uid, gid))
        .and_then(|path| {
            prune(Path::new(config.backup_path), KEEP)?;
            Ok(path)
        });
    drop(lock);
    finish(result, active)
}
pub fn restore(config: Config, archive: &Path, uid: u32, gid: u32) -> Result<PathBuf, Error> {
    config.validate()?;
    validate_archive(config, archive)?;
    let lock = crate::initial::lock()?;
    let active = active()?;
    if active {
        service("stop")?;
    }
    let result = (|| {
        crate::initial::no_bootstrap(config)?;
        let recovery = create_stopped(config, uid, gid)?;
        replace_state(config, archive, uid, gid)?;
        prune(Path::new(config.backup_path), KEEP)?;
        Ok(recovery)
    })();
    drop(lock);
    finish(result, active)
}
fn finish<T>(result: Result<T, Error>, active: bool) -> Result<T, Error> {
    if active {
        service("start")?;
    }
    result
}
fn active() -> Result<bool, Error> {
    match Command::new("systemctl")
        .args(["is-active", "--quiet", "gokapi.service"])
        .status()?
        .code()
    {
        Some(0) => Ok(true),
        Some(3) => Ok(false),
        _ => Err(Error::Consumer),
    }
}
fn service(operation: &str) -> Result<(), Error> {
    if Command::new("systemctl")
        .args([operation, "gokapi.service"])
        .status()?
        .success()
    {
        Ok(())
    } else {
        Err(Error::Consumer)
    }
}
fn stamp() -> Result<u128, Error> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Invalid)?
        .as_nanos())
}
fn validate_state(state: &Path) -> Result<(), Error> {
    let settings: Settings =
        serde_json::from_slice(&fs::read(state.join(".skillet-initial.json"))?)?;
    settings.validate()?;
    crate::initial::validate_native(&fs::read(state.join("config/config.json"))?, &settings)?;
    if !state.join("config/gokapi.sqlite").is_file() || !state.join("data").is_dir() {
        return Err(Error::Binding);
    }
    Ok(())
}
pub(crate) fn create_stopped(config: Config, uid: u32, gid: u32) -> Result<PathBuf, Error> {
    let state = Path::new(config.state_path);
    validate_state(state)?;
    let root = Path::new(config.backup_path);
    fs::create_dir_all(root)?;
    let nonce = stamp()?;
    // Syncthing recognizes this prefix as a temporary file, independently of
    // the operator's folder/ignore configuration. The published tar is ordinary.
    let pending = root.join(format!(".syncthing.gokapi-{nonce}.tmp"));
    let archive = root.join(format!("gokapi-{nonce}.tar"));
    let result = (|| {
        let file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&pending)?;
        chown(&pending, Some(uid), Some(gid))?;
        let mut builder = tar::Builder::new(file);
        append_manifest(&mut builder)?;
        builder.append_dir("state", state)?;
        for leaf in ["config", "data"] {
            append_tree(
                &mut builder,
                &state.join(leaf),
                &Path::new("state").join(leaf),
            )?;
        }
        builder.append_file(
            "state/.skillet-initial.json",
            &mut File::open(state.join(".skillet-initial.json"))?,
        )?;
        builder.into_inner()?.sync_all()?;
        fs::hard_link(&pending, &archive)?;
        fs::remove_file(&pending)?;
        File::open(root)?.sync_all()?;
        Ok(archive)
    })();
    if result.is_err() {
        let _ = fs::remove_file(pending);
    }
    result
}
fn append_manifest(builder: &mut tar::Builder<File>) -> Result<(), Error> {
    let bytes = serde_json::to_vec(&json!({"format":1,"kind":"skillet-gokapi","image":IMAGE}))?;
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o600);
    header.set_cksum();
    builder.append_data(&mut header, "manifest.json", bytes.as_slice())?;
    Ok(())
}
fn append_tree(builder: &mut tar::Builder<File>, source: &Path, name: &Path) -> Result<(), Error> {
    if !fs::symlink_metadata(source)?.is_dir() {
        return Err(Error::Invalid);
    }
    builder.append_dir(name, source)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = name.join(entry.file_name());
        if kind.is_dir() {
            append_tree(builder, &entry.path(), &target)?;
        } else if kind.is_file() {
            builder.append_file(target, &mut File::open(entry.path())?)?;
        } else {
            return Err(Error::Invalid);
        }
    }
    Ok(())
}
fn manifest(path: &Path) -> Result<serde_json::Value, Error> {
    let mut archive = tar::Archive::new(File::open(path)?);
    let mut entries = archive.entries()?;
    let mut entry = entries.next().ok_or(Error::Invalid)??;
    if entry.path()?.as_ref() != Path::new("manifest.json")
        || !entry.header().entry_type().is_file()
        || entry.size() > 4096
    {
        return Err(Error::Invalid);
    }
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}
fn validate_archive(config: Config, path: &Path) -> Result<(), Error> {
    let root = fs::canonicalize(config.backup_path)?;
    if !fs::symlink_metadata(path)?.is_file()
        || fs::canonicalize(path)?.parent() != Some(root.as_path())
        || archive_stamp(path).is_none()
        || manifest(path)? != json!({"format":1,"kind":"skillet-gokapi","image":IMAGE})
    {
        return Err(Error::Binding);
    }
    Ok(())
}
fn archive_stamp(path: &Path) -> Option<u128> {
    path.file_name()?
        .to_str()?
        .strip_prefix("gokapi-")?
        .strip_suffix(".tar")?
        .parse()
        .ok()
}
fn prune(root: &Path, keep: usize) -> Result<(), Error> {
    let mut archives = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let Some(stamp) = archive_stamp(&entry.path()) else {
            continue;
        };
        if entry.file_type()?.is_file()
            && manifest(&entry.path())
                .is_ok_and(|value| value["format"] == 1 && value["kind"] == "skillet-gokapi")
        {
            archives.push((stamp, entry.path()));
        }
    }
    archives.sort();
    let remove = archives.len().saturating_sub(keep);
    for (_, archive) in archives.into_iter().take(remove) {
        fs::remove_file(archive)?;
    }
    Ok(())
}
fn extract(archive: &Path, root: &Path, uid: u32, gid: u32) -> Result<(), Error> {
    let mut archive = tar::Archive::new(File::open(archive)?);
    let mut seen = BTreeSet::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.into_owned();
        let kind = entry.header().entry_type();
        if !seen.insert(name.clone())
            || name
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
            || (name != Path::new("manifest.json") && !name.starts_with("state"))
            || (!kind.is_file() && !kind.is_dir())
        {
            return Err(Error::Invalid);
        }
        let path = root.join(name);
        if kind.is_dir() {
            fs::create_dir(&path)?;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        } else {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .mode(0o600)
                .open(&path)?;
            std::io::copy(&mut entry, &mut file)?;
            file.sync_all()?;
        }
        chown(&path, Some(uid), Some(gid))?;
    }
    validate_state(&root.join("state"))?;
    File::open(root)?.sync_all()?;
    Ok(())
}
pub(crate) fn replace_state(
    config: Config,
    archive: &Path,
    uid: u32,
    gid: u32,
) -> Result<(), Error> {
    validate_archive(config, archive)?;
    let state = Path::new(config.state_path);
    let parent = state.parent().ok_or(Error::Invalid)?;
    let nonce = stamp()?;
    let staged = parent.join(format!(".gokapi-restore-{nonce}"));
    let previous = parent.join(format!(".gokapi-previous-{nonce}"));
    fs::create_dir(&staged)?;
    fs::set_permissions(&staged, fs::Permissions::from_mode(0o700))?;
    if let Err(error) = extract(archive, &staged, uid, gid) {
        let _ = fs::remove_dir_all(&staged);
        return Err(error);
    }
    fs::rename(state, &previous)?;
    if let Err(error) = fs::rename(staged.join("state"), state) {
        fs::rename(&previous, state)?;
        return Err(error.into());
    }
    File::open(parent)?.sync_all()?;
    fs::remove_dir_all(previous)?;
    fs::remove_dir_all(staged)?;
    Ok(())
}
#[cfg(test)]
mod tests;
