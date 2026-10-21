use super::*;
use skillet_core::files::StorageResource;
use skillet_core::test_utils::{MockFiles, MockSystem};
fn config() -> Config {
    Config {
        bluetooth: false,
        backup_subvolume: "/var/lib/data/syncthing",
        files_root: "/var/lib/data/syncthing/data",
        backup_files: "/var/lib/data/syncthing/data/backups/homeassistant",
        owner: "giacomo",
        group: "giacomo",
    }
}
#[test]
fn requires_data_mount() {
    assert!(apply(&MockSystem::new(), &MockFiles::new(), config()).is_err());
}
#[test]
fn host_network_is_pinned_loopback_and_existing_state_is_preserved() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    files
        .ensure_btrfs_subvolume(Path::new("/var/lib/data/syncthing"))
        .unwrap();
    apply(&system, &files, config()).unwrap();
    let data = files.files.lock().unwrap();
    let quadlet = String::from_utf8_lossy(&data["/etc/containers/systemd/homeassistant.container"]);
    assert!(quadlet.contains(IMAGE));
    assert!(quadlet.contains("Network=host"));
    assert!(quadlet.contains("StopTimeout=60"));
    assert!(!quadlet.contains("AutoUpdate="));
    assert!(!quadlet.contains("Privileged=true"));
    let yaml =
        String::from_utf8_lossy(&data["/etc/skillet/homeassistant-initialize/initialize.py"]);
    assert!(yaml.contains("\"server_host\": [\"127.0.0.1\"]"));
    assert!(yaml.contains("async_promote_pending"));
    drop(data);
    let path = "/var/lib/data/homeassistant/config/configuration.yaml";
    files
        .files
        .lock()
        .unwrap()
        .insert(path.into(), b"user settings".to_vec());
    files
        .ensure_btrfs_subvolume(Path::new("/var/lib/data/syncthing"))
        .unwrap();
    apply(&system, &files, config()).unwrap();
    assert_eq!(files.files.lock().unwrap()[path], b"user settings");
}

#[test]
fn export_ancestors_have_consumer_ownership_and_paths_stay_within_declared_root() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    let parent = Path::new("/var/lib/data/syncthing/data/backups");
    files
        .ensure_directory(
            parent,
            Some(0o700),
            &Ownership::named(Some("root"), Some("root")),
        )
        .unwrap();
    backup::install(&system, &files, config()).unwrap();
    let metadata = files.directory_metadata.lock().unwrap();
    assert_eq!(
        metadata[parent.to_str().unwrap()],
        (
            Some(0o700),
            Ownership::named(Some("giacomo"), Some("giacomo"))
        )
    );
    assert!(!metadata.contains_key("/var/lib/data/syncthing"));
    drop(metadata);
    for path in [
        "/outside",
        "/var/lib/data/syncthing/data/../outside",
        "/var/lib/data/syncthing/data",
    ] {
        let mut invalid = config();
        invalid.backup_files = path;
        assert!(invalid.validate().is_err());
    }
}
