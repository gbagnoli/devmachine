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
fn host_network_is_loopback_and_existing_state_is_preserved() {
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
    assert!(quadlet.contains("AutoUpdate=registry"));
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

#[test]
fn bluetooth_uses_a_filtered_confined_bus_and_repeat_apply_does_not_restart() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    files
        .ensure_btrfs_subvolume(Path::new("/var/lib/data/syncthing"))
        .unwrap();
    let mut selected = config();
    selected.bluetooth = true;
    apply(&system, &files, selected).unwrap();
    let data = files.files.lock().unwrap();
    let quadlet = String::from_utf8_lossy(&data["/etc/containers/systemd/homeassistant.container"]);
    assert!(quadlet.contains("Volume=/run/skillet-ha-bluetooth:/run/dbus:ro"));
    assert!(!quadlet.contains("Volume=/run/dbus:"));
    assert!(quadlet.contains("Requires=skillet-ha-bluetooth.service"));
    assert!(quadlet.contains("AddCapability=NET_ADMIN NET_RAW"));
    assert!(!quadlet.contains("SecurityLabelDisable"));
    let unit = String::from_utf8_lossy(&data["/etc/systemd/system/skillet-ha-bluetooth.service"]);
    assert!(unit.contains("--filter --talk=org.bluez"));
    assert!(!unit.contains("--talk=*"));
    assert!(unit.contains("SELinuxContext=system_u:system_r:skillet_ha_bluetooth_t:s0"));
    let policy = String::from_utf8_lossy(
        &data["/etc/skillet/homeassistant-bluetooth/skillet-ha-bluetooth.cil"],
    );
    assert!(!policy.contains("(allow container_t system_dbusd_t"));
    assert!(!policy.contains("(typepermissive"));
    let policy_unit =
        String::from_utf8_lossy(&data["/etc/systemd/system/skillet-ha-bluetooth-policy.service"]);
    assert!(policy_unit.contains(
        "ExecStart=/usr/bin/semodule -i /etc/skillet/homeassistant-bluetooth/skillet-ha-bluetooth.cil"
    ));
    drop(data);
    let restarts = system
        .restart_count
        .load(std::sync::atomic::Ordering::SeqCst);
    apply(&system, &files, selected).unwrap();
    assert_eq!(
        restarts,
        system
            .restart_count
            .load(std::sync::atomic::Ordering::SeqCst)
    );
}

#[test]
fn unselected_bluetooth_does_not_install_a_bus_bridge() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    files
        .ensure_btrfs_subvolume(Path::new("/var/lib/data/syncthing"))
        .unwrap();
    apply(&system, &files, config()).unwrap();
    let data = files.files.lock().unwrap();
    assert!(!data.keys().any(|path| path.contains("bluetooth")));
    let quadlet = String::from_utf8_lossy(&data["/etc/containers/systemd/homeassistant.container"]);
    assert!(!quadlet.contains("/run/dbus"));
    assert!(!quadlet.contains("AddCapability=NET_ADMIN"));
}
#[test]
fn image_exporter_retires_legacy_units_and_preserves_backup_storage() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    backup::install(&system, &files, config()).unwrap();
    for unit in backup::image_units() {
        files.files.lock().unwrap().insert(
            format!("/usr/lib/systemd/system/{}", unit.name),
            unit.content.into_bytes(),
        );
    }
    backup::install(&system, &files, config()).unwrap();
    assert!(!files
        .files
        .lock()
        .unwrap()
        .contains_key("/etc/systemd/system/skillet-homeassistant-backup.service"));
    assert!(!system.service_is_active(backup::UNIT).unwrap());
    assert!(system.service_is_active(backup::TIMER).unwrap());
    backup::install(&system, &files, config()).unwrap();
}
