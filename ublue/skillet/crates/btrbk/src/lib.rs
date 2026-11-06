pub mod image;
mod migration;

use sha2::{Digest, Sha256};
use skillet_core::{
    activation::{self, ActivationRequest, ConsumerKind},
    files::{FileError, FileMutationResource, FileReadResource, Ownership, StorageResource},
    system::{ServiceResource, SystemError},
};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

const DATA_MOUNT: &str = "/var/lib/data";
const CONFIG_PATH: &str = "/etc/btrbk/btrbk.conf";
const SERVICE_PATH: &str = "/etc/systemd/system/skillet-btrbk.service";
const TIMER_PATH: &str = "/etc/systemd/system/skillet-btrbk.timer";

#[derive(Debug, Error)]
pub enum BtrbkError {
    #[error("btrbk ownership conflict: {0}")]
    Conflict(String),
    #[error("File error: {0}")]
    File(#[from] FileError),
    #[error("System error: {0}")]
    System(#[from] SystemError),
    #[error("Activation error: {0}")]
    Activation(#[from] skillet_core::activation::ActivationError),
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
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    if config.snapshot_subvolumes.is_empty() {
        return Ok(());
    }

    prepare(files, config)?;
    let sources = sources(config)?;

    files.ensure_directory(
        Path::new("/etc/btrbk"),
        Some(0o755),
        &Ownership::named(Some("root"), Some("root")),
    )?;
    let config_changed = files.ensure_file(
        Path::new(CONFIG_PATH),
        render_config(&sources).as_bytes(),
        Some(0o644),
        &Ownership::named(Some("root"), Some("root")),
    )?;
    let service_changed = files.ensure_file(
        Path::new(SERVICE_PATH),
        service_unit().as_bytes(),
        Some(0o644),
        &Ownership::named(Some("root"), Some("root")),
    )?;
    let timer_changed = files.ensure_file(
        Path::new(TIMER_PATH),
        timer_unit().as_bytes(),
        Some(0o644),
        &Ownership::named(Some("root"), Some("root")),
    )?;

    let definition_changed = config_changed || service_changed || timer_changed;
    let mut hasher = Sha256::new();
    hasher.update(render_config(&sources).as_bytes());
    hasher.update(service_unit().as_bytes());
    hasher.update(timer_unit().as_bytes());
    let revision = hex::encode(hasher.finalize());
    if definition_changed
        || files
            .read_file(Path::new("/var/lib/skillet/btrbk/timer.applied"))?
            .as_deref()
            != Some(revision.as_bytes())
    {
        system.service_reenable("skillet-btrbk.timer")?;
    } else {
        system.service_enable("skillet-btrbk.timer")?;
    }
    let state_dir = Path::new("/var/lib/skillet/btrbk");
    files.ensure_directory(
        state_dir,
        Some(0o755),
        &Ownership::named(Some("root"), Some("root")),
    )?;
    activation::activate(
        system,
        files,
        &ActivationRequest {
            service: "skillet-btrbk.timer".to_string(),
            state_path: state_dir.join("timer.applied"),
            revision: revision.into_bytes(),
            definition_changed,
            reload_daemon: true,
            consumer_kind: ConsumerKind::Persistent,
        },
    )?;
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

fn sources(config: &BtrbkConfig) -> Result<Vec<(&PathBuf, PathBuf)>, BtrbkError> {
    config
        .snapshot_subvolumes
        .iter()
        .map(|relative| {
            validate_relative_subvolume(relative)?;
            Ok((relative, Path::new("snapshots").join(relative)))
        })
        .collect()
}

pub fn prepare<F>(files: &F, config: &BtrbkConfig) -> Result<(), BtrbkError>
where
    F: FileReadResource + FileMutationResource + StorageResource + ?Sized,
{
    let selected = sources(config)?;
    if selected.is_empty() {
        return Ok(());
    }
    files.require_btrfs_subvolume_mount(Path::new(DATA_MOUNT), Path::new("/var"), "/data")?;
    for (source, _) in &selected {
        files.require_btrfs_subvolume(&Path::new(DATA_MOUNT).join(source))?;
    }
    for (_, destination) in selected {
        files.ensure_directory(
            &Path::new(DATA_MOUNT).join(destination),
            Some(0o755),
            &Ownership::default(),
        )?;
    }
    Ok(())
}

pub fn apply_for_host<S, F>(
    system: &S,
    files: &F,
    config: &BtrbkConfig,
    binary: &str,
) -> Result<(), BtrbkError>
where
    S: ServiceResource + ?Sized,
    F: FileReadResource + FileMutationResource + StorageResource + ?Sized,
{
    if config.snapshot_subvolumes.is_empty() {
        return Ok(());
    }
    match files.read_regular_file(Path::new(image::MARKER))? {
        None => {
            for path in [image::CONFIG, image::SERVICE, image::TIMER] {
                if files.read_regular_file(Path::new(path))?.is_some() {
                    return Err(BtrbkError::Conflict(format!(
                        "partial image installation: {path} exists without ownership marker"
                    )));
                }
            }
            // Rollback reconciliation may restore absent generated definitions,
            // but must not overwrite edits made after the image migration.
            let legacy_config = render_config(&sources(config)?);
            let legacy = [
                (CONFIG_PATH, legacy_config.as_str()),
                (SERVICE_PATH, service_unit()),
                (TIMER_PATH, timer_unit()),
            ];
            let mut restoring = false;
            for (path, _) in legacy {
                restoring |= files
                    .read_regular_file(Path::new(&format!("{path}.skillet-legacy")))?
                    .is_some();
            }
            if restoring {
                for (path, expected) in legacy {
                    if files
                        .read_regular_file(Path::new(path))?
                        .as_deref()
                        .is_some_and(|bytes| bytes != expected.as_bytes())
                    {
                        return Err(BtrbkError::Conflict(format!(
                            "custom rollback definition: {path}; preserve it before reconciliation"
                        )));
                    }
                }
            }
            apply(system, files, config)
        }
        Some(_) => migration::apply(system, files, config, binary),
    }
}
