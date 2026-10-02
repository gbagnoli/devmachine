use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};

#[test]
fn fixture_requires_explicit_inputs() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    assert!(matches!(
        apply(&system, &files),
        Err(ApplyError::FixtureInput(_))
    ));
}

#[test]
fn fixture_uses_the_host_container_resources() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    files
        .files
        .lock()
        .unwrap()
        .insert(format!("{INPUT_DIR}/config"), b"one".to_vec());
    files
        .files
        .lock()
        .unwrap()
        .insert(format!("{INPUT_DIR}/secret"), b"dummy".to_vec());
    apply(&system, &files).unwrap();
    let resources = files.files.lock().unwrap();
    let quadlet = String::from_utf8(
        resources["/etc/containers/systemd/skillet-smoke-fixture.container"].clone(),
    )
    .unwrap();
    assert!(quadlet.contains("Secret=skillet-smoke-dummy"));
    assert!(quadlet.contains("WantedBy=multi-user.target"));
    assert!(resources.contains_key("/var/lib/skillet/containers/skillet-smoke-fixture.applied"));
}
