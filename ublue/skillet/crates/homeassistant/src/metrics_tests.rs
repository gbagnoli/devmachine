use super::*;
#[test]
fn missing_or_stale_backup_is_not_reported_as_zero_age() {
    assert_eq!(render(100, false, None).len(), 1);
    let missing = render(100, true, None);
    assert!(missing
        .iter()
        .any(|s| s.starts_with("_sc|homeassistant.backup.fresh|2")));
    assert_eq!(missing.len(), 2);
    let s = State {
        completed: 1,
        exported: 1,
    };
    assert!(render(200_000, true, Some(&s))
        .iter()
        .any(|s| s.starts_with("_sc|homeassistant.backup.fresh|2")));
    assert!(render(100, true, Some(&s))
        .iter()
        .any(|s| s.starts_with("_sc|homeassistant.backup.fresh|0")));
}

#[test]
fn daily_export_remains_fresh_between_scheduled_runs() {
    let state = State {
        completed: 1,
        exported: 1,
    };
    assert!(render(25 * 3600, true, Some(&state))
        .iter()
        .any(|s| s.starts_with("_sc|homeassistant.backup.fresh|0")));
    assert!(render(37 * 3600, true, Some(&state))
        .iter()
        .any(|s| s.starts_with("_sc|homeassistant.backup.fresh|2")));
}

#[test]
fn image_metrics_keep_socket_selection_in_runtime_input() {
    use skillet_core::test_utils::{MockFiles, MockSystem};
    let files = MockFiles::new();
    let system = MockSystem::new();
    let socket = Path::new("/run/test/dsd.socket");
    apply(&system, &files, socket).unwrap();
    for unit in image_units() {
        files.files.lock().unwrap().insert(
            format!("/usr/lib/systemd/system/{}", unit.name),
            unit.content.into_bytes(),
        );
    }
    apply(&system, &files, socket).unwrap();
    apply(&system, &files, Path::new("/run/changed/dsd.socket")).unwrap();
    assert_eq!(
        files.files.lock().unwrap()["/etc/skillet/homeassistant-metrics.env"],
        b"SKILLET_DOGSTATSD_SOCKET=/run/changed/dsd.socket\n"
    );
    assert!(!files
        .files
        .lock()
        .unwrap()
        .contains_key("/etc/systemd/system/skillet-homeassistant-metrics.service"));
}
