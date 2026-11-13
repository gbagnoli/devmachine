//! Install an on-demand, containerized API readiness gate without restarting `UniFi`.
use crate::UnifiError;
use skillet_core::{
    files::{FileMutationResource, FileReadResource, Ownership},
    system::ServiceResource,
};
use std::path::Path;

pub const UNIT: &str = "skillet-unifi-ready.service";
pub const CONTENT: &str = include_str!("readiness.service");

pub fn install<S, F>(system: &S, files: &F) -> Result<(), UnifiError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let root = Ownership::named(Some("root"), Some("root"));
    let units = image_units();
    if !skillet_core::vendor_units::reconcile(system, files, "unifi-ready", &units, &units, None)? {
        files.ensure_directory(Path::new("/etc/systemd/system"), Some(0o755), &root)?;
        let changed = files.ensure_file(
            &Path::new("/etc/systemd/system").join(UNIT),
            CONTENT.as_bytes(),
            Some(0o644),
            &root,
        )?;
        files.ensure_directory(
            Path::new("/var/lib/skillet/unifi-ready"),
            Some(0o700),
            &root,
        )?;
        let receipt = Path::new("/var/lib/skillet/unifi-ready/definition.applied");
        if changed || files.read_file(receipt)?.as_deref() != Some(CONTENT.as_bytes()) {
            system.daemon_reload()?;
            files.ensure_file(receipt, CONTENT.as_bytes(), Some(0o600), &root)?;
        }
    }
    Ok(())
}

pub fn image_units() -> Vec<skillet_core::vendor_units::Unit> {
    vec![skillet_core::vendor_units::Unit {
        name: UNIT,
        content: CONTENT.into(),
    }]
}
