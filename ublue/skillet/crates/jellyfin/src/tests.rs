use super::*;
use skillet_core::{
    system::AccountResource,
    test_utils::{MockFiles, MockSystem},
};
use std::sync::atomic::Ordering;

fn config() -> Config {
    Config {
        state_path: "/var/lib/data/jellyfin/config",
        cache_path: "/var/lib/data/jellyfin/cache",
        files_path: "/var/lib/data/media/files",
        music_path: "/var/lib/data/syncthing/data/music",
        data_owner: "media",
        data_group: "media",
    }
}

#[test]
fn shared_numeric_identity_read_only_libraries_and_repeat_apply() {
    let system = MockSystem::new();
    system.ensure_group("media", Some(3001)).unwrap();
    system.ensure_user("media", Some(2001), Some(3001)).unwrap();
    let files = MockFiles::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    apply(&system, &files, config(), "fixture").unwrap();
    let state = files.files.lock().unwrap();
    let quadlet = String::from_utf8_lossy(&state["/etc/containers/systemd/jellyfin.container"]);
    assert!(quadlet.contains("User=2001:3001"));
    assert!(!quadlet.contains("PublishPort="));
    assert!(!quadlet.contains("/downloads:"));
    for (source, destination) in config().libraries() {
        assert!(quadlet.contains(&format!("Volume={}:{}:ro,z", source.display(), destination)));
    }
    drop(state);
    let restarts = system.restart_count.load(Ordering::SeqCst);
    apply(&system, &files, config(), "fixture").unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), restarts);
}

#[test]
fn incompatible_storage_and_missing_accounts_fail_before_container_mutation() {
    let mut value = config();
    value.state_path = value.files_path;
    assert!(value.validate().is_err());
    value = config();
    value.cache_path = "/etc/cache";
    assert!(value.validate().is_err());
    assert!(apply(&MockSystem::new(), &MockFiles::new(), config(), "fixture").is_err());
}
