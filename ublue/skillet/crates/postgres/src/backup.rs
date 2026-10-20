//! Per-application logical PostgreSQL backups and explicit restore operations.
use crate::PostgresError;
use skillet_core::{
    files::{FileMutationResource, Ownership},
    system::{AccountLookupResource, ServiceResource},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self},
    os::fd::AsRawFd,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

pub const KEEP_ARCHIVES: usize = 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Config {
    pub instance: &'static str,
    pub database: &'static str,
    pub username: &'static str,
    pub container: &'static str,
    pub application_unit: &'static str,
    pub backup_path: &'static str,
    pub backup_subvolume: &'static str,
    pub owner: &'static str,
    pub group: &'static str,
    pub keep: usize,
}

impl Config {
    fn validate(self) -> Result<(), PostgresError> {
        for (value, label) in [
            (self.instance, "instance"),
            (self.database, "database"),
            (self.username, "username"),
            (self.container, "container"),
        ] {
            if value.is_empty()
                || value.len() > 63
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
            {
                return Err(PostgresError::InvalidConfiguration(label));
            }
        }
        let subvolume = Path::new(self.backup_subvolume);
        let destination = Path::new(self.backup_path);
        let relative = destination
            .strip_prefix(subvolume)
            .map_err(|_| PostgresError::InvalidConfiguration("backup path"))?;
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
            || !subvolume.is_absolute()
            || self.keep == 0
            || self.keep > KEEP_ARCHIVES
        {
            return Err(PostgresError::InvalidConfiguration(
                "backup path or retention",
            ));
        }
        Ok(())
    }
}

/// Install a daily systemd backup timer for one app-owned database.
pub fn install<S, F>(
    system: &S,
    files: &F,
    host: &str,
    config: &Config,
) -> Result<(), PostgresError>
where
    S: ServiceResource + AccountLookupResource + ?Sized,
    F: FileMutationResource + ?Sized,
{
    config.validate()?;
    if host.is_empty()
        || !host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(PostgresError::InvalidConfiguration("host"));
    }
    if system.user_by_name(config.owner)?.is_none() {
        return Err(PostgresError::InvalidConfiguration("backup owner"));
    }
    if system.group_by_name(config.group)?.is_none() {
        return Err(PostgresError::InvalidConfiguration("backup group"));
    }
    let ownership = Ownership::named(Some(config.owner), Some(config.group));
    let directory = Path::new(config.backup_path);
    files.ensure_directory(directory, Some(0o750), &ownership)?;
    let root = Ownership::named(Some("root"), Some("root"));
    let unit_dir = Path::new("/etc/systemd/system");
    files.ensure_directory(unit_dir, Some(0o755), &root)?;
    let service_name = format!("skillet-postgres-{}-backup.service", config.instance);
    let timer_name = format!("skillet-postgres-{}-backup.timer", config.instance);
    let binary = format!("/var/usrlocal/bin/skillet-{host}");
    let service = format!(
        "[Unit]\nDescription=Back up the {} PostgreSQL database\nRequiresMountsFor=/var/lib/data\nRequires={}.service\nAfter={}.service\nConditionPathIsMountPoint=/var/lib/data\n[Service]\nType=oneshot\nTimeoutStartSec=2h\nExecStart={binary} postgres backup --instance {}\nUMask=0027\nNoNewPrivileges=yes\nProtectHome=yes\nPrivateTmp=yes\n",
        config.instance, config.container, config.container, config.instance
    );
    let timer = format!(
        "[Unit]\nDescription=Daily backup of the {} PostgreSQL database\n[Timer]\nOnCalendar=daily\nRandomizedDelaySec=4h\nPersistent=true\n[Install]\nWantedBy=timers.target\n",
        config.instance
    );
    let service_changed = files.ensure_file(
        &unit_dir.join(&service_name),
        service.as_bytes(),
        Some(0o644),
        &root,
    )?;
    let timer_changed = files.ensure_file(
        &unit_dir.join(&timer_name),
        timer.as_bytes(),
        Some(0o644),
        &root,
    )?;
    if service_changed || timer_changed {
        system.daemon_reload()?;
    }
    system.service_enable(&timer_name)?;
    system.service_start(&timer_name)?;
    Ok(())
}

/// Dump one database to a private temporary file, atomically publish it, then
/// retain at most the configured newest archives.
pub fn create(config: &Config, uid: u32, gid: u32) -> Result<PathBuf, PostgresError> {
    create_inner(config, uid, gid, None)
}

/// Create the pre-restore recovery point while protecting the selected source
/// archive from the retention pass.
pub fn create_recovery(
    config: &Config,
    uid: u32,
    gid: u32,
    selected_archive: &Path,
) -> Result<PathBuf, PostgresError> {
    validate_archive(config, selected_archive)?;
    create_inner(config, uid, gid, Some(selected_archive))
}

