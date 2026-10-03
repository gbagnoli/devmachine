use super::*;
use std::{fs, os::unix::fs::symlink};

fn target(dir: &Path) -> SshTarget {
    let identity = dir.join("id_ed25519");
    let known_hosts = dir.join("known_hosts");
    fs::write(&identity, "dummy fixture key").unwrap();
    fs::write(&known_hosts, "dummy fixture host key").unwrap();
    SshTarget {
        user: "fixture".into(),
        address: "127.0.0.1".into(),
        port: 2209,
        identity,
        known_hosts,
    }
}

#[test]
fn remote_executable_arguments_are_quoted_and_transport_is_noninteractive() {
    let dir = tempfile::tempdir().unwrap();
    let transport = SshTransport::new(target(dir.path()), HostKeyPolicy::Verify).unwrap();
    let command = transport
        .command(&GuestCommand {
            program: "sudo",
            arguments: &["-n", "printf", "$(touch nope); 'value'\n"],
        })
        .unwrap();
    let arguments: Vec<_> = command
        .get_args()
        .map(|arg| arg.to_str().unwrap())
        .collect();
    assert!(arguments.contains(&"BatchMode=yes"));
    assert!(arguments.contains(&"StrictHostKeyChecking=yes"));
    assert!(arguments.contains(&"-T"));
    assert!(arguments.contains(&"2209"));
    assert_eq!(arguments[arguments.len() - 2], "fixture@127.0.0.1");
    assert_eq!(
        arguments.last().unwrap(),
        &"'sudo' '-n' 'printf' '$(touch nope); '\\''value'\\''\n'"
    );
}

#[test]
fn enroll_policy_does_not_replace_a_recorded_host_key_and_verify_requires_it() {
    let dir = tempfile::tempdir().unwrap();
    let target = target(dir.path());
    fs::remove_file(&target.known_hosts).unwrap();
    assert!(SshTransport::new(target.clone(), HostKeyPolicy::Verify).is_err());
    let transport = SshTransport::new(target, HostKeyPolicy::Enroll).unwrap();
    let command = transport
        .command(&GuestCommand {
            program: "true",
            arguments: &[],
        })
        .unwrap();
    assert!(command
        .get_args()
        .any(|arg| arg == "StrictHostKeyChecking=accept-new"));
}

#[test]
fn interactive_session_inherits_a_terminal_and_uses_recorded_target() {
    let dir = tempfile::tempdir().unwrap();
    let transport = SshTransport::new(target(dir.path()), HostKeyPolicy::Enroll).unwrap();
    let command = transport.interactive_command();
    let arguments: Vec<_> = command
        .get_args()
        .map(|arg| arg.to_str().unwrap())
        .collect();
    assert!(arguments.contains(&"-tt"));
    assert!(!arguments.contains(&"-T"));
    assert_eq!(arguments.last(), Some(&"fixture@127.0.0.1"));
}

#[test]
fn upload_uses_recorded_port_and_refuses_shell_or_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let transport = SshTransport::new(target(dir.path()), HostKeyPolicy::Verify).unwrap();
    let source = dir.path().join("binary");
    fs::write(&source, "fixture").unwrap();
    let command = transport
        .upload_command(&source, "/var/tmp/skillet-update")
        .unwrap();
    let arguments: Vec<_> = command
        .get_args()
        .map(|arg| arg.to_str().unwrap())
        .collect();
    assert!(arguments.windows(2).any(|pair| pair == ["-P", "2209"]));
    assert_eq!(
        arguments.last().unwrap(),
        &"fixture@127.0.0.1:/var/tmp/skillet-update"
    );
    for destination in [
        "/etc/binary",
        "/var/tmp/../etc/binary",
        "/var/tmp/a;command",
        "/var/tmp/$(command)",
        "/var/tmp/a b",
    ] {
        assert!(transport.upload_command(&source, destination).is_err());
    }
}

#[test]
fn malformed_targets_and_linked_keys_are_refused_before_execution() {
    let dir = tempfile::tempdir().unwrap();
    let original = target(dir.path());
    let mut malformed = original.clone();
    malformed.user = "user;command".into();
    assert!(SshTransport::new(malformed, HostKeyPolicy::Verify).is_err());
    let mut malformed = original.clone();
    malformed.address = "host;command".into();
    assert!(SshTransport::new(malformed, HostKeyPolicy::Verify).is_err());
    let mut linked = original.clone();
    linked.identity = dir.path().join("linked");
    symlink(&original.identity, &linked.identity).unwrap();
    assert!(SshTransport::new(linked, HostKeyPolicy::Verify).is_err());
}

#[test]
fn generic_transport_supports_dns_and_ipv6_targets_without_option_injection() {
    let dir = tempfile::tempdir().unwrap();
    let mut address = target(dir.path());
    address.address = "host.example.invalid".into();
    assert!(SshTransport::new(address.clone(), HostKeyPolicy::Verify).is_ok());
    address.user = "-option".into();
    assert!(SshTransport::new(address, HostKeyPolicy::Verify).is_err());
    let mut address = target(dir.path());
    address.address = "::1".into();
    let transport = SshTransport::new(address, HostKeyPolicy::Verify).unwrap();
    let source = dir.path().join("binary");
    fs::write(&source, "fixture").unwrap();
    let command = transport
        .upload_command(&source, "/var/tmp/skillet")
        .unwrap();
    assert_eq!(
        command.get_args().last().unwrap(),
        "fixture@[::1]:/var/tmp/skillet"
    );
}
