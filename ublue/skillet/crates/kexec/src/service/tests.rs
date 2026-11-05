use super::*;
use skillet_core::{
    files::FileReadResource,
    test_utils::{MockFiles, MockSystem},
};
use std::sync::atomic::Ordering;

#[test]
fn apply_only_installs_and_preserves_definition_on_repeat() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    let binary = "/var/usrlocal/bin/skillet-fixture";
    apply(&system, &files, binary).unwrap();
    let first = files.read_file(Path::new(PATH)).unwrap().unwrap();
    assert!(String::from_utf8_lossy(&first).contains("skillet-fixture kexec run"));
    assert!(!String::from_utf8_lossy(&first).contains("[Install]"));
    assert!(system
        .services
        .lock()
        .unwrap()
        .contains_key("daemon-reload"));
    system.services.lock().unwrap().clear();
    apply(&system, &files, binary).unwrap();
    assert_eq!(files.read_file(Path::new(PATH)).unwrap().unwrap(), first);
    assert_eq!(system.start_count.load(Ordering::SeqCst), 0);
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 0);
    assert!(system.services.lock().unwrap().is_empty());
}

#[test]
fn unsafe_unit_executable_is_rejected() {
    for binary in [
        "relative",
        "/bin/tool\nExecStart=other",
        "/bin/%H",
        "/bin/a b",
    ] {
        assert!(render(binary).is_err());
    }
}

#[test]
fn interrupted_reload_is_retried_even_when_the_definition_already_exists() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    system.fail_reload_once.store(true, Ordering::SeqCst);
    let binary = "/var/usrlocal/bin/skillet-fixture";
    assert!(apply(&system, &files, binary).is_err());
    assert!(files.read_file(Path::new(PATH)).unwrap().is_some());
    assert!(files
        .read_file(Path::new("/var/lib/skillet/kexec/definition.applied"))
        .unwrap()
        .is_none());
    apply(&system, &files, binary).unwrap();
    assert!(system
        .services
        .lock()
        .unwrap()
        .contains_key("daemon-reload"));
    assert_eq!(system.start_count.load(Ordering::SeqCst), 0);
}
