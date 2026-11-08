use super::*;
use crate::test_utils::{MockFiles, MockSystem};
use std::sync::atomic::Ordering;

fn definitions() -> (Vec<Unit>, Vec<Unit>) {
    let make = |body: &str| {
        vec![
            Unit {
                name: "fixture.service",
                content: body.into(),
            },
            Unit {
                name: "fixture.timer",
                content: "[Timer]\nOnCalendar=hourly\n".into(),
            },
        ]
    };
    (
        make("[Service]\nExecStart=/usr/bin/true\n"),
        make("[Service]\nExecStart=/bin/true\n"),
    )
}

fn seed(files: &MockFiles, root: &str, units: &[Unit]) {
    for unit in units {
        files.files.lock().unwrap().insert(
            format!("{root}/{}", unit.name),
            unit.content.as_bytes().to_vec(),
        );
    }
}

#[test]
fn migration_recovers_interrupted_reload_and_preserves_old_bytes() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    let (vendor, legacy) = definitions();
    seed(&files, "/usr/lib/systemd/system", &vendor);
    seed(&files, "/etc/systemd/system", &legacy);
    system.fail_reload_once.store(true, Ordering::SeqCst);
    assert!(reconcile(&system, &files, "fixture", &vendor, &legacy, None).is_err());
    assert!(files
        .read_regular_file(Path::new("/var/lib/skillet/vendor-units/fixture.applied"))
        .unwrap()
        .is_none());
    assert!(reconcile(&system, &files, "fixture", &vendor, &legacy, None).unwrap());
    for old in &legacy {
        assert!(files
            .read_regular_file(&Path::new("/etc/systemd/system").join(old.name))
            .unwrap()
            .is_none());
        assert_eq!(
            files
                .read_regular_file(&PathBuf::from(format!(
                    "/etc/systemd/system/{}.skillet-legacy",
                    old.name
                )))
                .unwrap()
                .unwrap(),
            old.content.as_bytes()
        );
    }
    system.services.lock().unwrap().clear();
    assert!(reconcile(&system, &files, "fixture", &vendor, &legacy, None).unwrap());
    assert!(system.services.lock().unwrap().is_empty());
}

#[test]
fn preflight_preserves_all_definitions_for_custom_or_incomplete_groups() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    let (vendor, legacy) = definitions();
    assert!(!reconcile(&system, &files, "fixture", &vendor, &legacy, None).unwrap());
    seed(&files, "/usr/lib/systemd/system", &vendor[..1]);
    assert!(reconcile(&system, &files, "fixture", &vendor, &legacy, None).is_err());
    seed(&files, "/usr/lib/systemd/system", &vendor);
    seed(&files, "/etc/systemd/system", &legacy);
    files.files.lock().unwrap().insert(
        "/etc/systemd/system/fixture.timer".into(),
        b"administrator override".to_vec(),
    );
    assert!(reconcile(&system, &files, "fixture", &vendor, &legacy, None).is_err());
    assert!(files
        .read_regular_file(Path::new("/etc/systemd/system/fixture.service"))
        .unwrap()
        .is_some());
    assert!(system.services.lock().unwrap().is_empty());
}

#[test]
fn busy_consumer_refuses_retirement_and_activation() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    let (vendor, legacy) = definitions();
    seed(&files, "/usr/lib/systemd/system", &vendor);
    seed(&files, "/etc/systemd/system", &legacy);
    system
        .services
        .lock()
        .unwrap()
        .insert("fixture.service".into(), "activating".into());
    assert!(reconcile(&system, &files, "fixture", &vendor, &legacy, None).is_err());
    assert!(files
        .read_regular_file(Path::new("/etc/systemd/system/fixture.service"))
        .unwrap()
        .is_some());
}

#[test]
fn mismatched_receipt_refuses_before_any_retirement() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    let (vendor, legacy) = definitions();
    seed(&files, "/usr/lib/systemd/system", &vendor);
    seed(&files, "/etc/systemd/system", &legacy);
    let receipt = LegacyReceipt {
        path: "/var/lib/skillet/fixture.applied".into(),
        revisions: vec![b"expected".to_vec()],
    };
    assert!(reconcile(&system, &files, "fixture", &vendor, &legacy, Some(&receipt)).is_err());
    assert!(system.services.lock().unwrap().is_empty());
    assert!(files
        .read_regular_file(Path::new("/etc/systemd/system/fixture.service"))
        .unwrap()
        .is_some());
}
