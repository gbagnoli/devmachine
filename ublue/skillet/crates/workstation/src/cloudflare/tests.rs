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
        let body = r#"{"success":true,"errors":[],"messages":[],"result":{"id":"0123456789abcdef0123456789abcdef","name":"example.test","account":{"id":"abcdef0123456789abcdef0123456789"}}}"#;
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
                "/client/v4/accounts/abcdef0123456789abcdef0123456789/tokens/permission_groups",
                r#"{"success":true,"errors":[],"messages":[],"result":[{"id":"zone-read-id","name":"Zone Read","scopes":["com.cloudflare.api.account.zone"],"is_selectable":true},{"id":"dns-write-id","name":"DNS Write","scopes":["com.cloudflare.api.account.zone"],"is_selectable":true}]}"#,
            ),
            (
                "/client/v4/accounts/abcdef0123456789abcdef0123456789/tokens",
                r#"{"success":true,"errors":[],"messages":[],"result":{"id":"0123456789abcdef0123456789abcdef","value":"child-secret","expires_on":"2026-10-02T00:00:00Z"}}"#,
            ),
        ] {
            let (mut stream, _) = listener.accept().expect("client");
            let request = read_request(&mut stream);
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer issuer-token"));
            assert!(request.contains(expected_path));
            if expected_path.ends_with("/tokens") {
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
            "abcdef0123456789abcdef0123456789",
            "skillet:test:clamps-smoke",
            Some(Duration::from_mins(30)),
        )
        .expect("scoped token");
    assert_eq!(token.value, "child-secret");
    assert_eq!(token.id, "0123456789abcdef0123456789abcdef");
    server.join().expect("server thread");
}

#[test]
fn lists_and_revokes_tokens_through_account_endpoints() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = thread::spawn(move || {
        for (expected, body) in [
            (
                "GET /client/v4/accounts/abcdef0123456789abcdef0123456789/tokens?",
                r#"{"success":true,"errors":[],"messages":[],"result":[{"id":"0123456789abcdef0123456789abcdef","name":"skillet:test:clamps-smoke"}],"result_info":{"total_pages":1}}"#,
            ),
            (
                "DELETE /client/v4/accounts/abcdef0123456789abcdef0123456789/tokens/0123456789abcdef0123456789abcdef",
                r#"{"success":true,"errors":[],"messages":[],"result":{}}"#,
            ),
        ] {
            let (mut stream, _) = listener.accept().expect("client");
            let request = read_request(&mut stream);
            assert!(request.starts_with(expected));
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer issuer-token"));
            write!(stream, "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).expect("response");
        }
    });
    let api = Cloudflare::with_base(&format!("http://{address}/client/v4/"));
    let ids = api
        .token_ids_by_name(
            "issuer-token",
            "abcdef0123456789abcdef0123456789",
            "skillet:test:clamps-smoke",
        )
        .expect("list account tokens");
    assert_eq!(ids, ["0123456789abcdef0123456789abcdef"]);
    api.revoke_token("issuer-token", "abcdef0123456789abcdef0123456789", &ids[0])
        .expect("revoke account token");
    server.join().expect("server thread");
}

