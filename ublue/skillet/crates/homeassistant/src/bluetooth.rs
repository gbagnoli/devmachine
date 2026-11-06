//! A filtering, independently confined host-bus bridge for the selected adapter.
use crate::Error;
use skillet_core::{
    activation::{activate, ActivationRequest, ConsumerKind},
    files::{FileMutationResource, FileReadResource, Ownership},
    system::ServiceResource,
};
use std::path::Path;

const POLICY: &[u8] = include_bytes!("skillet-ha-bluetooth.cil");
const POLICY_UNIT: &[u8] = include_bytes!("skillet-ha-bluetooth-policy.service");
const BROKER_UNIT: &[u8] = include_bytes!("skillet-ha-bluetooth.service");
const READY: &[u8] = include_bytes!("wait-ready.sh");
pub const UNIT: &str = "skillet-ha-bluetooth.service";

pub fn apply<S, F>(system: &S, files: &F) -> Result<(), Error>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let root = Ownership::named(Some("root"), Some("root"));
    let directory = Path::new("/etc/skillet/homeassistant-bluetooth");
    let state = Path::new("/var/lib/skillet/homeassistant");
    files.ensure_directory(directory, Some(0o755), &root)?;
    files.ensure_directory(state, Some(0o700), &root)?;
    let policy_changed = files.ensure_file(
        &directory.join("skillet-ha-bluetooth.cil"),
        POLICY,
        Some(0o644),
        &root,
    )?;
    let unit_changed = files.ensure_file(
        Path::new("/etc/systemd/system/skillet-ha-bluetooth-policy.service"),
        POLICY_UNIT,
        Some(0o644),
        &root,
    )?;
    activate(
        system,
        files,
        &ActivationRequest {
            service: "skillet-ha-bluetooth-policy.service".into(),
            state_path: state.join("bluetooth-policy.applied"),
            revision: [POLICY, POLICY_UNIT].concat(),
            definition_changed: policy_changed || unit_changed,
            reload_daemon: true,
            consumer_kind: ConsumerKind::OneShot,
        },
    )?;
    let ready_changed =
        files.ensure_file(&directory.join("wait-ready.sh"), READY, Some(0o644), &root)?;
    let broker_changed = files.ensure_file(
        Path::new("/etc/systemd/system/skillet-ha-bluetooth.service"),
        BROKER_UNIT,
        Some(0o644),
        &root,
    )?;
    system.service_enable(UNIT)?;
    activate(
        system,
        files,
        &ActivationRequest {
            service: UNIT.into(),
            state_path: state.join("bluetooth.applied"),
            revision: [POLICY, POLICY_UNIT, BROKER_UNIT, READY].concat(),
            definition_changed: broker_changed || ready_changed || policy_changed || unit_changed,
            reload_daemon: true,
            consumer_kind: ConsumerKind::Persistent,
        },
    )?;
    Ok(())
}
