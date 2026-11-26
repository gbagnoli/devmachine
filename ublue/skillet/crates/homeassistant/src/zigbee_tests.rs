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
    let frontend_network = skillet_podman::isolated_network("fixture-zigbee2mqtt");
    let payload = serde_json::to_string(&settings).unwrap();
    apply(&system, &files, &payload, &frontend_network).unwrap();
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
            assert_eq!(
                quadlet.contains("Network=fixture-zigbee2mqtt.network"),
                name == "zigbee2mqtt"
            );
            assert!(!quadlet.contains(&settings.ha_password));
            assert!(!quadlet.contains(&settings.zigbee_password));
        }
        let checks: serde_json::Value = serde_json::from_slice(
            &written["/etc/skillet/datadog/conf.d/http_check.d/skillet-zigbee.yaml"],
        )
        .unwrap();
        assert_eq!(checks["instances"][0]["url"], "http://127.0.0.1:8099/");
        assert_eq!(checks["instances"][1]["url"], "http://192.0.2.6/");
        assert_eq!(checks["instances"][1]["http_response_status_code"], "^401$");
        assert!(written.contains_key(seed));
        written.insert(seed.into(), b"paired coordinator state".to_vec());
    }
    apply(&system, &files, &payload, &frontend_network).unwrap();
    assert_eq!(
        files.files.lock().unwrap()[seed],
        b"paired coordinator state"
    );
}
