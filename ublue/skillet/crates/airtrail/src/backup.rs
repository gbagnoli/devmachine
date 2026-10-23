//! Coordinated `AirTrail` PostgreSQL and upload backups.
use crate::Config;
use skillet_postgres::backup;
use std::{
    fs::{self, File, OpenOptions},
    os::fd::AsRawFd,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

const ARCHIVE_PREFIX: &str = "airtrail-";

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Postgres(#[from] skillet_postgres::PostgresError),
    #[error("AirTrail backup configuration is invalid")]
    InvalidConfiguration,
    #[error("AirTrail service transition failed")]
    Service,
    #[error("AirTrail restore and rollback both failed")]
    RestoreAndRollback,
    #[error("AirTrail operation failed and the service could not be restarted")]
    OperationAndRestart,
}

/// Stop `AirTrail` while taking the database and upload snapshot so the returned
/// directory is one consistent restore point.
pub fn create(config: &Config, uid: u32, gid: u32) -> Result<PathBuf, Error> {
    validate(config)?;
    let was_active = systemctl_is_active("airtrail.service")?;
    if was_active {
        systemctl("stop", "airtrail.service")?;
    }
    finish_service_transition(create_stopped(config, uid, gid), was_active)
}

/// Restore a paired archive and retain a paired pre-restore recovery snapshot.
pub fn restore(config: &Config, selected: &Path, uid: u32, gid: u32) -> Result<PathBuf, Error> {
    validate(config)?;
    validate_archive(config, selected)?;
    let was_active = systemctl_is_active("airtrail.service")?;
    if was_active {
        systemctl("stop", "airtrail.service")?;
    }

    let result = (|| {
        let recovery = create_stopped(config, uid, gid)?;
        restore_stopped(config, selected, &recovery)?;
        Ok(recovery)
    })();
    finish_service_transition(result, was_active)
}

fn finish_service_transition<T>(result: Result<T, Error>, was_active: bool) -> Result<T, Error> {
    if !was_active {
        return result;
    }
    match (result, systemctl("start", "airtrail.service")) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (_, Err(_)) => Err(Error::OperationAndRestart),
    }
}

fn create_stopped(config: &Config, uid: u32, gid: u32) -> Result<PathBuf, Error> {
    let root = Path::new(config.backup_path);
    fs::create_dir_all(root)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::InvalidConfiguration)?
        .as_nanos();
    let final_path = root.join(format!("{ARCHIVE_PREFIX}{stamp}"));
    let pending = root.join(format!(
        ".{ARCHIVE_PREFIX}{stamp}-{}.partial",
        std::process::id()
    ));
    fs::create_dir(&pending)?;
    set_dir_owner_mode(&pending, uid, gid, 0o750)?;
    let uploads = pending.join("uploads");
    fs::create_dir(&uploads)?;
    set_dir_owner_mode(&uploads, uid, gid, 0o750)?;

    let result = (|| {
        copy_tree(Path::new(config.uploads_path), &uploads, uid, gid)?;
        let postgres = crate::backup_config(*config);
        backup::dump_to(&postgres, &pending.join("database.dump"), uid, gid)?;
        File::open(&pending)?.sync_all()?;
        fs::rename(&pending, &final_path)?;
        File::open(root)?.sync_all()?;
        prune(config)?;
        Ok(final_path.clone())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&pending);
    }
    result
}

fn restore_stopped(config: &Config, selected: &Path, recovery: &Path) -> Result<(), Error> {
    validate_archive(config, selected)?;
    let database = selected.join("database.dump");
    let source_uploads = selected.join("uploads");
    let uploads = Path::new(config.uploads_path);
    let parent = uploads.parent().ok_or(Error::InvalidConfiguration)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::InvalidConfiguration)?
        .as_nanos();
    let staged_uploads = parent.join(format!(".airtrail-restore-{nonce}"));
    let previous_uploads = parent.join(format!(".airtrail-previous-{nonce}"));
    fs::create_dir(&staged_uploads)?;
    let (uid, gid) =
        fs::metadata(uploads).map_or((0, 0), |metadata| (metadata.uid(), metadata.gid()));
    let staged_result = copy_tree(&source_uploads, &staged_uploads, uid, gid);
    if let Err(error) = staged_result {
        let _ = fs::remove_dir_all(&staged_uploads);
        return Err(error);
    }

    let had_uploads = uploads.exists();
    if had_uploads {
        fs::rename(uploads, &previous_uploads)?;
    }
    if let Err(error) = fs::rename(&staged_uploads, uploads) {
        if had_uploads {
            let _ = fs::rename(&previous_uploads, uploads);
        }
        let _ = fs::remove_dir_all(&staged_uploads);
        return Err(error.into());
    }

    let result = backup::restore_dump(&crate::backup_config(*config), &database);
    if let Err(error) = result {
        let rollback_db = backup::restore_dump(
            &crate::backup_config(*config),
            &recovery.join("database.dump"),
        );
        let rollback_files = restore_uploads_from(&recovery.join("uploads"), uploads);
        if rollback_db.is_err() || rollback_files.is_err() {
            return Err(Error::RestoreAndRollback);
        }
        if had_uploads {
            fs::remove_dir_all(&previous_uploads)?;
        }
        return Err(error.into());
    }
    if had_uploads {
        fs::remove_dir_all(previous_uploads)?;
    }
    Ok(())
}

