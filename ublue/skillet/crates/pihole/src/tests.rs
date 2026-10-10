use crate::{apply, PiholeUser};
use skillet_core::{
    system::PodmanSecretResource,
    test_utils::{MockFiles, MockSystem},
};
use skillet_podman::{QuadletSecret, SecretTarget};
use std::collections::BTreeMap;

#[test]
fn pihole_uses_dual_stack_dns_network_and_registry_updates() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    files.record_btrfs_mount(std::path::Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(
        std::path::Path::new("/var/lib/data"),
        "/dev/test",
        "/data",
        "btrfs",
    );
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
        "clamps".to_string(),
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
}
