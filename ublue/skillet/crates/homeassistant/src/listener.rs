//! Explicit host listener boundary independent of application-owned settings.
use crate::Error;
use skillet_core::{
    activation::{activate, ActivationRequest, ConsumerKind},
    files::{FileMutationResource, FileReadResource, Ownership},
    system::ServiceResource,
};
use std::path::Path;
pub const UNIT: &str = "skillet-homeassistant-listener.service";
pub fn apply<S, F>(system: &S, files: &F) -> Result<(), Error>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let root = Ownership::named(Some("root"), Some("root"));
    files.ensure_directory(Path::new("/etc/skillet/homeassistant"), Some(0o755), &root)?;
    files.ensure_directory(Path::new("/etc/systemd/system"), Some(0o755), &root)?;
    files.ensure_directory(
        Path::new("/var/lib/skillet/homeassistant"),
        Some(0o700),
        &root,
    )?;
    let rules = include_bytes!("listener.nft");
    let unit = include_bytes!("listener.service");
    let a = files.ensure_file(
        Path::new("/etc/skillet/homeassistant/listener.nft"),
        rules,
        Some(0o644),
        &root,
    )?;
    let b = files.ensure_file(
        &Path::new("/etc/systemd/system").join(UNIT),
        unit,
        Some(0o644),
        &root,
    )?;
    system.service_enable(UNIT)?;
    activate(
        system,
        files,
        &ActivationRequest {
            service: UNIT.into(),
            state_path: "/var/lib/skillet/homeassistant/listener.applied".into(),
            revision: [rules.as_slice(), unit.as_slice()].concat(),
            definition_changed: a || b,
            reload_daemon: true,
            consumer_kind: ConsumerKind::OneShot,
        },
    )?;
    Ok(())
}
