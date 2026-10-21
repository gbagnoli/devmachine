use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};
use std::sync::atomic::Ordering;

fn payload(token: &str) -> String {
    config::PrivateConfig::parse(r#"{"version":1,"records":[{"name":"edge","proxied":false}]}"#)
        .unwrap()
        .payload("0123456789abcdef0123456789abcdef", token)
        .unwrap()
        .render()
        .unwrap()
}

fn mounted_files() -> MockFiles {
    let files = MockFiles::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    files
}

#[test]
fn mounts_secret_as_root_without_publishing_ports_or_plaintext() {
    let system = MockSystem::new();
    let files = mounted_files();
    apply(&system, &files, &payload("dummy-token"), "test-net").unwrap();
    let state = files.files.lock().unwrap();
    let quadlet =
        String::from_utf8_lossy(&state["/etc/containers/systemd/cloudflare-ddns.container"]);
    assert!(quadlet.contains(IMAGE));
    assert!(quadlet.contains("Network=test-net.network"));
    assert!(quadlet.contains("User=0:0"));
    assert!(
        quadlet.contains("Secret=cloudflare_ddns_config,target=/config.json,mode=0400,uid=0,gid=0")
    );
    assert!(quadlet.contains("Restart=always"));
    assert!(quadlet.contains("WantedBy=multi-user.target"));
    assert!(quadlet.contains("AssertPathIsMountPoint=/var/lib/data"));
    assert!(!quadlet.contains("PublishPort="));
    assert!(!quadlet.contains("AutoUpdate="));
    assert!(state
        .values()
        .all(|bytes| !String::from_utf8_lossy(bytes).contains("dummy-token")));
}

#[test]
fn no_op_rotation_and_activation_retry() {
    let system = MockSystem::new();
    let files = mounted_files();
    apply(&system, &files, &payload("dummy-one"), "test-net").unwrap();
    let restarts = system.restart_count.load(Ordering::SeqCst);
    apply(&system, &files, &payload("dummy-one"), "test-net").unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), restarts);
    system.fail_restart_once.store(true, Ordering::SeqCst);
    assert!(apply(&system, &files, &payload("dummy-two"), "test-net").is_err());
    apply(&system, &files, &payload("dummy-two"), "test-net").unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), restarts + 2);
}

#[test]
fn rejects_missing_storage_and_bad_input_without_mutation() {
    for input in [payload("dummy-one"), "invalid".into()] {
        let system = MockSystem::new();
        let files = MockFiles::new();
        assert!(apply(&system, &files, &input, "test-net").is_err());
        assert!(system.podman_secrets.lock().unwrap().is_empty());
        assert!(files.files.lock().unwrap().is_empty());
    }
}
