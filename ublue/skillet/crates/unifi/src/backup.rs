//! Publish stable native `UniFi` archives without copying the live database.
use crate::UnifiError;
use sha2::{Digest as _, Sha256};
use skillet_core::{
    activation::{activate, ActivationRequest, ConsumerKind},
    files::{FileMutationResource, FileReadResource, Ownership},
    system::ServiceResource,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read as _, Seek as _, SeekFrom, Write as _},
    os::{
        fd::AsRawFd as _,
        unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _},
    },
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime},
};

pub const SOURCE: &str = "/var/lib/data/unifi/data/backup/autobackup";
pub const UNIT: &str = "skillet-unifi-backup.service";
pub const TIMER: &str = "skillet-unifi-backup.timer";
const KEEP: usize = 7;

#[derive(Clone, Copy)]
pub struct Config {
    pub files_root: &'static str,
    pub owner: &'static str,
    pub group: &'static str,
}

impl Config {
    pub fn destination(self) -> PathBuf {
        Path::new(self.files_root).join("backups/unifi")
    }
}

pub fn image_units() -> Vec<skillet_core::vendor_units::Unit> {
    vec![
        skillet_core::vendor_units::Unit {
            name: UNIT,
            content: "[Unit]\nDescription=Export completed UniFi backups to Syncthing\nAfter=unifi.service\nRequiresMountsFor=/var/lib/data\nConditionPathExists=/var/lib/data/unifi/data/backup/autobackup\n[Service]\nType=oneshot\nExecStart=/usr/bin/skillet unifi export --host-file /etc/skillet/host\nTimeoutStartSec=600\nUMask=0077\nNoNewPrivileges=yes\nProtectSystem=strict\nProtectHome=yes\nPrivateTmp=yes\nReadWritePaths=/var/lib/data /run\n".into(),
        },
        skillet_core::vendor_units::Unit {
            name: TIMER,
            content: "[Unit]\nDescription=Export UniFi backups daily\n[Timer]\nOnCalendar=*-*-* 03:30:00 Europe/Paris\nRandomizedDelaySec=0\nPersistent=true\n[Install]\nWantedBy=timers.target\n".into(),
        },
    ]
}

pub fn install<S, F>(system: &S, files: &F, config: Config) -> Result<(), UnifiError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    if !Path::new(config.files_root).is_absolute()
        || Path::new(config.files_root)
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(UnifiError::Backup("invalid backup destination"));
    }
    let owner = Ownership::named(Some(config.owner), Some(config.group));
    for path in [
        Path::new(config.files_root).join("backups"),
        config.destination(),
    ] {
        files.ensure_directory(&path, None, &owner)?;
    }
    let root = Ownership::named(Some("root"), Some("root"));
    files.ensure_directory(
        Path::new("/var/lib/skillet/unifi-backup"),
        Some(0o700),
        &root,
    )?;
    let units = image_units();
    let vendor =
        skillet_core::vendor_units::reconcile(system, files, "unifi-backup", &units, &units, None)?;
    let mut changed = false;
    if !vendor {
        files.ensure_directory(Path::new("/etc/systemd/system"), Some(0o755), &root)?;
        for unit in &units {
            changed |= files.ensure_file(
                &Path::new("/etc/systemd/system").join(unit.name),
                unit.content.as_bytes(),
                Some(0o644),
                &root,
            )?;
        }
    }
    system.service_enable(TIMER)?;
    activate(
        system,
        files,
        &ActivationRequest {
            service: TIMER.into(),
            state_path: "/var/lib/skillet/unifi-backup/timer.applied".into(),
            revision: units
                .iter()
                .flat_map(|unit| unit.content.as_bytes())
                .copied()
                .collect(),
            definition_changed: changed,
            reload_daemon: true,
            consumer_kind: ConsumerKind::Persistent,
        },
    )?;
    Ok(())
}

fn managed(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.starts_with("autobackup_")
                && path.extension().is_some_and(|extension| extension == "unf")
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
        })
}

