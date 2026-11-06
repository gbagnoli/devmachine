use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};
use std::sync::atomic::Ordering;

fn interface(name: &str, address: &str) -> Interface {
    Interface {
        interface: name.into(),
        address: address.into(),
        prefix: 24,
        gateway: "192.0.2.1".into(),
        dns: ["192.0.2.53".into(), "127.0.0.1".into()],
    }
}
fn fixture() -> Input {
    Input {
        version: 1,
        host: "fixture".into(),
        wired: Some(interface("eth0", "192.0.2.10")),
        wifi: Some(Wireless {
            network: interface("wlan0", "192.0.2.11"),
            ssid: "Test ;\\網\n".into(),
            password: " secret\\word;# ".into(),
        }),
    }
}
const BOTH: Selection = Selection {
    wired: true,
    wifi: true,
};

#[test]
fn render_preserves_dns_policy_and_escapes_wifi_values() {
    let input = fixture();
    input.validate("fixture", BOTH).unwrap();
    let wifi = input.wifi.as_ref().unwrap();
    let rendered = render(&wifi.network, Some(wifi), false).unwrap();
    for directive in [
        "autoconnect=false",
        "route-metric=600",
        "dns=192.0.2.53;127.0.0.1;",
        "ignore-auto-dns=true",
        "cloned-mac-address=permanent",
        "psk=\\ssecret\\\\word;#\\s",
    ] {
        assert!(rendered.contains(directive), "missing {directive}");
    }
    assert_eq!(rendered.matches("[wifi-security]").count(), 1);
    let byte_ssid = wifi.ssid.bytes().fold(String::new(), |mut output, byte| {
        write!(output, "{byte};").unwrap();
        output
    });
    assert!(rendered.contains(&format!("ssid={byte_ssid}\n")));
    let wired = render(input.wired.as_ref().unwrap(), None, true).unwrap();
    assert!(wired.contains("route-metric=100"));
    assert!(!wired.contains("psk="));
}

#[test]
fn fresh_live_preparation_does_not_change_connectivity_and_boot_enables_profiles() {
    let system = MockSystem::new();
    system
        .services
        .lock()
        .unwrap()
        .insert("NetworkManager.service".into(), "started".into());
    let files = MockFiles::new();
    let input = fixture();
    install(&system, &files).unwrap();
    prepare(&system, &files, &input, "fixture", BOTH).unwrap();
    let path = Path::new(CONNECTION_DIR).join("skillet-wifi.nmconnection");
    let contents = files.read_file(&path).unwrap().unwrap();
    assert!(String::from_utf8(contents)
        .unwrap()
        .contains("autoconnect=false"));
    assert_eq!(
        files.metadata.lock().unwrap()[path.to_str().unwrap()],
        (Some(0o600), root())
    );
    let before = files.files.lock().unwrap().clone();
    prepare(&system, &files, &input, "fixture", BOTH).unwrap();
    assert_eq!(*files.files.lock().unwrap(), before);
    system.services.lock().unwrap().clear();
    prepare(&system, &files, &input, "fixture", BOTH).unwrap();
    assert!(String::from_utf8(files.read_file(&path).unwrap().unwrap())
        .unwrap()
        .contains("autoconnect=true"));
    // Subsequent live apply preserves an already boot-prepared profile's reconnect behavior.
    system
        .services
        .lock()
        .unwrap()
        .insert("NetworkManager.service".into(), "started".into());
    prepare(&system, &files, &input, "fixture", BOTH).unwrap();
    assert!(String::from_utf8(files.read_file(&path).unwrap().unwrap())
        .unwrap()
        .contains("autoconnect=true"));
    assert_eq!(system.start_count.load(Ordering::SeqCst), 0);
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 0);
}

#[test]
fn invalid_input_fails_before_mutation_and_never_echoes_private_values() {
    for payload in ["PRIVATE-SECRET", r#"{"version":1,"host":"PRIVATE-SECRET"}"#] {
        let error = Input::parse(payload, "fixture", BOTH).err().unwrap();
        assert!(!error.to_string().contains("PRIVATE-SECRET"));
    }
    let mut input = fixture();
    input.wired.as_mut().unwrap().interface = "bad\n[injected]".into();
    let files = MockFiles::new();
    assert!(prepare(&MockSystem::new(), &files, &input, "fixture", BOTH).is_err());
    assert!(files.files.lock().unwrap().is_empty());
    let mut input = fixture();
    input.wifi.as_mut().unwrap().network.address = "192.0.2.10".into();
    assert!(input.validate("fixture", BOTH).is_err());
    for (address, gateway) in [
        ("192.0.2.0", "192.0.2.1"),
        ("192.0.2.10", "198.51.100.1"),
        ("192.0.2.10", "192.0.2.255"),
    ] {
        let mut input = fixture();
        let wired = input.wired.as_mut().unwrap();
        wired.address = address.into();
        wired.gateway = gateway.into();
        assert!(input.validate("fixture", BOTH).is_err());
    }
    assert!(fixture().validate("other", BOTH).is_err());
    assert!(fixture()
        .validate(
            "fixture",
            Selection {
                wired: true,
                wifi: false
            }
        )
        .is_err());
}

