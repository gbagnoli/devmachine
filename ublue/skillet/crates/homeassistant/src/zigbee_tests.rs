use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};

#[test]
fn only_loopback_is_published_and_pairing_state_survives_reapply() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    let settings = Settings {
        coordinator: "192.0.2.6:6638".parse().unwrap(),
        ha_password: "h".repeat(64),
        zigbee_password: "z".repeat(64),
    };
    let payload = serde_json::to_string(&settings).unwrap();
    apply(&system, &files, &payload).unwrap();
    let seed = "/var/lib/data/zigbee2mqtt/configuration.yaml";
    {
        let mut written = files.files.lock().unwrap();
        for (name, binding) in [
            ("mosquitto", "127.0.0.1:1883:1883"),
            ("zigbee2mqtt", "127.0.0.1:8099:8080"),
        ] {
            let quadlet = String::from_utf8_lossy(
                &written[&format!("/etc/containers/systemd/{name}.container")],
            );
            assert!(quadlet.contains(binding));
            assert!(!quadlet.contains(&settings.ha_password));
            assert!(!quadlet.contains(&settings.zigbee_password));
        }
        assert!(written.contains_key(seed));
        written.insert(seed.into(), b"paired coordinator state".to_vec());
    }
    apply(&system, &files, &payload).unwrap();
    assert_eq!(
        files.files.lock().unwrap()[seed],
        b"paired coordinator state"
    );
}
