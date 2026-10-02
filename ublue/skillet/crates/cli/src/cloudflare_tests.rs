use super::*;
use std::{
    io::{Read, Write},
    net::TcpListener,
    thread,
};

#[test]
fn desired_dns_contains_dual_stack_machine_ui_and_alias_records() {
    let services = vec![skillet_caddy::UiService {
        name: "syncthing".to_string(),
        upstream: "syncthing".to_string(),
        port: 8384,
        aliases: vec!["sync.{host}".to_string()],
    }];
    let sites = skillet_caddy::CaddySites::from_host(
        "clamps",
        &skillet_caddy::UiEnvironment {
            ui_domain: "example.test".to_string(),
            acme_staging: true,
        },
        &services,
    )
    .expect("valid sites");
    let addresses = BTreeSet::from(["100.64.0.2".to_string(), "fd7a:115c:a1e0::2".to_string()]);
    let records = desired_records(&sites.machine_hostname, &addresses, &sites)
        .expect("valid dual-stack records");
    assert!(records.contains(&DesiredRecord {
        name: "clamps.example.test".to_string(),
        record_type: "A".to_string(),
        content: "100.64.0.2".to_string(),
    }));
    assert!(records.contains(&DesiredRecord {
        name: "clamps.example.test".to_string(),
        record_type: "AAAA".to_string(),
        content: "fd7a:115c:a1e0::2".to_string(),
    }));
    assert!(records.contains(&DesiredRecord {
        name: "syncthing.clamps.example.test".to_string(),
        record_type: "CNAME".to_string(),
        content: "clamps.example.test".to_string(),
    }));
    assert!(records.contains(&DesiredRecord {
        name: "sync.clamps.example.test".to_string(),
        record_type: "CNAME".to_string(),
        content: "syncthing.clamps.example.test".to_string(),
    }));
}

#[test]
fn desired_dns_rejects_missing_address_family() {
    let sites = skillet_caddy::CaddySites::from_host(
        "clamps",
        &skillet_caddy::UiEnvironment {
            ui_domain: "example.test".to_string(),
            acme_staging: true,
        },
        &[skillet_caddy::UiService {
            name: "syncthing".to_string(),
            upstream: "syncthing".to_string(),
            port: 8384,
            aliases: Vec::new(),
        }],
    )
    .expect("valid sites");
    let addresses = BTreeSet::from(["100.64.0.2".to_string()]);
    assert!(desired_records(&sites.machine_hostname, &addresses, &sites).is_err());
}

#[test]
fn sdk_transport_calls_cloudflare_with_bearer_token() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("client");
        let mut request = [0_u8; 4096];
        let count = stream.read(&mut request).expect("request");
        let request = String::from_utf8_lossy(&request[..count]).to_ascii_lowercase();
        assert!(request.contains("authorization: bearer test-token"));
        assert!(request.contains("/client/v4/zones/0123456789abcdef0123456789abcdef"));
        let body = r#"{"success":true,"errors":[],"messages":[],"result":{"id":"0123456789abcdef0123456789abcdef","name":"example.test"}}"#;
        write!(stream, "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).expect("response");
    });
    let api = Cloudflare::with_base(&format!("http://{address}/client/v4/"));
    let zone = api
        .zone("test-token", "0123456789abcdef0123456789abcdef")
        .expect("zone request");
    assert_eq!(zone.name, "example.test");
    server.join().expect("server thread");
}

#[test]
fn forbidden_zone_lookup_reports_permission_context_without_zone_id() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("client");
        let _request = read_request(&mut stream);
        let body = r#"{"success":false,"errors":[{"code":9109,"message":"Unauthorized to access requested resource"}],"messages":[],"result":null}"#;
        write!(stream, "HTTP/1.1 403 Forbidden\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).expect("response");
    });
    let api = Cloudflare::with_base(&format!("http://{address}/client/v4/"));
    let error = api
        .zone("issuer-token", "0123456789abcdef0123456789abcdef")
        .expect_err("forbidden lookup");
    let diagnostic = format!("{error:#}");
    assert!(diagnostic.contains("reading configured Cloudflare zone"));
    assert!(diagnostic.contains("Zone > Zone > Read"));
    assert!(diagnostic.contains("403"));
    assert!(!diagnostic.contains("issuer-token"));
    assert!(!diagnostic.contains("0123456789abcdef0123456789abcdef"));
    server.join().expect("server thread");
}

#[test]
fn discovers_permission_group_ids_and_creates_scoped_token_through_sdk() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = thread::spawn(move || {
        for (expected_path, body) in [
            (
                "/client/v4/user/tokens/permission_groups",
                r#"{"success":true,"errors":[],"messages":[],"result":[{"id":"zone-read-id","name":"Zone Read","scopes":["com.cloudflare.api.account.zone"],"is_selectable":true},{"id":"dns-write-id","name":"DNS Write","scopes":["com.cloudflare.api.account.zone"],"is_selectable":true}]}"#,
            ),
            (
                "/client/v4/user/tokens",
                r#"{"success":true,"errors":[],"messages":[],"result":{"id":"0123456789abcdef0123456789abcdef","value":"child-secret","expires_on":"2026-10-02T00:00:00Z"}}"#,
            ),
        ] {
            let (mut stream, _) = listener.accept().expect("client");
            let request = read_request(&mut stream);
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer issuer-token"));
            assert!(request.contains(expected_path));
            if expected_path.ends_with("user/tokens")
                && !expected_path.ends_with("permission_groups")
            {
                let (_, request_body) = request.split_once("\r\n\r\n").expect("POST body");
                let payload: Value = serde_json::from_str(request_body).expect("JSON payload");
                assert_eq!(
                    payload["policies"][0]["permission_groups"][0]["id"],
                    "zone-read-id"
                );
                assert_eq!(
                    payload["policies"][0]["permission_groups"][1]["id"],
                    "dns-write-id"
                );
                assert_eq!(
                    payload["policies"][0]["resources"]
                        ["com.cloudflare.api.account.zone.0123456789abcdef0123456789abcdef"],
                    "*"
                );
                assert_eq!(payload["name"], "skillet:test:clamps-smoke");
                assert!(payload.get("expires_on").is_some());
            }
            write!(stream, "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).expect("response");
        }
    });
    let api = Cloudflare::with_base(&format!("http://{address}/client/v4/"));
    let token = api
        .create_zone_token(
            "issuer-token",
            "0123456789abcdef0123456789abcdef",
            "skillet:test:clamps-smoke",
            Some(Duration::from_mins(30)),
        )
        .expect("scoped token");
    assert_eq!(token.value, "child-secret");
    assert_eq!(token.id, "0123456789abcdef0123456789abcdef");
    server.join().expect("server thread");
}

fn read_request(stream: &mut std::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 2048];
    loop {
        let count = stream.read(&mut chunk).expect("request bytes");
        request.extend_from_slice(&chunk[..count]);
        let text = String::from_utf8_lossy(&request);
        let Some((headers, body)) = text.split_once("\r\n\r\n") else {
            continue;
        };
        let body_length = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .unwrap_or_default();
        if body.len() >= body_length {
            return text.into_owned();
        }
    }
}

#[test]
fn zone_and_token_identifiers_are_validated() {
    assert!(validate_zone_id("0123456789abcdef0123456789abcdef").is_ok());
    assert!(validate_zone_id("example.test").is_err());
    assert!(validate_token_id("0123456789abcdef0123456789abcdef").is_ok());
    assert!(validate_token_id("../token").is_err());
}
