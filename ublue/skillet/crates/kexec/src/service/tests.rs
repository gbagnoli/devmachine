use super::*;
use skillet_core::{
    files::FileReadResource,
    test_utils::{MockFiles, MockSystem},
};
use std::sync::atomic::Ordering;

#[test]
fn image_migration_retires_overrides_and_disables_legacy_timers() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    let binary = "/var/usrlocal/bin/skillet-fixture";
    apply(&system, &files, binary).unwrap();
    for unit in image_units().unwrap() {
        files.files.lock().unwrap().insert(
            format!("/usr/lib/systemd/system/{}", unit.name),
            unit.content.into_bytes(),
        );
    }
    apply(&system, &files, binary).unwrap();
    assert!(files.read_regular_file(Path::new(PATH)).unwrap().is_none());
    assert!(files
        .read_regular_file(Path::new(
            "/etc/systemd/system/skillet-kexec-activate.service.skillet-legacy"
        ))
        .unwrap()
        .is_some());
    assert!(!system.service_is_active(UNIT).unwrap());
    assert!(!system
        .service_is_active("skillet-firmware-reboot.service")
        .unwrap());
    assert!(!system.service_is_active(TIMER).unwrap());
    assert_eq!(
        system
            .services
            .lock()
            .unwrap()
            .get(TIMER)
            .map(String::as_str),
        Some("disabled")
    );
    assert_eq!(
        system
            .services
            .lock()
            .unwrap()
            .get(REBOOT_TIMER)
            .map(String::as_str),
        Some("disabled")
    );
    assert!(
        String::from_utf8(files.read_file(Path::new(UPDATE_DROPIN)).unwrap().unwrap())
            .unwrap()
            .contains("OnSuccess=skillet-kexec-activate.service")
    );
    assert!(!system.service_is_active(REBOOT_TIMER).unwrap());
    apply(&system, &files, binary).unwrap();
}

#[test]
fn apply_only_installs_and_preserves_definition_on_repeat() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    let binary = "/var/usrlocal/bin/skillet-fixture";
    apply(&system, &files, binary).unwrap();
    let first = files.read_file(Path::new(PATH)).unwrap().unwrap();
    assert!(String::from_utf8_lossy(&first).contains("skillet-fixture kexec run"));
    assert!(!String::from_utf8_lossy(&first).contains("[Install]"));
    let daily = files
        .read_file(Path::new(
            "/etc/systemd/system/skillet-kexec-activate.timer",
        ))
        .unwrap()
        .unwrap();
    let daily = String::from_utf8(daily).unwrap();
    assert!(daily.contains("OnCalendar=Mon..Sat *-*-01..07 05:30:00 Europe/Paris"));
    assert!(daily.contains("OnCalendar=*-*-08..31 05:30:00 Europe/Paris"));
    assert!(daily.contains("Persistent=false"));
    let monthly = files
        .read_file(Path::new(
            "/etc/systemd/system/skillet-firmware-reboot.timer",
        ))
        .unwrap()
        .unwrap();
    assert!(String::from_utf8(monthly)
        .unwrap()
        .contains("OnCalendar=Sun *-*-01..07 05:30:00 Europe/Paris"));
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
    assert!(!system.services.lock().unwrap().contains_key(UNIT));
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
