use super::{apply, BtrbkConfig, BtrbkError, CONFIG_PATH, SERVICE_PATH, TIMER_PATH};
use skillet_core::files::FileReadResource;
use skillet_core::test_utils::{MockFiles, MockSystem};
use std::path::PathBuf;

#[test]
fn renders_only_caller_selected_subvolume() {
    let files = MockFiles::new();
    setup_data_storage(&files, true);
    let system = MockSystem::new();
    apply(
        &system,
        &files,
        &BtrbkConfig {
            snapshot_subvolumes: vec![PathBuf::from("syncthing")],
        },
    )
    .unwrap();
    let config = read(&files, CONFIG_PATH);
    assert!(config.contains("subvolume syncthing"));
    assert!(config.contains("snapshot_dir snapshots/syncthing"));
    assert!(!config.contains("subvolume containers"));
    assert!(!config.contains("subvolume data"));
    assert!(read(&files, SERVICE_PATH).contains("skillet-data-prepare"));
    assert!(read(&files, TIMER_PATH).contains("OnCalendar=hourly"));
}

#[test]
fn rejects_parent_absolute_and_traversal_paths() {
    for invalid in [
        "/var/lib/data",
        "..",
        "nested/../escape",
        ".",
        "with space",
        "bad\nvalue",
    ] {
        let files = MockFiles::new();
        setup_data_storage(&files, false);
        let error = apply(
            &MockSystem::new(),
            &files,
            &BtrbkConfig {
                snapshot_subvolumes: vec![PathBuf::from(invalid)],
            },
        )
        .expect_err("unsafe source should be rejected");
        assert!(matches!(error, BtrbkError::InvalidSubvolume(_)));
    }
}

#[test]
fn empty_config_is_an_opt_out() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    apply(
        &system,
        &files,
        &BtrbkConfig {
            snapshot_subvolumes: vec![],
        },
    )
    .unwrap();
    assert!(files
        .read_file(std::path::Path::new(CONFIG_PATH))
        .unwrap()
        .is_none());
}

#[test]
fn wrong_data_mount_fails_before_any_state_is_written() {
    let files = MockFiles::new();
    setup_data_storage(&files, true);
    files
        .fail_btrfs_mount_check
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let result = apply(
        &MockSystem::new(),
        &files,
        &BtrbkConfig {
            snapshot_subvolumes: vec![PathBuf::from("syncthing")],
        },
    );
    assert!(result.is_err());
    assert!(files
        .read_file(std::path::Path::new(CONFIG_PATH))
        .unwrap()
        .is_none());
}

fn read(files: &MockFiles, path: &str) -> String {
    String::from_utf8(
        files
            .read_file(std::path::Path::new(path))
            .unwrap()
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn repeated_apply_keeps_managed_state_and_timer_running() {
    let files = MockFiles::new();
    setup_data_storage(&files, true);
    let system = MockSystem::new();
    let config = BtrbkConfig {
        snapshot_subvolumes: vec![PathBuf::from("syncthing")],
    };
    apply(&system, &files, &config).unwrap();
    apply(&system, &files, &config).unwrap();
    assert_eq!(
        system
            .restart_count
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

#[test]
fn ordinary_directory_cannot_be_used_as_snapshot_subvolume() {
    let files = MockFiles::new();
    setup_data_storage(&files, false);
    files
        .directories
        .lock()
        .unwrap()
        .insert("/var/lib/data/syncthing".into());
    let result = apply(
        &MockSystem::new(),
        &files,
        &BtrbkConfig {
            snapshot_subvolumes: vec![PathBuf::from("syncthing")],
        },
    );
    assert!(matches!(result, Err(BtrbkError::File(_))));
    assert!(files
        .read_file(std::path::Path::new(CONFIG_PATH))
        .unwrap()
        .is_none());
}

#[test]
fn missing_data_mount_fails_before_state_is_written() {
    let files = MockFiles::new();
    let result = apply(
        &MockSystem::new(),
        &files,
        &BtrbkConfig {
            snapshot_subvolumes: vec![PathBuf::from("syncthing")],
        },
    );
    assert!(result.is_err());
    assert!(files
        .read_file(std::path::Path::new(CONFIG_PATH))
        .unwrap()
        .is_none());
}

#[test]
fn data_mount_on_a_different_device_from_var_is_rejected() {
    let files = MockFiles::new();
    files.record_btrfs_mount(std::path::Path::new("/var"), "/dev/root", "/", "btrfs");
    files.record_btrfs_mount(
        std::path::Path::new("/var/lib/data"),
        "/dev/data",
        "/data",
        "btrfs",
    );
    files.record_btrfs_subvolume(std::path::Path::new("/var/lib/data/syncthing"));
    let result = apply(
        &MockSystem::new(),
        &files,
        &BtrbkConfig {
            snapshot_subvolumes: vec![PathBuf::from("syncthing")],
        },
    );
    assert!(result.is_err());
    assert!(files
        .read_file(std::path::Path::new(CONFIG_PATH))
        .unwrap()
        .is_none());
}

fn setup_data_storage(files: &MockFiles, include_source: bool) {
    files.record_btrfs_mount(std::path::Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(
        std::path::Path::new("/var/lib/data"),
        "/dev/test",
        "/data",
        "btrfs",
    );
    if include_source {
        files.record_btrfs_subvolume(std::path::Path::new("/var/lib/data/syncthing"));
    }
}
