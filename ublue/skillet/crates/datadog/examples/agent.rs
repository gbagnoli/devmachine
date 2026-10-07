//! Offline Quadlet fixture: no real containers, credentials, or host mutations.
use skillet_core::test_utils::{MockFiles, MockSystem};
use skillet_datadog::{apply, RuntimeConfig};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let files = MockFiles::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    apply(
        &MockSystem::new(),
        &files,
        r#"{"version":1,"api_key":"00000000000000000000000000000000","site":"datadoghq.eu","tags":["env:test"]}"#,
        &RuntimeConfig {
            hostname: "fixture-test-monitoring",
            network_monitoring: true,
            monitored_units: &["syncthing.service".into(), "btrbk.timer".into()],
        },
    )?;
    let state = files
        .files
        .lock()
        .map_err(|_| std::io::Error::other("fixture lock poisoned"))?;
    let quadlet = state
        .get("/etc/containers/systemd/datadog-agent.container")
        .ok_or_else(|| std::io::Error::other("fixture Quadlet missing"))?;
    print!("{}", std::str::from_utf8(quadlet)?);
    Ok(())
}
