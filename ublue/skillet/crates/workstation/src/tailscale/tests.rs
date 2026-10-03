use super::{
    create_auth_key, device_record, find_device_by_hostname, form_encode,
    remove_device_for_hostname, OAuthCredentials, SMOKE_TAG,
};
use serde_json::json;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

#[test]
fn form_encoding_escapes_oauth_credentials() {
    assert_eq!(form_encode("a b+c/%"), "a%20b%2Bc%2F%25");
}

#[test]
fn device_record_requires_expected_tag_and_identity_fields() {
    let value = json!({
        "id": "node-id",
        "hostname": "clamps-test-smoke",
        "addresses": ["100.64.0.5", "fd7a:115c:a1e0::5"],
        "tags": [SMOKE_TAG]
    });
    let record = device_record(&value, SMOKE_TAG).expect("valid tagged device");
    assert_eq!(record.id, "node-id");
    assert_eq!(record.hostname, "clamps-test-smoke");
    assert_eq!(record.addresses.len(), 2);
    assert!(device_record(&value, "tag:skillet-server").is_none());
}

fn fixture(responses: Vec<(u16, &'static str)>) -> (String, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind HTTP fixture");
    let address = listener.local_addr().expect("fixture address");
    let handle = thread::spawn(move || {
        let mut requests = Vec::new();
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().expect("accept HTTP request");
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(3)))
                .expect("set read timeout");
            let mut request = Vec::new();
            let mut buffer = [0u8; 1024];
            loop {
                let read = stream.read(&mut buffer).expect("read request");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                let text = String::from_utf8_lossy(&request);
                let Some((headers, payload)) = text.split_once("\r\n\r\n") else {
                    continue;
                };
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|value| value.parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if payload.len() >= content_length {
                    break;
                }
            }
            requests.push(String::from_utf8_lossy(&request).into_owned());
            let reason = if status == 200 { "OK" } else { "Forbidden" };
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        }
        requests
    });
    (format!("http://{address}/api/v2"), handle)
}

#[test]
fn auth_key_requests_use_the_injected_http_endpoint() {
    let (base, server) = fixture(vec![
        (200, r#"{"access_token":"oauth-access"}"#),
        (200, r#"{"key":"tskey-test"}"#),
    ]);
    let credentials =
        OAuthCredentials::with_api_base("client-id".into(), "client-secret".into(), &base)
            .expect("test credentials");
    let issued = create_auth_key(&credentials, SMOKE_TAG, "local fixture").expect("auth key");
    assert_eq!(issued.key, "tskey-test");
    let requests = server.join().expect("fixture thread");
    assert!(requests[0].starts_with("POST /api/v2/oauth/token "));
    assert!(requests[0].contains("client_secret=client-secret"));
    assert!(requests[1].starts_with("POST /api/v2/tailnet/-/keys "));
    assert!(requests[1]
        .to_ascii_lowercase()
        .contains("authorization: bearer oauth-access"));
}

#[test]
fn device_lookup_and_removal_use_the_same_http_client() {
    let (base, server) = fixture(vec![
        (200, r#"{"access_token":"oauth-access"}"#),
        (
            200,
            r#"{"devices":[{"id":"device-1","hostname":"clamps-test-smoke","addresses":["100.64.0.5"],"tags":["tag:skillet-smoke"]}]}"#,
        ),
        (200, r#"{"access_token":"oauth-access"}"#),
        (
            200,
            r#"{"devices":[{"id":"device-1","hostname":"clamps-test-smoke","addresses":["100.64.0.5"],"tags":["tag:skillet-smoke"]}]}"#,
        ),
        (200, "{}"),
    ]);
    let credentials =
        OAuthCredentials::with_api_base("client-id".into(), "client-secret".into(), &base)
            .expect("test credentials");
    let found = find_device_by_hostname(&credentials, "clamps-test-smoke", SMOKE_TAG)
        .expect("device lookup");
    assert_eq!(found.id, "device-1");
    let removed = remove_device_for_hostname(&credentials, "clamps-test-smoke", SMOKE_TAG, None)
        .expect("device removal");
    assert_eq!(removed.expect("removed record").id, "device-1");
    let requests = server.join().expect("fixture thread");
    assert!(requests[1].starts_with("GET /api/v2/tailnet/-/devices "));
    assert!(requests[4].starts_with("DELETE /api/v2/device/device-1 "));
}

#[test]
fn api_errors_keep_status_context() {
    let (base, server) = fixture(vec![(403, r#"{"message":"denied"}"#)]);
    let credentials =
        OAuthCredentials::with_api_base("client-id".into(), "client-secret".into(), &base)
            .expect("test credentials");
    let error = credentials.access_token("auth_keys").unwrap_err();
    assert!(error.to_string().contains("HTTP 403"));
    assert!(error.to_string().contains("denied"));
    server.join().expect("fixture thread");
}
