use super::{apply, BtrbkConfig, BtrbkError, CONFIG_PATH, SERVICE_PATH, TIMER_PATH};
use skillet_core::files::FileReadResource;
use skillet_core::test_utils::{MockFiles, MockSystem};
use std::path::PathBuf;

#[test]
fn renders_only_caller_selected_subvolume() {
    let files = MockFiles::new();
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
        let error = apply(
            &MockSystem::new(),
            &MockFiles::new(),
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
    let system = MockSystem::new();
    let config = BtrbkConfig {
        snapshot_subvolumes: vec![PathBuf::from("syncthing")],
    };
    apply(&system, &files, &config).unwrap();
    apply(&system, &files, &config).unwrap();
    assert_eq!(
        system.start_count.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert_eq!(
        system
            .restart_count
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
}