#[test]
fn named_token_retry_recovers_and_replaces_a_token_after_ambiguous_creation() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = thread::spawn(move || {
        let mut tokens: Vec<Value> = Vec::new();
        let mut post_count = 0;
        for _ in 0..7 {
            let (mut stream, _) = listener.accept().expect("client");
            let request = read_request(&mut stream);
            let line = request.lines().next().expect("request line");
            let (headers, body) = request.split_once("\r\n\r\n").expect("headers");
            assert!(headers
                .to_ascii_lowercase()
                .contains("authorization: bearer issuer-token"));
            if line.starts_with("GET /client/v4/accounts/") && line.contains("/tokens?") {
                send_json(&mut stream, 200, &json!({"result": tokens}));
            } else if line.starts_with("GET /client/v4/accounts/")
                && line.contains("/tokens/permission_groups?")
            {
                send_json(
                    &mut stream,
                    200,
                    &json!({"result": [
                        {"id":"zone-read-id", "name":"Zone Read", "scopes":["com.cloudflare.api.account.zone"], "is_selectable":true},
                        {"id":"dns-write-id", "name":"DNS Write", "scopes":["com.cloudflare.api.account.zone"], "is_selectable":true}
                    ]}),
                );
            } else if line.starts_with("DELETE /client/v4/accounts/") {
                let id = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|path| path.split('/').next_back())
                    .expect("token ID");
                tokens.retain(|token| token["id"] != id);
                send_json(&mut stream, 200, &json!({"result": {}}));
            } else if line.starts_with("POST /client/v4/accounts/") && line.contains("/tokens ") {
                let payload: Value = serde_json::from_str(body).expect("token payload");
                post_count += 1;
                let id = if post_count == 1 {
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                } else {
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                };
                tokens.push(json!({"id":id, "name":payload["name"]}));
                if post_count == 1 {
                    // The API committed the token, but the caller saw a failed
                    // response. This models the retry ambiguity.
                    send_json(&mut stream, 500, &json!({"errors":[{"code":1}]}));
                } else {
                    send_json(
                        &mut stream,
                        200,
                        &json!({"result":{"id":id, "value":"replacement-secret"}}),
                    );
                }
            } else {
                panic!("unexpected mock Cloudflare request: {line}");
            }
        }
        assert_eq!(post_count, 2);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0]["id"], "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    });
    let api = Cloudflare::with_base(&format!("http://{address}/client/v4/"));
    let account = "abcdef0123456789abcdef0123456789";
    let zone = "0123456789abcdef0123456789abcdef";
    let name = "skillet:production:clamps";

    assert!(api
        .replace_named_zone_token("issuer-token", zone, account, name, None)
        .is_err());
    let retry = api
        .replace_named_zone_token("issuer-token", zone, account, name, None)
        .expect("retry replaces ambiguous first issuance");
    assert_eq!(retry.id, "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    assert_eq!(retry.value, "replacement-secret");
    server.join().expect("server thread");
}

#[test]
fn dns_reconcile_retry_does_not_duplicate_a_record_after_lost_create_response() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = thread::spawn(move || {
        let mut records = Vec::<Value>::new();
        let mut posts = 0;
        for _ in 0..4 {
            let (mut stream, _) = listener.accept().expect("client");
            let request = read_request(&mut stream);
            let line = request.lines().next().expect("request line");
            if line.starts_with("GET /client/v4/zones/") {
                send_json(&mut stream, 200, &json!({"result": records}));
            } else if line.starts_with("POST /client/v4/zones/") {
                let (_, body) = request.split_once("\r\n\r\n").expect("POST body");
                let payload: Value = serde_json::from_str(body).expect("record payload");
                posts += 1;
                let id = format!("{posts:032x}");
                let record = json!({
                    "id": id,
                    "name": payload["name"],
                    "type": payload["type"],
                    "content": payload["content"],
                    "comment": payload["comment"]
                });
                records.push(record.clone());
                if posts == 2 {
                    send_json(&mut stream, 500, &json!({"errors":[{"code":1}]}));
                } else {
                    send_json(&mut stream, 200, &json!({"result": record}));
                }
            } else {
                panic!("unexpected mock Cloudflare request: {line}");
            }
        }
        assert_eq!(posts, 2);
        assert_eq!(records.len(), 2);
    });
    let api = Cloudflare::with_base(&format!("http://{address}/client/v4/"));
    let desired = [
        DesiredRecord {
            name: "clamps.example.test".to_string(),
            record_type: "A".to_string(),
            content: "100.64.0.2".to_string(),
        },
        DesiredRecord {
            name: "syncthing.clamps.example.test".to_string(),
            record_type: "CNAME".to_string(),
            content: "clamps.example.test".to_string(),
        },
    ];
    assert!(api
        .reconcile_dns(
            "child-token",
            "0123456789abcdef0123456789abcdef",
            "skillet:test:clamps:smoke",
            "example.test",
            &desired,
        )
        .is_err());
    let reconciled = api
        .reconcile_dns(
            "child-token",
            "0123456789abcdef0123456789abcdef",
            "skillet:test:clamps:smoke",
            "example.test",
            &desired,
        )
        .expect("retry adopts both previously created records");
    assert_eq!(reconciled.records.len(), 2);
    server.join().expect("server thread");
}

