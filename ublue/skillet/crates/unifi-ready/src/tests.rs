use super::*;

#[test]
fn accepts_only_explicit_json_controller_readiness() {
    let body = br#"{"meta":{"rc":"ok","up":true}}"#;
    assert!(ready(
        StatusCode::OK,
        "application/json;charset=UTF-8",
        body
    ));
    for (code, kind, bytes) in [
        (StatusCode::OK, "text/html", body.as_slice()),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "application/json",
            body.as_slice(),
        ),
        (
            StatusCode::OK,
            "application/json",
            b"<html>starting</html>".as_slice(),
        ),
        (
            StatusCode::OK,
            "application/json",
            br#"{"meta":{"rc":"ok","up":false}}"#.as_slice(),
        ),
        (
            StatusCode::OK,
            "application/json",
            br#"{"meta":{"up":true}}"#.as_slice(),
        ),
    ] {
        assert!(!ready(code, kind, bytes));
    }
    assert!(!ready(
        StatusCode::OK,
        "application/json",
        &vec![b' '; 16385]
    ));
}

#[test]
fn local_tls_exception_cannot_be_redirected_to_remote_hosts() {
    for endpoint in [
        "https://127.0.0.1:8443/status",
        "http://127.0.0.1:12345/status",
        "https://[::1]:8443/status",
    ] {
        assert!(validate_endpoint(endpoint).is_ok());
    }
    for endpoint in [
        "https://example.invalid/status",
        "https://localhost/status",
        "https://192.0.2.1/status",
        "https://user:password@127.0.0.1/status",
        "https://127.0.0.1/api/login",
        "https://127.0.0.1/status?key=secret",
        "ftp://127.0.0.1/status",
    ] {
        assert!(validate_endpoint(endpoint).is_err());
    }
}

#[test]
fn retries_transient_failures_and_obeys_deadline() {
    let mut calls = 0;
    wait_with(Duration::from_secs(1), Duration::ZERO, |_| {
        calls += 1;
        calls >= 3
    })
    .unwrap();
    assert_eq!(calls, 3);
    let start = Instant::now();
    assert!(
        wait_with(Duration::from_millis(20), Duration::from_millis(5), |_| {
            false
        })
        .is_err()
    );
    assert!(start.elapsed() < Duration::from_secs(1));
}

#[test]
fn real_http_transition_from_startup_html_to_ready_json() {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/status", server.server_addr());
    let worker = std::thread::spawn(move || {
        for (kind, body) in [
            ("text/html", "<html>Starting</html>"),
            ("application/json", r#"{"meta":{"rc":"ok","up":true}}"#),
        ] {
            let request = server
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap();
            assert_eq!(request.url(), "/status");
            assert_eq!(request.method(), &tiny_http::Method::Get);
            request
                .respond(
                    tiny_http::Response::from_string(body)
                        .with_header(tiny_http::Header::from_bytes("Content-Type", kind).unwrap()),
                )
                .unwrap();
        }
    });
    wait(&endpoint, Duration::from_secs(5)).unwrap();
    worker.join().unwrap();
}

#[test]
#[ignore = "requires the locally built scratch readiness image and a container engine"]
fn scratch_container_handles_startup_and_timeout_without_credentials() {
    let engine = std::env::var("SKILLET_READY_TEST_ENGINE").unwrap_or_else(|_| "podman".into());
    assert!(matches!(engine.as_str(), "podman" | "docker"));
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/status", server.server_addr());
    let worker = std::thread::spawn(move || {
        for (kind, body) in [
            ("text/html", "<html>Starting</html>"),
            ("application/json", r#"{"meta":{"rc":"ok","up":true}}"#),
        ] {
            let request = server
                .recv_timeout(Duration::from_secs(30))
                .unwrap()
                .unwrap();
            request
                .respond(
                    tiny_http::Response::from_string(body)
                        .with_header(tiny_http::Header::from_bytes("Content-Type", kind).unwrap()),
                )
                .unwrap();
        }
    });
    let output = std::process::Command::new(&engine)
        .args([
            "run",
            "--rm",
            "--network=host",
            "--read-only",
            "--cap-drop=all",
            "--security-opt=no-new-privileges",
            "localhost/skillet-unifi-ready:check",
            "--url",
            &endpoint,
            "--timeout-secs",
            "10",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    worker.join().unwrap();
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let unavailable = format!("http://{}/status", server.server_addr());
    // No request is accepted: the client must time out even when TCP connects.
    let output = std::process::Command::new(engine)
        .args([
            "run",
            "--rm",
            "--network=host",
            "--read-only",
            "--cap-drop=all",
            "--security-opt=no-new-privileges",
            "localhost/skillet-unifi-ready:check",
            "--url",
            &unavailable,
            "--timeout-secs",
            "1",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("NotReady"));
}
