use super::{apply, DATA_PATH};
use skillet_core::test_utils::{MockFiles, MockSystem};
use std::sync::atomic::Ordering;

#[test]
fn unifi_uses_host_network_and_persistent_numeric_owned_data() {
    let system = MockSystem::new();
    let files = MockFiles::new();

    apply(&system, &files).unwrap();

    let quadlets = files.files.lock().unwrap();
    let content = String::from_utf8_lossy(
        quadlets
            .get("/etc/containers/systemd/unifi.container")
            .unwrap(),
    );
    assert!(content.contains("Image=docker.io/jacobalberty/unifi:latest"));
    assert!(content.contains("Network=host"));
    assert!(content.contains("User=unifi"));
    assert!(content.contains("Environment=TZ=Europe/Madrid"));
    assert!(content.contains("AutoUpdate=registry"));
    assert!(content.contains("Volume=/var/lib/data/unifi:/unifi:Z"));
    assert!(content.contains("Requires=skillet-data-prepare.service"));
    assert!(content.contains("AssertPathIsMountPoint=/var/lib/data"));
    drop(quadlets);

    assert_eq!(
        files.directory_owner_ids.lock().unwrap().get(DATA_PATH),
        Some(&(999, 999))
    );
}

#[test]
fn unifi_rejects_a_missing_data_mount_before_writing_state() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    files.fail_btrfs_mount_check.store(true, Ordering::SeqCst);

    assert!(apply(&system, &files).is_err());
    assert!(files.directories.lock().unwrap().is_empty());
    assert!(files.files.lock().unwrap().is_empty());
}

#[test]
fn repeated_unifi_apply_does_not_restart_an_unchanged_container() {
    let system = MockSystem::new();
    let files = MockFiles::new();

    apply(&system, &files).unwrap();
    let restart_count = system.restart_count.load(Ordering::SeqCst);
    apply(&system, &files).unwrap();

    assert_eq!(system.restart_count.load(Ordering::SeqCst), restart_count);
    assert_eq!(
        files.directory_owner_ids.lock().unwrap().get(DATA_PATH),
        Some(&(999, 999))
    );
}
