//! Caller composition for HA with a local `UniFi` controller.
use crate::{ApplyError, HostProfile};
use skillet_core::{
    files::{FileMutationResource, FileReadResource, Ownership},
    system::ServiceResource,
};
use std::path::Path;

const PATH: &str = "/etc/systemd/system/homeassistant.service.d/50-skillet-unifi-ready.conf";
const CONTENT: &str =
    "[Unit]\nWants=skillet-unifi-ready.service\nAfter=skillet-unifi-ready.service\n";

pub fn install<S, F>(system: &S, files: &F, profile: &HostProfile) -> Result<(), ApplyError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let root = Ownership::named(Some("root"), Some("root"));
    let selected = profile.supports_service("homeassistant") && profile.supports_service("unifi");
    let previous = files.read_regular_file(Path::new(PATH))?;
    if previous
        .as_ref()
        .is_some_and(|bytes| bytes != CONTENT.as_bytes())
    {
        return Err(ApplyError::FixtureInput(
            "HA readiness drop-in has unknown edits".into(),
        ));
    }
    let receipt = Path::new("/var/lib/skillet/unifi-ready/ha.applied");
    if !selected && previous.is_none() && files.read_file(receipt)?.is_none() {
        return Ok(());
    }
    let changed = if selected {
        files.ensure_directory(
            Path::new(PATH)
                .parent()
                .ok_or_else(|| ApplyError::FixtureInput("invalid readiness drop-in path".into()))?,
            Some(0o755),
            &root,
        )?;
        files.ensure_file(Path::new(PATH), CONTENT.as_bytes(), Some(0o644), &root)?
    } else {
        match files.read_regular_file(Path::new(PATH))? {
            None => false,
            Some(bytes) if bytes == CONTENT.as_bytes() => files.delete_file(Path::new(PATH))?,
            Some(_) => {
                return Err(ApplyError::FixtureInput(
                    "HA readiness drop-in has unknown edits".into(),
                ))
            }
        }
    };
    files.ensure_directory(
        Path::new("/var/lib/skillet/unifi-ready"),
        Some(0o700),
        &root,
    )?;
    let revision: &[u8] = if selected {
        CONTENT.as_bytes()
    } else {
        b"disabled"
    };
    if changed || files.read_file(receipt)?.as_deref() != Some(revision) {
        system.daemon_reload()?;
        files.ensure_file(receipt, revision, Some(0o600), &root)?;
    }
    Ok(())
}