fn create_inner(
    config: &Config,
    uid: u32,
    gid: u32,
    protected: Option<&Path>,
) -> Result<PathBuf, PostgresError> {
    config.validate()?;
    let directory = Path::new(config.backup_path);
    fs::create_dir_all(directory)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| PostgresError::InvalidConfiguration("system clock"))?
        .as_nanos();
    let stem = format!("{}-{stamp}", config.instance);
    let temporary = directory.join(format!(".{stem}-{}.partial", std::process::id()));
    let archive = directory.join(format!("{stem}.dump"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o640)
        .open(&temporary)?;
    nix::unistd::fchown(
        file.as_raw_fd(),
        Some(nix::unistd::Uid::from_raw(uid)),
        Some(nix::unistd::Gid::from_raw(gid)),
    )
    .map_err(|_| PostgresError::InvalidConfiguration("backup file ownership"))?;
    let mut child = Command::new("/usr/bin/podman")
        .args([
            "exec",
            config.container,
            "pg_dump",
            "--format=custom",
            "--username",
            config.username,
            "--dbname",
            config.database,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let result = copy_child_stdout(&mut child, &mut file);
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        let _ = fs::remove_file(&temporary);
        return result.map(|()| archive);
    }
    if !child.wait()?.success() {
        let _ = fs::remove_file(&temporary);
        return Err(PostgresError::BackupCommand);
    }
    file.sync_all()?;
    drop(file);
    // Hard-link publication is atomic and refuses to overwrite an archive
    // created by another concurrent invocation.
    fs::hard_link(&temporary, &archive)?;
    fs::remove_file(&temporary)?;
    File::open(directory)?.sync_all()?;
    prune_except(config, protected)?;
    Ok(archive)
}

fn copy_child_stdout(
    child: &mut std::process::Child,
    target: &mut File,
) -> Result<(), PostgresError> {
    let mut stdout = child.stdout.take().ok_or(PostgresError::BackupCommand)?;
    io::copy(&mut stdout, target)?;
    Ok(())
}

fn archives(config: &Config) -> Result<Vec<(u128, PathBuf)>, PostgresError> {
    let prefix = format!("{}-", config.instance);
    let mut found = Vec::new();
    for entry in fs::read_dir(config.backup_path)? {
        let entry = entry?;
        let metadata = entry.file_type()?;
        if !metadata.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(stamp) = name
            .strip_prefix(&prefix)
            .and_then(|s| s.strip_suffix(".dump"))
        else {
            continue;
        };
        if let Ok(stamp) = stamp.parse::<u128>() {
            found.push((stamp, entry.path()));
        }
    }
    found.sort_by_key(|archive| std::cmp::Reverse(archive.0));
    Ok(found)
}

fn prune_except(config: &Config, protected: Option<&Path>) -> Result<(), PostgresError> {
    let mut all = archives(config)?;
    if let Some(protected) = protected {
        all.retain(|(_, path)| path != protected);
    }
    let allowed = config.keep.saturating_sub(usize::from(protected.is_some()));
    for (_, path) in all.into_iter().skip(allowed) {
        fs::remove_file(path)?;
    }
    Ok(())
}

/// Restore an archive created for this instance. The app is stopped during the
/// restore and restarted afterward; caller must explicitly select an archive.
pub fn restore(config: &Config, archive: &Path) -> Result<(), PostgresError> {
    validate_archive(config, archive)?;
    let was_active = Command::new("/usr/bin/systemctl")
        .args(["is-active", "--quiet", config.application_unit])
        .status()?
        .success();
    if was_active {
        systemctl("stop", config.application_unit)?;
    }
    let result = restore_database(config, archive);
    if was_active {
        let start_result = systemctl("start", config.application_unit);
        if result.is_ok() {
            start_result?;
        }
    }
    result
}

fn validate_archive(config: &Config, archive: &Path) -> Result<(), PostgresError> {
    config.validate()?;
    let metadata = fs::symlink_metadata(archive)?;
    if !metadata.is_file() || archive.parent() != Some(Path::new(config.backup_path)) {
        return Err(PostgresError::InvalidConfiguration("restore archive"));
    }
    Ok(())
}

fn restore_database(config: &Config, archive: &Path) -> Result<(), PostgresError> {
    let file = File::open(archive)?;
    let mut child = Command::new("/usr/bin/podman")
        .args([
            "exec",
            "--interactive",
            config.container,
            "pg_restore",
            "--clean",
            "--if-exists",
            "--exit-on-error",
            "--no-owner",
            "--username",
            config.username,
            "--dbname",
            config.database,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let copy_result = io::copy(
        &mut io::BufReader::new(file),
        child.stdin.as_mut().ok_or(PostgresError::RestoreCommand)?,
    );
    drop(child.stdin.take());
    if let Err(error) = copy_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(PostgresError::Io(error));
    }
    if child.wait()?.success() {
        Ok(())
    } else {
        Err(PostgresError::RestoreCommand)
    }
}

fn systemctl(action: &str, unit: &str) -> Result<(), PostgresError> {
    let status = Command::new("/usr/bin/systemctl")
        .args([action, unit])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(PostgresError::ServiceCommand)
    }
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod tests;