#[test]
fn mask_interface_ssid_and_key_formats_are_bounded() {
    for mask in ["24", "/24", "255.255.255.0"] {
        assert_eq!(prefix_from_mask(mask), Some(24));
    }
    for mask in ["0", "32", "33", "255.0.255.0", "PRIVATE"] {
        assert!(prefix_from_mask(mask).is_none());
    }
    for name in ["lo", "a/b", "a b", "a\n", "very-long-interface"] {
        assert!(!valid_interface(name));
    }
    assert!(valid_interface("wlp0s20f3"));
    assert!(!valid_ssid(&"x".repeat(33)));
    assert!(!valid_ssid("bad\0ssid"));
    assert!(!valid_wifi_key("short"));
    assert!(!valid_wifi_key("password\n"));
    assert!(valid_wifi_key(&"a".repeat(64)));
    assert!(!valid_wifi_key(&"g".repeat(64)));
}

#[test]
fn units_prepare_before_nm_without_network_or_data_dependencies() {
    let unit = include_str!("prepare.service");
    assert!(unit.contains("Before=NetworkManager.service"));
    assert!(unit.contains("DefaultDependencies=no"));
    assert!(unit.contains("--host-file /etc/skillet/host --phase network"));
    assert!(!unit.contains("network-online"));
    assert!(!unit.contains("/var/lib/data"));
    assert!(!unit.contains("RemainAfterExit=yes"));
    assert!(include_str!("network-manager.conf").contains("Wants=skillet-network-prepare.service"));
    let system = MockSystem::new();
    let files = MockFiles::new();
    install(&system, &files).unwrap();
    let before = files.files.lock().unwrap().clone();
    install(&system, &files).unwrap();
    assert_eq!(*files.files.lock().unwrap(), before);
}

#[test]
fn only_managed_runtime_paths_are_mutated_and_payload_omission_is_explicit() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    files
        .ensure_file(
            Path::new("/etc/NetworkManager/system-connections/dhcp.nmconnection"),
            b"operator-owned",
            None,
            &Ownership::default(),
        )
        .unwrap();
    prepare(&system, &files, &fixture(), "fixture", BOTH).unwrap();
    let mut input = fixture();
    input.wifi = None;
    prepare(
        &system,
        &files,
        &input,
        "fixture",
        Selection {
            wired: true,
            wifi: false,
        },
    )
    .unwrap();
    assert!(files
        .read_file(&Path::new(CONNECTION_DIR).join("skillet-wifi.nmconnection"))
        .unwrap()
        .is_none());
    assert_eq!(
        files
            .read_file(Path::new(
                "/etc/NetworkManager/system-connections/dhcp.nmconnection"
            ))
            .unwrap()
            .unwrap(),
        b"operator-owned"
    );
}

#[test]
#[ignore = "requires installed nmcli; offline parser only, never touches live networking"]
fn networkmanager_offline_consumer_accepts_literal_profiles() {
    use std::{
        io::Write as _,
        process::{Command, Stdio},
    };
    let input = fixture();
    for (network, wifi) in [
        (input.wired.as_ref().unwrap(), None),
        (&input.wifi.as_ref().unwrap().network, input.wifi.as_ref()),
    ] {
        let text = render(network, wifi, true).unwrap();
        let mut child = Command::new("nmcli")
            .args([
                "--offline",
                "connection",
                "modify",
                "ipv4.route-metric",
                "101",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(text.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let normalized = String::from_utf8(output.stdout).unwrap();
        assert!(normalized.contains("dns=192.0.2.53;127.0.0.1;"));
        assert!(normalized.contains("ignore-auto-dns=true"));
        assert!(normalized.contains("method=manual"));
        assert!(normalized.contains("address1=192.0.2."));
        assert!(normalized.contains("autoconnect-priority=100"));
        if wifi.is_some() {
            assert!(normalized.contains("key-mgmt=wpa-psk"));
            assert!(normalized.contains("cloned-mac-address=permanent"));
            // Roundtrip stable under the actual consumer, including SSID bytes and escaped PSK.
            let mut roundtrip = Command::new("nmcli")
                .args([
                    "--offline",
                    "connection",
                    "modify",
                    "ipv4.route-metric",
                    "101",
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            roundtrip
                .stdin
                .take()
                .unwrap()
                .write_all(normalized.as_bytes())
                .unwrap();
            assert_eq!(
                roundtrip.wait_with_output().unwrap().stdout,
                normalized.as_bytes()
            );
        }
    }
}
