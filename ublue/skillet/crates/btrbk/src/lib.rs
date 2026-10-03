use skillet_core::{
    files::{FileError, FileMutationResource, StorageResource},
    system::{SystemError, SystemResource},
};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

const DATA_MOUNT: &str = "/var/lib/data";
const CONFIG_PATH: &str = "/etc/btrbk/btrbk.conf";
const SERVICE_PATH: &str = "/etc/systemd/system/skillet-btrbk.service";
const TIMER_PATH: &str = "/etc/systemd/system/skillet-btrbk.timer";

#[derive(Debug, Error)]
pub enum BtrbkError {
    #[error("File error: {0}")]
    File(#[from] FileError),
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("Invalid Btrfs subvolume path '{0}': use safe relative path components beneath /var/lib/data")]
    InvalidSubvolume(String),
}

/// Caller-selected paths relative to `/var/lib/data`. The parent data
/// subvolume is deliberately not a default source.
pub struct BtrbkConfig {
    pub snapshot_subvolumes: Vec<PathBuf>,
}

pub fn apply<S, F>(system: &S, files: &F, config: &BtrbkConfig) -> Result<(), BtrbkError>
where
    S: SystemResource + ?Sized,
    F: FileMutationResource + StorageResource + ?Sized,
{
    if config.snapshot_subvolumes.is_empty() {
        return Ok(());
    }

    files.require_btrfs_subvolume_mount(Path::new(DATA_MOUNT), Path::new("/var"), "/data")?;

    let mut sources = Vec::with_capacity(config.snapshot_subvolumes.len());
    for relative in &config.snapshot_subvolumes {
        validate_relative_subvolume(relative)?;
        let source = Path::new(DATA_MOUNT).join(relative);
        files.require_btrfs_subvolume(&source)?;
        let snapshot_dir = Path::new(DATA_MOUNT).join("snapshots").join(relative);
        files.ensure_directory(&snapshot_dir, Some(0o755), None, None)?;
        sources.push((
            relative,
            snapshot_dir
                .strip_prefix(DATA_MOUNT)
                .map_err(|_| BtrbkError::InvalidSubvolume(relative.display().to_string()))?
                .to_path_buf(),
        ));
    }

    files.ensure_directory(
        Path::new("/etc/btrbk"),
        Some(0o755),
        Some("root"),
        Some("root"),
    )?;
    let config_changed = files.ensure_file(
        Path::new(CONFIG_PATH),
        render_config(&sources).as_bytes(),
        Some(0o644),
        Some("root"),
        Some("root"),
    )?;
    let service_changed = files.ensure_file(
        Path::new(SERVICE_PATH),
        service_unit().as_bytes(),
        Some(0o644),
        Some("root"),
        Some("root"),
    )?;
    let timer_changed = files.ensure_file(
        Path::new(TIMER_PATH),
        timer_unit().as_bytes(),
        Some(0o644),
        Some("root"),
        Some("root"),
    )?;

    if config_changed || service_changed || timer_changed {
        system.daemon_reload()?;
    }
    system.service_enable("skillet-btrbk.timer")?;
    if !system.service_is_active("skillet-btrbk.timer")? {
        system.service_start("skillet-btrbk.timer")?;
    }
    Ok(())
}

fn validate_relative_subvolume(path: &Path) -> Result<(), BtrbkError> {
    if path.as_os_str().is_empty()
        || path.components().any(|component| match component {
            Component::Normal(value) => value.to_str().is_none_or(|part| {
                part.is_empty()
                    || !part
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
            }),
            _ => true,
        })
    {
        return Err(BtrbkError::InvalidSubvolume(path.display().to_string()));
    }
    Ok(())
}

fn render_config(sources: &[(&PathBuf, PathBuf)]) -> String {
    let mut output = String::from(
        "timestamp_format long\nsnapshot_preserve_min 6h\nsnapshot_preserve 24h 31d 6m\n\
         volume /var/lib/data\n",
    );
    for (source, snapshot_dir) in sources {
        output.push_str("  subvolume ");
        output.push_str(&source.display().to_string());
        output.push_str("\n    snapshot_dir ");
        output.push_str(&snapshot_dir.display().to_string());
        output.push('\n');
    }
    output
}

fn service_unit() -> &'static str {
    "[Unit]\n\
     Description=Create configured Btrfs snapshots with btrbk\n\
     Requires=var-lib-data.mount\n\
     After=var-lib-data.mount\n\
     BindsTo=var-lib-data.mount\n\
     AssertPathIsMountPoint=/var/lib/data\n\
     [Service]\n\
     Type=oneshot\n\
     ExecStartPre=/var/usrlocal/bin/skillet-data-prepare\n\
     ExecStart=/usr/bin/btrbk --config /etc/btrbk/btrbk.conf run\n"
}

fn timer_unit() -> &'static str {
    "[Unit]\n\
     Description=Hourly configured Btrfs snapshots\n\
     [Timer]\n\
     OnCalendar=hourly\n\
     Persistent=true\n\
     Unit=skillet-btrbk.service\n\
     [Install]\n\
     WantedBy=timers.target\n"
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
