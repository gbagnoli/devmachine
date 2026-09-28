use crate::{apply, PiholeUser};
use skillet_core::{
    system::SystemResource,
    test_utils::{MockFiles, MockSystem},
};
use skillet_podman::{PodmanNetwork, QuadletSecret, SecretTarget};
use std::collections::BTreeMap;

#[test]
fn pihole_uses_dual_stack_dns_network_and_registry_updates() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    system
        .ensure_podman_secret("pihole_web_password", "dummy")
        .unwrap();

    apply(
        &system,
        &files,
        &PiholeUser {
            uid: None,
            gid: None,
            name: "pihole".to_string(),
            group_name: "pihole".to_string(),
        },
        vec![QuadletSecret {
            secret_name: "pihole_web_password".to_string(),
            target: SecretTarget::File {
                target_path: "/run/secrets/pihole_web_password".to_string(),
                mode: Some("0400".to_string()),
                uid: None,
                gid: None,
            },
        }],
        BTreeMap::new(),
        PodmanNetwork {
            unit_name: "clamps".to_string(),
            options: vec![
                "DisableDNS=false".to_string(),
                "Driver=bridge".to_string(),
                "Gateway=172.26.26.1".to_string(),
                "Gateway=fd59:4e23:2950:11f5::1".to_string(),
                "IPv6=true".to_string(),
                "NetworkName=clamps".to_string(),
                "Subnet=172.26.26.0/24".to_string(),
                "Subnet=fd59:4e23:2950:11f5::/64".to_string(),
            ],
        },
    )
    .unwrap();

    let generated = files.files.lock().unwrap();
    let quadlet = String::from_utf8_lossy(
        generated
            .get("/etc/containers/systemd/pihole.container")
            .unwrap(),
    );
    for directive in [
        "AutoUpdate=registry",
        "ContainerName=pihole",
        "Network=clamps.network",
        "Environment=FTLCONF_dns_listeningMode=ALL",
        "Environment=FTLCONF_webserver_port=8088o,[::]:8088o",
        "Environment=TZ=Europe/Madrid",
        "PublishPort=[::]:53:53/tcp",
        "PublishPort=[::]:53:53/udp",
        "PublishPort=0.0.0.0:53:53/tcp",
        "PublishPort=0.0.0.0:53:53/udp",
    ] {
        assert!(
            quadlet.contains(directive),
            "missing Quadlet directive: {directive}"
        );
    }
    assert!(!quadlet.contains("Pod="));
    let network = String::from_utf8_lossy(
        generated
            .get("/etc/containers/systemd/clamps.network")
            .unwrap(),
    );
    assert!(network.contains("DisableDNS=false"));
    assert!(network.contains("IPv6=true"));
}
