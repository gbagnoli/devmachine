use super::*;
use std::os::unix::net::UnixListener;

fn identity(user: &str, tags: &[&str]) -> Identity {
    Identity {
        node: Node {
            user: 1,
            tags: tags.iter().map(|tag| (*tag).into()).collect(),
        },
        user_profile: UserProfile {
            id: 1,
            login_name: user.into(),
        },
    }
}

#[test]
fn full_identities_not_prefixes_are_authorized_and_tags_are_denied() {
    let policy = Policy::new(vec![
        "alice@example.invalid".into(),
        "irene@example.invalid".into(),
    ])
    .unwrap();
    assert!(policy.authorize(&identity("alice@example.invalid", &[])));
    assert!(policy.authorize(&identity("irene@example.invalid", &[])));
    for denied in ["alice@other.invalid", "alice", "unknown@example.invalid"] {
        assert!(!policy.authorize(&identity(denied, &[])));
    }
    assert!(!policy.authorize(&identity("alice@example.invalid", &["tag:server"])));
    let mut mismatch = identity("alice@example.invalid", &[]);
    mismatch.node.user = 2;
    assert!(!policy.authorize(&mismatch));
    assert!(!Policy::new(Vec::new())
        .unwrap()
        .authorize(&identity("alice@example.invalid", &[])));
}

#[test]
fn malformed_policy_and_empty_required_entry_are_rejected_without_values() {
    for invalid in ["", "\n", "alice smith", "alice\nalice", "bad\0identity"] {
        let error = Policy::parse_users(invalid).unwrap_err();
        assert!(!error.to_string().contains(invalid) || invalid.is_empty());
    }
    // Headscale's complete LoginName can differ from a hosted email identity.
    assert_eq!(
        Policy::parse_users("person\nother-person").unwrap(),
        ["person", "other-person"]
    );
}