fn restore_uploads_from(source: &Path, target: &Path) -> Result<(), Error> {
    let parent = target.parent().ok_or(Error::InvalidConfiguration)?;
    let staged = parent.join(format!(".airtrail-rollback-{}", std::process::id()));
    if staged.exists() {
        fs::remove_dir_all(&staged)?;
    }
    fs::create_dir(&staged)?;
    let metadata = fs::metadata(target)?;
    let copied = copy_tree(source, &staged, metadata.uid(), metadata.gid());
    if let Err(error) = copied {
        let _ = fs::remove_dir_all(staged);
        return Err(error);
    }
    let displaced = parent.join(format!(".airtrail-failed-{}", std::process::id()));
    if displaced.exists() {
        fs::remove_dir_all(&displaced)?;
    }
    fs::rename(target, &displaced)?;
    if let Err(error) = fs::rename(&staged, target) {
        let _ = fs::rename(&displaced, target);
        return Err(error.into());
    }
    fs::remove_dir_all(displaced)?;
    Ok(())
}

fn validate_archive(config: &Config, archive: &Path) -> Result<(), Error> {
    let root = Path::new(config.backup_path);
    let metadata = fs::symlink_metadata(archive)?;
    if !metadata.is_dir()
        || archive.parent() != Some(root)
        || !archive
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix(ARCHIVE_PREFIX))
            .is_some_and(|stamp| {
                !stamp.is_empty() && stamp.bytes().all(|byte| byte.is_ascii_digit())
            })
    {
        return Err(Error::InvalidConfiguration);
    }
    for path in [archive.join("database.dump"), archive.join("uploads")] {
        let metadata = fs::symlink_metadata(&path)?;
        if path.ends_with("database.dump") && !metadata.is_file() {
            return Err(Error::InvalidConfiguration);
        }
        if path.ends_with("uploads") && !metadata.is_dir() {
            return Err(Error::InvalidConfiguration);
        }
    }
    Ok(())
}

fn prune(config: &Config) -> Result<(), Error> {
    let mut archives = Vec::new();
    for entry in fs::read_dir(config.backup_path)? {
        let entry = entry?;
        let Some(stamp) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_prefix(ARCHIVE_PREFIX))
            .and_then(|value| value.parse::<u128>().ok())
        else {
            continue;
        };
        if entry.file_type()?.is_dir() {
            archives.push((stamp, entry.path()));
        }
    }
    archives.sort_by_key(|(stamp, _)| std::cmp::Reverse(*stamp));
    for (_, path) in archives.into_iter().skip(crate::BACKUP_KEEP) {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn validate(config: &Config) -> Result<(), Error> {
    let root = Path::new(config.backup_path);
    let subvolume = Path::new(config.backup_subvolume);
    if !root.is_absolute()
        || root == subvolume
        || !root.starts_with(subvolume)
        || !Path::new(config.uploads_path).is_absolute()
        || !Path::new(config.uploads_path).starts_with("/var/lib/data/")
    {
        return Err(Error::InvalidConfiguration);
    }
    Ok(())
}

fn copy_tree(source: &Path, target: &Path, uid: u32, gid: u32) -> Result<(), Error> {
    let source_metadata = fs::symlink_metadata(source)?;
    if !source_metadata.is_dir() {
        return Err(Error::InvalidConfiguration);
    }
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path)?;
        if metadata.is_dir() {
            fs::create_dir(&target_path)?;
            set_dir_owner_mode(&target_path, uid, gid, 0o750)?;
            copy_tree(&source_path, &target_path, uid, gid)?;
        } else if metadata.is_file() {
            copy_file(&source_path, &target_path, uid, gid)?;
        } else {
            return Err(Error::InvalidConfiguration);
        }
    }
    set_dir_owner_mode(target, uid, gid, 0o750)?;
    File::open(target)?.sync_all()?;
    Ok(())
}

fn copy_file(source: &Path, target: &Path, uid: u32, gid: u32) -> Result<(), Error> {
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o640)
        .open(target)?;
    nix::unistd::fchown(
        output.as_raw_fd(),
        Some(nix::unistd::Uid::from_raw(uid)),
        Some(nix::unistd::Gid::from_raw(gid)),
    )
    .map_err(|_| Error::InvalidConfiguration)?;
    std::io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    Ok(())
}

fn set_dir_owner_mode(path: &Path, uid: u32, gid: u32, mode: u32) -> Result<(), Error> {
    nix::unistd::chown(
        path,
        Some(nix::unistd::Uid::from_raw(uid)),
        Some(nix::unistd::Gid::from_raw(gid)),
    )
    .map_err(|_| Error::InvalidConfiguration)?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
}

fn systemctl_is_active(unit: &str) -> Result<bool, Error> {
    let status = Command::new("/usr/bin/systemctl")
        .args(["is-active", "--quiet", unit])
        .status()?;
    match status.code() {
        Some(0) => Ok(true),
        Some(3) => Ok(false),
        _ => Err(Error::Service),
    }
}

fn systemctl(action: &str, unit: &str) -> Result<(), Error> {
    if Command::new("/usr/bin/systemctl")
        .args([action, unit])
        .status()?
        .success()
    {
        Ok(())
    } else {
        Err(Error::Service)
    }
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;
