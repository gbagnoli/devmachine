use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};

#[test]
fn test_hardening_applies_sysctl() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    apply(&system, &files).unwrap();
    assert!(files
        .files
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains_key("/etc/sysctl.d/99-hardening.conf"));
    assert_eq!(
        system
            .services
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get("systemd-sysctl")
            .unwrap(),
        "restarted"
    );
}

#[test]
fn test_hardening_applies_ssh_server() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    apply(&system, &files).unwrap();
    let files_map = files.files.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(files_map.contains_key("/etc/ssh/sshd_config"));

    let content = String::from_utf8(files_map.get("/etc/ssh/sshd_config").unwrap().clone()).unwrap();
    assert!(content.contains("PermitRootLogin without-password"));
    assert!(content.contains("Ciphers chacha20-poly1305@openssh.com"));

    assert_eq!(
        system
            .services
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get("sshd")
            .unwrap(),
        "restarted"
    );
}

#[test]
fn test_hardening_applies_ssh_client() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    apply(&system, &files).unwrap();
    let files_map = files.files.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(files_map.contains_key("/etc/ssh/ssh_config"));

    let content = String::from_utf8(files_map.get("/etc/ssh/ssh_config").unwrap().clone()).unwrap();
    assert!(content.contains("StrictHostKeyChecking ask"));
}

#[test]
fn test_hardening_applies_pihole() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    apply(&system, &files).unwrap();
    let files_map = files.files.lock().unwrap_or_else(std::sync::PoisonError::into_inner);

    // Verify directories
    let dirs = files
        .directories
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(dirs.contains("/etc/pihole"));
    assert!(dirs.contains("/etc/pihole/conf"));
    assert!(dirs.contains("/etc/pihole/dnsmasq.d"));
    assert!(dirs.contains("/var/log/pihole"));

    // Verify custom.list
    assert!(files_map.contains_key("/etc/pihole/conf/custom.list"));
    let custom_list =
        String::from_utf8(files_map.get("/etc/pihole/conf/custom.list").unwrap().clone()).unwrap();
    assert!(custom_list.contains("192.168.1.100 my.custom.domain"));

    // Verify Quadlet
    assert!(files_map.contains_key("/etc/containers/systemd/pihole.container"));
    let quadlet = String::from_utf8(
        files_map
            .get("/etc/containers/systemd/pihole.container")
            .unwrap()
            .clone(),
    )
    .unwrap();
    assert!(quadlet.contains("Image=docker.io/pihole/pihole:latest"));
    assert!(quadlet.contains("Volume=/etc/pihole/conf:/etc/pihole"));
}