#[test]
fn real_unix_localapi_checks_exact_peer_and_daemon_failure_denies() {
    let tmp = tempfile::tempdir().unwrap();
    let socket = tmp.path().join("tailscaled.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let server = tiny_http::Server::from_listener(listener, None).unwrap();
    let thread = std::thread::spawn(move || {
        let request = server
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert_eq!(
            request.url(),
            "/localapi/v0/whois?addr=100.100.100.1%3A1234"
        );
        let body =
            r#"{"Node":{"User":1},"UserProfile":{"ID":1,"LoginName":"alice@example.invalid"}}"#;
        request
            .respond(tiny_http::Response::from_string(body))
            .unwrap();
    });
    let api = LocalApi::new(&socket).unwrap();
    let policy = Policy::new(vec!["alice@example.invalid".into()]).unwrap();
    assert!(authorize_peer(&policy, "127.0.0.1:1234", &api).is_none());
    assert!(authorize_peer(&policy, "forged", &api).is_none());
    assert_eq!(
        authorize_peer(&policy, "100.100.100.1:1234", &api).as_deref(),
        Some("alice@example.invalid")
    );
    thread.join().unwrap();
    assert!(authorize_peer(&policy, "100.100.100.1:1234", &api).is_none());
}

#[test]
fn ipv4_and_ipv6_ranges_are_exact() {
    for addr in [
        "100.64.0.1:10",
        "100.127.255.254:10",
        "[fd7a:115c:a1e0::1]:10",
    ] {
        assert!(tailnet_peer(addr.parse().unwrap()));
    }
    for addr in [
        "100.63.0.1:10",
        "100.128.0.1:10",
        "[fd7a:115c:a1e1::1]:10",
        "[::1]:10",
    ] {
        assert!(!tailnet_peer(addr.parse().unwrap()));
    }
}

#[test]
fn users_accept_commas_newlines_and_whitespace_but_never_duplicates() {
    assert_eq!(
        Policy::parse_users(
            " alice@example.invalid, irene@example.invalid\nother@example.invalid "
        )
        .unwrap(),
        [
            "alice@example.invalid",
            "irene@example.invalid",
            "other@example.invalid"
        ]
    );
    assert!(Policy::parse_users("alice, alice").is_err());
}

#[test]
fn authorization_endpoint_ignores_forged_headers_and_reloads_revocations() {
    let temp = tempfile::tempdir().unwrap();
    let daemon = temp.path().join("daemon.sock");
    let auth = temp.path().join("auth.sock");
    let policy = temp.path().join("policy.json");
    std::fs::write(
        &policy,
        serde_json::to_vec(&Policy::new(vec!["alice@example.invalid".into()]).unwrap()).unwrap(),
    )
    .unwrap();
    let api_server = tiny_http::Server::http_unix(&daemon).unwrap();
    let api_thread = std::thread::spawn(move || {
        for _ in 0..2 {
            let request = api_server
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap();
            request.respond(tiny_http::Response::from_string(r#"{"Node":{"User":1},"UserProfile":{"ID":1,"LoginName":"alice@example.invalid"}}"#)).unwrap();
        }
    });
    let server = tiny_http::Server::http_unix(&auth).unwrap();
    let api = LocalApi::new(&daemon).unwrap();
    let policy_path = policy.clone();
    let auth_thread = std::thread::spawn(move || {
        for _ in 0..5 {
            let request = server
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap();
            super::respond(request, &api, &policy_path);
        }
    });
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .unix_socket(auth.as_path())
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let url = "http://auth.local/auth";
    let allowed = client
        .get(url)
        .header("X-Skillet-Peer", "100.100.100.1:1234")
        .header("X-Skillet-User", "attacker")
        .send()
        .unwrap();
    assert_eq!(allowed.status(), 204);
    assert_eq!(allowed.headers()["X-Skillet-User"], "alice@example.invalid");
    assert_eq!(
        client
            .get(url)
            .header("X-Skillet-User", "alice@example.invalid")
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        client
            .get(url)
            .header("X-Skillet-Peer", "127.0.0.1:1234")
            .header("X-Forwarded-For", "100.100.100.1")
            .send()
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        client
            .get(url)
            .header("X-Skillet-Peer", "100.100.100.1:1234")
            .header("X-Skillet-Peer", "100.100.100.2:1234")
            .send()
            .unwrap()
            .status(),
        403
    );
    std::fs::write(&policy, br#"{"users":[]}"#).unwrap();
    assert_eq!(
        client
            .get(url)
            .header("X-Skillet-Peer", "100.100.100.1:1234")
            .send()
            .unwrap()
            .status(),
        403
    );
    auth_thread.join().unwrap();
    api_thread.join().unwrap();
}

#[test]
fn socket_policy_precedes_broker_and_repeat_apply_does_not_reload_it() {
    use skillet_core::test_utils::{MockFiles, MockSystem};
    use std::sync::atomic::Ordering;
    let system = MockSystem::new();
    let files = MockFiles::new();
    let policy = Policy::new(vec!["alice@example.invalid".into()]).unwrap();
    apply(&system, &files, &policy).unwrap();
    let managed = files.files.lock().unwrap();
    let unit = String::from_utf8_lossy(&managed["/etc/systemd/system/skillet-tailnet-ui.service"]);
    assert!(unit.contains("Requires=skillet-tailnet-ui-policy.service"));
    assert!(unit.contains("After=skillet-tailnet-ui-policy.service tailscale.service"));
    assert!(unit.contains("SELinuxContext=system_u:system_r:skillet_tailnet_ui_t:s0"));
    assert!(unit.contains("RuntimeDirectoryMode=0700"));
    assert!(unit.contains("NoNewPrivileges=yes"));
    assert!(unit.contains("RestrictAddressFamilies=AF_UNIX"));
    assert_eq!(
        managed["/etc/skillet/selinux/skillet-tailnet-ui.cil"],
        SOCKET_POLICY.as_bytes()
    );
    assert!(SOCKET_POLICY_UNIT.contains("ConditionSecurity=selinux"));
    assert!(SOCKET_POLICY
        .contains("(allow container_t skillet_tailnet_ui_t (unix_stream_socket (connectto)))"));
    assert!(!SOCKET_POLICY.contains("(allow container_t unconfined_service_t"));
    assert!(!SOCKET_POLICY.contains("typeattributeset unconfined"));
    drop(managed);
    let starts = system.start_count.load(Ordering::SeqCst);
    let restarts = system.restart_count.load(Ordering::SeqCst);
    apply(&system, &files, &policy).unwrap();
    assert_eq!(system.start_count.load(Ordering::SeqCst), starts);
    assert_eq!(system.restart_count.load(Ordering::SeqCst), restarts);
}

#[test]
fn failed_socket_policy_load_blocks_broker_and_retries_without_applied_marker() {
    use skillet_core::{
        system::ServiceResource,
        test_utils::{MockFiles, MockSystem},
    };
    use std::sync::atomic::Ordering;
    let system = MockSystem::new();
    let files = MockFiles::new();
    let policy = Policy::new(vec!["alice@example.invalid".into()]).unwrap();
    system
        .service_start("skillet-tailnet-ui-policy.service")
        .unwrap();
    system.fail_restart_once.store(true, Ordering::SeqCst);
    assert!(apply(&system, &files, &policy).is_err());
    {
        let managed = files.files.lock().unwrap();
        assert!(!managed.contains_key("/var/lib/skillet/tailnet-ui/policy.applied"));
        assert!(!managed.contains_key("/etc/systemd/system/skillet-tailnet-ui.service"));
        assert!(!managed.contains_key(POLICY_FILE));
    }
    apply(&system, &files, &policy).unwrap();
    let managed = files.files.lock().unwrap();
    assert!(managed.contains_key("/var/lib/skillet/tailnet-ui/policy.applied"));
    assert!(managed.contains_key("/etc/systemd/system/skillet-tailnet-ui.service"));
}
