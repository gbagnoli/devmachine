//! Preserve verified legacy definitions before selecting immutable defaults.
use super::{
    image, prepare, render_config, service_unit, sources, timer_unit, BtrbkConfig, BtrbkError,
    CONFIG_PATH, SERVICE_PATH, TIMER_PATH,
};
use sha2::{Digest, Sha256};
use skillet_core::{
    activation::{self, ActivationRequest, ConsumerKind},
    files::{FileMutationResource, FileReadResource, Ownership, StorageResource},
    system::ServiceResource,
};
use std::path::{Path, PathBuf};
const STATE: &str = "/var/lib/skillet/btrbk/timer.applied";
const TIMER: &str = "skillet-btrbk.timer";

pub(super) fn apply<S, F>(
    system: &S,
    files: &F,
    config: &BtrbkConfig,
    binary: &str,
) -> Result<(), BtrbkError>
where
    S: ServiceResource + ?Sized,
    F: FileReadResource + FileMutationResource + StorageResource + ?Sized,
{
    let vendor = image::render(config, binary)?;
    for artifact in &vendor {
        if files
            .read_regular_file(Path::new(artifact.path))?
            .as_deref()
            != Some(artifact.content.as_bytes())
        {
            return Err(BtrbkError::Conflict(format!(
                "missing or mismatched image artifact: {}",
                artifact.path
            )));
        }
    }
    let legacy_config = render_config(&sources(config)?);
    let legacy = [
        (CONFIG_PATH, legacy_config.as_str()),
        (SERVICE_PATH, service_unit()),
        (TIMER_PATH, timer_unit()),
    ];
    let revision = revision(legacy.iter().map(|(_, content)| content.as_bytes()));
    let applied = files.read_regular_file(Path::new(STATE))?;
    let mut old = Vec::new();
    for (path, content) in legacy {
        let archive = PathBuf::from(format!("{path}.skillet-legacy"));
        let current = files.read_regular_file(Path::new(path))?;
        let backup = files.read_regular_file(&archive)?;
        if current
            .as_deref()
            .is_some_and(|bytes| bytes != content.as_bytes())
            || backup
                .as_deref()
                .is_some_and(|bytes| bytes != content.as_bytes())
        {
            return Err(BtrbkError::Conflict(format!("custom definition or recovery copy: {path}; inspect and preserve it before migration")));
        }
        if current.is_some() {
            old.push((path, content, archive));
        }
    }
    if !old.is_empty() && applied.as_deref() != Some(revision.as_bytes()) {
        return Err(BtrbkError::Conflict(
            "legacy applied receipt does not match; inspect retained definitions".into(),
        ));
    }
    prepare(files, config)?;
    let image_revision = revision_for_image(&vendor);
    let pending = applied.as_deref() != Some(image_revision.as_bytes());
    if !old.is_empty() || pending {
        if system.service_is_busy("skillet-btrbk.service")? {
            return Err(BtrbkError::Conflict(
                "snapshot service is running; retry after completion".into(),
            ));
        }
        system.service_stop(TIMER)?;
        if system.service_is_busy("skillet-btrbk.service")? {
            return Err(BtrbkError::Conflict(
                "snapshot started before timer stopped; retry after completion to reactivate timer"
                    .into(),
            ));
        }
    }
    let mut changed = false;
    for (path, content, archive) in old {
        changed |= files.retire_file(Path::new(path), content.as_bytes(), &archive)?;
    }
    if changed || pending {
        system.daemon_reload()?;
        system.service_reenable(TIMER)?;
    }
    files.ensure_directory(
        Path::new("/var/lib/skillet/btrbk"),
        Some(0o755),
        &Ownership::named(Some("root"), Some("root")),
    )?;
    activation::activate(
        system,
        files,
        &ActivationRequest {
            service: TIMER.into(),
            state_path: STATE.into(),
            revision: image_revision.into_bytes(),
            definition_changed: changed,
            reload_daemon: false,
            consumer_kind: ConsumerKind::Persistent,
        },
    )?;
    Ok(())
}

fn revision<'a>(parts: impl Iterator<Item = &'a [u8]>) -> String {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update(part);
    }
    hex::encode(hash.finalize())
}

fn revision_for_image(parts: &[image::Artifact]) -> String {
    revision(parts.iter().map(|p| p.content.as_bytes()))
}
