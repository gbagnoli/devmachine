use super::*;
use skillet_core::{
    files::{FileMutationResource, Ownership},
    recorder::Recorder,
    resource_op::ResourceOp,
};
use std::{path::Path, sync::atomic::Ordering};

const BINARY: &str = "/usr/bin/skillet-clamps";
const STATE: &str = "/var/lib/skillet/btrbk/timer.applied";

fn config() -> BtrbkConfig {
    BtrbkConfig {
        snapshot_subvolumes: vec!["syncthing".into()],
    }
}

fn vendor(files: &MockFiles) {
    for artifact in image::render(&config(), BINARY).unwrap() {
        files
            .ensure_file(
                Path::new(artifact.path),
                artifact.content.as_bytes(),
                Some(0o644),
                &Ownership::default(),
            )
            .unwrap();
    }
}

fn selected(
    system: &impl skillet_core::system::ServiceResource,
    files: &MockFiles,
) -> Result<(), BtrbkError> {
    apply_for_host(system, files, &config(), BINARY)
}

#[test]
fn fresh_image_and_repeat_have_one_vendor_owner() {
    let files = MockFiles::new();
    setup_data_storage(&files, true);
    vendor(&files);
    let system = MockSystem::new();
    selected(&system, &files).unwrap();
    let receipt = read(&files, STATE);
    selected(&system, &files).unwrap();
    assert_eq!(read(&files, STATE), receipt);
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 1);
    for path in [CONFIG_PATH, SERVICE_PATH, TIMER_PATH] {
        assert!(files.read_file(Path::new(path)).unwrap().is_none());
    }
    let service = read(&files, image::SERVICE);
    assert!(service.contains("ExecStartPre=/usr/bin/skillet-clamps btrbk prepare"));
    assert!(service.contains(image::CONFIG));
    assert!(!service.contains("/var/usrlocal"));
}

#[test]
fn legacy_retirement_rollback_and_remigration_preserve_recovery() {
    let files = MockFiles::new();
    setup_data_storage(&files, true);
    let system = MockSystem::new();
    apply(&system, &files, &config()).unwrap();
    let legacy_receipt = read(&files, STATE);
    vendor(&files);
    selected(&system, &files).unwrap();
    for path in [CONFIG_PATH, SERVICE_PATH, TIMER_PATH] {
        assert!(files.read_file(Path::new(path)).unwrap().is_none());
        assert!(files
            .read_file(Path::new(&format!("{path}.skillet-legacy")))
            .unwrap()
            .is_some());
    }
    assert_ne!(read(&files, STATE), legacy_receipt);
    // Previous image is booted, using the retained migration-capable caller.
    for artifact in image::render(&config(), BINARY).unwrap() {
        files.delete_file(Path::new(artifact.path)).unwrap();
    }
    selected(&system, &files).unwrap();
    assert_eq!(read(&files, STATE), legacy_receipt);
    assert!(read(&files, SERVICE_PATH).contains("skillet-data-prepare"));
    vendor(&files);
    selected(&system, &files).unwrap();
    assert!(files.read_file(Path::new(SERVICE_PATH)).unwrap().is_none());
}

#[test]
fn custom_edits_and_wrong_receipts_refuse_before_any_retirement() {
    for custom_path in [CONFIG_PATH, TIMER_PATH, STATE] {
        let files = MockFiles::new();
        setup_data_storage(&files, true);
        let system = MockSystem::new();
        apply(&system, &files, &config()).unwrap();
        vendor(&files);
        files
            .ensure_file(
                Path::new(custom_path),
                b"admin edit",
                None,
                &Ownership::default(),
            )
            .unwrap();
        assert!(selected(&system, &files).is_err());
        for path in [CONFIG_PATH, SERVICE_PATH, TIMER_PATH] {
            assert!(files.read_file(Path::new(path)).unwrap().is_some());
        }
        assert_eq!(read(&files, custom_path), "admin edit");
    }
}

#[test]
fn interrupted_partial_retirement_and_failed_activation_retry() {
    let files = MockFiles::new();
    setup_data_storage(&files, true);
    let system = MockSystem::new();
    apply(&system, &files, &config()).unwrap();
    let receipt = read(&files, STATE);
    vendor(&files);
    // Simulate a killed migration after its first atomic retirement.
    let config_bytes = files.read_file(Path::new(CONFIG_PATH)).unwrap().unwrap();
    files
        .retire_file(
            Path::new(CONFIG_PATH),
            &config_bytes,
            Path::new(&format!("{CONFIG_PATH}.skillet-legacy")),
        )
        .unwrap();
    system.fail_restart_once.store(true, Ordering::SeqCst);
    assert!(selected(&system, &files).is_err());
    assert_eq!(read(&files, STATE), receipt);
    selected(&system, &files).unwrap();
    assert_ne!(read(&files, STATE), receipt);
    selected(&system, &files).unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 3);
}

#[test]
fn running_snapshot_or_partial_vendor_tree_is_not_migrated() {
    let files = MockFiles::new();
    setup_data_storage(&files, true);
    let system = MockSystem::new();
    apply(&system, &files, &config()).unwrap();
    vendor(&files);
    system
        .services
        .lock()
        .unwrap()
        .insert("skillet-btrbk.service".into(), "started".into());
    assert!(selected(&system, &files).is_err());
    assert!(files.read_file(Path::new(CONFIG_PATH)).unwrap().is_some());
    files.delete_file(Path::new(image::MARKER)).unwrap();
    assert!(selected(&system, &files).is_err());
}

#[test]
fn rollback_reconciliation_preserves_new_admin_edits() {
    let files = MockFiles::new();
    setup_data_storage(&files, true);
    let system = MockSystem::new();
    apply(&system, &files, &config()).unwrap();
    vendor(&files);
    selected(&system, &files).unwrap();
    for artifact in image::render(&config(), BINARY).unwrap() {
        files.delete_file(Path::new(artifact.path)).unwrap();
    }
    files
        .ensure_file(
            Path::new(TIMER_PATH),
            b"admin timer",
            None,
            &Ownership::default(),
        )
        .unwrap();
    assert!(selected(&system, &files).is_err());
    assert_eq!(read(&files, TIMER_PATH), "admin timer");
    assert!(files.read_file(Path::new(CONFIG_PATH)).unwrap().is_none());
}

#[test]
fn migration_records_retirement_and_reenable_without_payloads() {
    let files = MockFiles::new();
    setup_data_storage(&files, true);
    let system = MockSystem::new();
    apply(&system, &files, &config()).unwrap();
    vendor(&files);
    let recorded_files = Recorder::new(files);
    let recorded_system = Recorder::new(system);
    apply_for_host(&recorded_system, &recorded_files, &config(), BINARY).unwrap();
    assert_eq!(
        recorded_files
            .get_ops()
            .iter()
            .filter(|record| matches!(record.operation, ResourceOp::RetireFile { .. }))
            .count(),
        3
    );
    assert!(recorded_system
        .get_ops()
        .iter()
        .any(|record| matches!(record.operation, ResourceOp::ServiceReenable { .. })));
}
