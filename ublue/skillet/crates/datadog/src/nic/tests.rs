use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};
use std::sync::atomic::Ordering;

fn nic(root: &Path, name: &str, hardware: bool, up: &str, speed: &str) -> std::path::PathBuf {
    let path = root.join(name);
    fs::create_dir(&path).unwrap();
    if hardware {
        fs::create_dir(path.join("device")).unwrap();
    }
    for (file, content) in [("type", "1"), ("carrier", up), ("speed", speed)] {
        fs::write(path.join(file), content).unwrap();
    }
    path
}

#[test]
fn discovers_physical_ethernet_and_excludes_bridges_loopback_and_wifi() {
    let root = tempfile::tempdir().unwrap();
    nic(root.path(), "enp1s0", true, "1", "1000\n");
    nic(root.path(), "eth1", true, "0", "-1");
    nic(root.path(), "podman0", false, "1", "10000");
    nic(root.path(), "lo", false, "1", "-1");
    let wifi = nic(root.path(), "wlo1", true, "1", "-1");
    fs::create_dir(wifi.join("wireless")).unwrap();
    let samples = collect(root.path()).unwrap();
    assert_eq!(samples.len(), 2);
    assert_eq!(samples[0].interface, "enp1s0");
    assert_eq!(samples[0].speed_mbps, Some(1000));
    assert_eq!(samples[1].speed_mbps, Some(0));
    let packets = render(&samples);
    assert!(packets.contains(&"skillet.net.link_speed_mbps:1000|g|#interface:enp1s0".into()));
    assert!(packets.contains(&"skillet.net.link_up:0|g|#interface:eth1".into()));
}

#[test]
fn failed_speed_readings_are_not_reported_as_zero_and_recover() {
    let root = tempfile::tempdir().unwrap();
    let path = nic(root.path(), "enp1s0", true, "1", "-1");
    for invalid in ["-1", "0", "unknown", "99999999999999999999"] {
        fs::write(path.join("speed"), invalid).unwrap();
        let packets = render(&collect(root.path()).unwrap());
        assert!(packets.contains(&"skillet.net.collection_success:0|g|#interface:enp1s0".into()));
        assert!(!packets
            .iter()
            .any(|packet| packet.starts_with("skillet.net.link_speed_mbps:")));
    }
    fs::remove_file(path.join("speed")).unwrap();
    assert!(collect(root.path()).unwrap()[0].speed_mbps.is_none());
    fs::write(path.join("speed"), "2500").unwrap();
    assert_eq!(collect(root.path()).unwrap()[0].speed_mbps, Some(2500));
    fs::remove_file(path.join("carrier")).unwrap();
    let packets = render(&collect(root.path()).unwrap());
    assert_eq!(
        packets,
        ["skillet.net.collection_success:0|g|#interface:enp1s0"]
    );
}

#[test]
fn injected_tag_values_are_rejected_and_missing_discovery_is_an_error() {
    assert_eq!(
        render(&[Sample {
            interface: "eth0,host:other".into(),
            link_up: Some(true),
            speed_mbps: Some(1000)
        }]),
        Vec::<String>::new()
    );
    let root = tempfile::tempdir().unwrap();
    assert!(collect(&root.path().join("absent")).is_err());
}

#[test]
fn timer_is_idempotent_and_image_migration_retires_only_known_overrides() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    install(&system, &files).unwrap();
    let starts = system.start_count.load(Ordering::SeqCst);
    install(&system, &files).unwrap();
    assert_eq!(system.start_count.load(Ordering::SeqCst), starts);
    for unit in image_units() {
        files.files.lock().unwrap().insert(
            format!("/usr/lib/systemd/system/{}", unit.name),
            unit.content.into_bytes(),
        );
    }
    install(&system, &files).unwrap();
    assert!(!files
        .files
        .lock()
        .unwrap()
        .contains_key(&format!("/etc/systemd/system/{UNIT}")));
}