#[test]
fn dns_cleanup_retry_finishes_after_a_partial_delete_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = thread::spawn(move || {
        let mut records = vec![
            json!({"id":"11111111111111111111111111111111", "name":"clamps.example.test", "type":"A", "content":"100.64.0.2", "comment":"skillet:test:clamps:smoke"}),
            json!({"id":"22222222222222222222222222222222", "name":"syncthing.clamps.example.test", "type":"CNAME", "content":"clamps.example.test", "comment":"skillet:test:clamps:smoke"}),
            json!({"id":"33333333333333333333333333333333", "name":"other.example.test", "type":"A", "content":"192.0.2.4", "comment":"other-owner"}),
        ];
        let mut fail_once = true;
        for _ in 0..5 {
            let (mut stream, _) = listener.accept().expect("client");
            let request = read_request(&mut stream);
            let line = request.lines().next().expect("request line");
            if line.starts_with("GET /client/v4/zones/") {
                send_json(&mut stream, 200, &json!({"result": records}));
            } else if line.starts_with("DELETE /client/v4/zones/") {
                let id = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|path| path.split('/').next_back())
                    .expect("record ID");
                if id.starts_with('2') && fail_once {
                    fail_once = false;
                    send_json(&mut stream, 500, &json!({"errors":[{"code":1}]}));
                } else {
                    records.retain(|record| record["id"] != id);
                    send_json(&mut stream, 200, &json!({"result": {}}));
                }
            } else {
                panic!("unexpected mock Cloudflare request: {line}");
            }
        }
        assert_eq!(records.len(), 1);
        assert_eq!(records[0]["comment"], "other-owner");
    });
    let api = Cloudflare::with_base(&format!("http://{address}/client/v4/"));
    assert!(api
        .remove_dns_marker(
            "child-token",
            "0123456789abcdef0123456789abcdef",
            "skillet:test:clamps:smoke",
            "example.test",
        )
        .is_err());
    api.remove_dns_marker(
        "child-token",
        "0123456789abcdef0123456789abcdef",
        "skillet:test:clamps:smoke",
        "example.test",
    )
    .expect("retry removes remaining owned record");
    server.join().expect("server thread");
}

#[test]
fn ddns_cleanup_refuses_an_unjournaled_owned_record_before_deleting() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = thread::spawn(move || {
        let records = json!([
            {"id":"11111111111111111111111111111111", "name":"host.example.test", "type":"A", "content":"203.0.113.10", "comment":"skillet-ddns:test:clamps-ddns"},
            {"id":"22222222222222222222222222222222", "name":"host.example.test", "type":"A", "content":"203.0.113.11", "comment":"skillet-ddns:test:clamps-ddns"}
        ]);
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().expect("client");
            let request = read_request(&mut stream);
            assert!(request.starts_with("GET /client/v4/zones/"));
            send_json(&mut stream, 200, &json!({"result": records}));
        }
    });
    let api = Cloudflare::with_base(&format!("http://{address}/client/v4/"));
    let error = api
        .remove_owned_ddns_records(
            "child-token",
            "0123456789abcdef0123456789abcdef",
            "skillet-ddns:test:clamps-ddns",
            &["host.example.test".into()],
            &["11111111111111111111111111111111".into()],
        )
        .expect_err("unrecorded duplicate must prevent cleanup");
    assert!(error.to_string().contains("identity changed"));
    server.join().expect("server thread");
}

fn send_json(stream: &mut std::net::TcpStream, status: u16, value: &Value) {
    let success = (200..300).contains(&status);
    let body = json!({
        "success": success,
        "errors": value.get("errors").cloned().unwrap_or_else(|| json!([])),
        "messages": [],
        "result": value.get("result").cloned().unwrap_or(Value::Null),
        "result_info": value.get("result_info").cloned().unwrap_or(Value::Null)
    })
    .to_string();
    let phrase = if success {
        "OK"
    } else {
        "Internal Server Error"
    };
    write!(stream, "HTTP/1.1 {status} {phrase}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).expect("mock response");
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
