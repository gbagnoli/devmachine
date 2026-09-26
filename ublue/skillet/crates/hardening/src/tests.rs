use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};
use std::sync::atomic::Ordering;

#[test]
fn interrupted_service_activation_is_retried() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    system.fail_restart_once.store(true, Ordering::SeqCst);
    assert!(apply(&system, &files).is_err());
    assert!(!files
        .files
        .lock()
        .unwrap()
        .contains_key("/var/lib/skillet/hardening/systemd-sysctl.applied"));
    apply(&system, &files).unwrap();
    let restarts = system.restart_count.load(Ordering::SeqCst);
    apply(&system, &files).unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), restarts);
}

#[test]
fn stopped_sshd_is_started_without_rewriting_config() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    apply(&system, &files).unwrap();
    let restarts = system.restart_count.load(Ordering::SeqCst);
    system.service_stop("sshd").unwrap();
    apply(&system, &files).unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), restarts);
    assert_eq!(system.start_count.load(Ordering::SeqCst), 1);
}

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
    let files_map = files
        .files
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(files_map.contains_key("/etc/ssh/sshd_config"));

    let content =
        String::from_utf8(files_map.get("/etc/ssh/sshd_config").unwrap().clone()).unwrap();
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
    let files_map = files
        .files
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(files_map.contains_key("/etc/ssh/ssh_config"));

    let content = String::from_utf8(files_map.get("/etc/ssh/ssh_config").unwrap().clone()).unwrap();
    assert!(content.contains("StrictHostKeyChecking ask"));
}