fn regular(path: &Path) -> Result<File, UnifiError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(UnifiError::Backup("archive is not a regular file"));
    }
    Ok(file)
}

fn digest(file: &mut File) -> Result<Vec<u8>, UnifiError> {
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash.finalize().to_vec())
}

#[derive(Debug, Default)]
pub struct Outcome {
    pub published: usize,
    pub pruned: usize,
}

/// Only finalized-looking archives stable for a minute are eligible. Publication
/// is atomic, content-verified and non-overwriting; prune only after all copies pass.
pub fn export(
    source: &Path,
    destination: &Path,
    uid: u32,
    gid: u32,
    now: SystemTime,
) -> Result<Outcome, UnifiError> {
    for directory in [source, destination] {
        if directory.canonicalize()? != directory || !directory.is_dir() {
            return Err(UnifiError::Backup("linked or invalid backup directory"));
        }
    }
    let mut candidates = Vec::new();
    for entry in fs::read_dir(source)? {
        let path = entry?.path();
        if !managed(&path) {
            continue;
        }
        let metadata = regular(&path)?.metadata()?;
        if metadata.len() > 0
            && now.duration_since(metadata.modified()?).unwrap_or_default()
                >= Duration::from_secs(60)
        {
            candidates.push((metadata.modified()?, path));
        }
    }
    candidates.sort_by(|a, b| b.cmp(a));
    if candidates.is_empty() {
        return Err(UnifiError::Backup(
            "no completed automatic backups available",
        ));
    }
    let mut outcome = Outcome::default();
    for (_, path) in candidates.into_iter().take(KEEP) {
        let name = path
            .file_name()
            .ok_or(UnifiError::Backup("invalid backup filename"))?;
        let target = destination.join(name);
        let mut input = regular(&path)?;
        let before = input.metadata()?;
        if before.len() == 0
            || now.duration_since(before.modified()?).unwrap_or_default() < Duration::from_secs(60)
        {
            continue;
        }
        let hash = digest(&mut input)?;
        if fs::symlink_metadata(&target).is_ok() {
            if digest(&mut regular(&target)?)? != hash {
                return Err(UnifiError::Backup("existing export differs from source"));
            }
            continue;
        }
        let mut temporary = tempfile::Builder::new()
            .prefix(".syncthing.unifi-")
            .suffix(".tmp")
            .tempfile_in(destination)?;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
        nix::unistd::fchown(
            temporary.as_file().as_raw_fd(),
            Some(nix::unistd::Uid::from_raw(uid)),
            Some(nix::unistd::Gid::from_raw(gid)),
        )
        .map_err(|_| UnifiError::Backup("could not set export ownership"))?;
        input.seek(SeekFrom::Start(0))?;
        std::io::copy(&mut input, &mut temporary)?;
        temporary.flush()?;
        let after = fs::symlink_metadata(&path)?;
        if !after.is_file()
            || before.dev() != after.dev()
            || before.ino() != after.ino()
            || before.len() != after.len()
            || before.modified()? != after.modified()?
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
            || hash != digest(temporary.as_file_mut())?
            || hash != digest(&mut input)?
        {
            return Err(UnifiError::Backup("source changed during export"));
        }
        temporary
            .as_file()
            .set_times(fs::FileTimes::new().set_modified(before.modified()?))?;
        temporary.as_file().sync_all()?;
        temporary
            .persist_noclobber(&target)
            .map_err(|_| UnifiError::Backup("could not atomically publish backup"))?;
        File::open(destination)?.sync_all()?;
        outcome.published += 1;
    }
    let mut archives = Vec::new();
    for entry in fs::read_dir(destination)? {
        let path = entry?.path();
        if managed(&path) {
            archives.push((regular(&path)?.metadata()?.modified()?, path));
        }
    }
    archives.sort_by(|a, b| b.cmp(a));
    for (_, path) in archives.into_iter().skip(KEEP) {
        fs::remove_file(path)?;
        outcome.pruned += 1;
    }
    File::open(destination)?.sync_all()?;
    Ok(outcome)
}

#[cfg(test)]
#[path = "backup/tests.rs"]
mod tests;
