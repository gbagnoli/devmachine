use super::*;

pub(super) fn public_service() -> UiService {
    UiService {
        name: "joplin".into(),
        upstream: "joplin-server".into(),
        transport: UpstreamTransport::Bridge,
        port: 22300,
        aliases: Vec::new(),
        public_names: vec!["notes".into()],
        cache: None,
        tailnet_identity: false,
        anonymous_token_refresh: false,
        sharing: None,
    }
}

#[test]
fn guest_setup_readiness_denies_public_proxying_without_changing_credentials() {
    let mut value = sites(&[public_service()]);
    let original = serde_json::to_string(&value).unwrap();
    value.services[0].setup_pending = true;
    assert_eq!(serde_json::to_string(&value).unwrap(), original);
    let rendered = value.render();
    assert!(rendered.contains("Service setup is incomplete\" 503"));
    assert!(!rendered.contains("reverse_proxy joplin-server"));
    let parsed = CaddySites::parse(&original, "server", &[public_service()]).unwrap();
    assert!(!parsed.services[0].setup_pending);
}

fn environment() -> UiEnvironment {
    UiEnvironment {
        ui_domain: "custom.ui.example.invalid".into(),
        acme_staging: true,
    }
}

fn sites(services: &[UiService]) -> CaddySites {
    CaddySites::from_host_with_zone("server", &environment(), services, Some("example.invalid"))
        .unwrap()
}

#[test]
fn public_names_are_zone_root_names_and_payloads_bind_the_access_policy() {
    let mut private = public_service();
    private.name = "files".into();
    private.upstream = "mikochi".into();
    private.public_names.clear();
    private.tailnet_identity = true;
    let declarations = [public_service(), private];
    let value = sites(&declarations);
    assert_eq!(value.version, 6);
    assert_eq!(value.services[0].hostname, "notes.example.invalid");
    assert_eq!(
        value.services[1].hostname,
        "files.server.custom.ui.example.invalid"
    );
    let rendered = value.render();
    let public = rendered
        .split("\nnotes.example.invalid {")
        .nth(1)
        .unwrap()
        .split("\n}\n")
        .next()
        .unwrap();
    assert!(!public.contains("outside_tailnet"));
    assert!(!public.contains("forward_auth"));
    assert!(public.contains("request_header -X-Skillet-User"));
    assert!(public.contains("reverse_proxy joplin-server:22300"));
    assert!(public.contains("Strict-Transport-Security"));
    assert_eq!(rendered.matches("respond @outside_tailnet").count(), 1);
    let encoded = serde_json::to_string(&value).unwrap();
    assert_eq!(
        CaddySites::parse(&encoded, "server", &declarations).unwrap(),
        value
    );
    let mut tampered = value.clone();
    tampered.services[1].access = SiteAccess::Public;
    assert!(CaddySites::parse(
        &serde_json::to_string(&tampered).unwrap(),
        "server",
        &declarations
    )
    .is_err());
    tampered = value;
    tampered.version = 5;
    assert!(CaddySites::parse(
        &serde_json::to_string(&tampered).unwrap(),
        "server",
        &declarations
    )
    .is_err());
}

#[test]
fn legacy_private_joplin_stays_identity_protected_until_redelivery() {
    let public = public_service();
    let mut legacy = public.clone();
    legacy.public_names.clear();
    legacy.tailnet_identity = true;
    let old = sites(&[legacy]);
    assert_eq!(old.version, 5);
    let encoded = serde_json::to_string(&old).unwrap();
    assert!(!encoded.contains("\"access\""));
    let retained = CaddySites::parse(&encoded, "server", std::slice::from_ref(&public)).unwrap();
    assert_eq!(retained, old);
    assert!(retained.render().contains("forward_auth"));
    assert_eq!(
        retained.services[0].hostname,
        "joplin.server.custom.ui.example.invalid"
    );
    let mut insecure = old;
    insecure.services[0].tailnet_identity = false;
    assert!(CaddySites::parse(
        &serde_json::to_string(&insecure).unwrap(),
        "server",
        &[public]
    )
    .is_err());
}

#[test]
fn public_aliases_share_the_same_upstream_and_invalid_policies_are_refused() {
    let mut service = public_service();
    service.name = "jellyfin".into();
    service.public_names = vec!["video".into(), "media".into()];
    let value = sites(&[service.clone()]);
    assert_eq!(value.services[0].hostname, "video.example.invalid");
    assert_eq!(value.services[0].aliases, ["media.example.invalid"]);
    assert_eq!(
        value
            .render()
            .matches("reverse_proxy joplin-server:22300")
            .count(),
        2
    );
    assert!(!value.render().contains("outside_tailnet"));
    for names in [vec!["*"], vec!["notes.bad"], vec!["notes", "notes"]] {
        service.public_names = names.into_iter().map(str::to_string).collect();
        assert!(CaddySites::from_host_with_zone(
            "server",
            &environment(),
            &[service.clone()],
            Some("example.invalid")
        )
        .is_err());
    }
    service = public_service();
    service.tailnet_identity = true;
    assert!(CaddySites::from_host_with_zone(
        "server",
        &environment(),
        &[service.clone()],
        Some("example.invalid")
    )
    .is_err());
    service.tailnet_identity = false;
    assert!(CaddySites::from_host("server", &environment(), &[service]).is_err());
}

struct FixtureContainer(String);
impl Drop for FixtureContainer {
    fn drop(&mut self) {
        let _ = std::process::Command::new("podman")
            .args(["rm", "--force", &self.0])
            .output();
    }
}

struct NativeBackend {
    server: std::sync::Arc<tiny_http::Server>,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl NativeBackend {
    fn new(path: &std::path::Path) -> Self {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        let server = Arc::new(tiny_http::Server::http_unix(path).unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let (worker_server, worker_stop) = (server.clone(), stop.clone());
        let worker = std::thread::spawn(move || {
            while !worker_stop.load(Ordering::SeqCst) {
                let Some(mut request) = worker_server
                    .recv_timeout(std::time::Duration::from_millis(100))
                    .unwrap()
                else {
                    continue;
                };
                let authorized = request.headers().iter().any(|header| {
                    header.field.equiv("X-API-AUTH")
                        && header.value.as_str() == "fixture-native-session"
                });
                let (status, body) = if request.url() == "/api/ping" {
                    (200, r#"{"status":"ok"}"#.to_string())
                } else if authorized {
                    let identity = request
                        .headers()
                        .iter()
                        .filter(|header| {
                            [
                                "X-Skillet-User",
                                "X-Webauth-User",
                                "Remote-User",
                                "Tailscale-User",
                            ]
                            .iter()
                            .any(|name| header.field.equiv(name))
                        })
                        .count();
                    let mut body = Vec::new();
                    request.as_reader().read_to_end(&mut body).unwrap();
                    (
                        200,
                        serde_json::json!({"method": request.method().as_str(), "uri": request.url(), "body": body, "identities": identity, "cookie": request.headers().iter().any(|h|h.field.equiv("Cookie")), "authorization":request.headers().iter().any(|h|h.field.equiv("Authorization")), "host": request.headers().iter().find(|header| header.field.equiv("Host")).map(|header| header.value.as_str())}).to_string(),
                    )
                } else {
                    (401, r#"{"error":"native authentication required"}"#.into())
                };
                request
                    .respond(tiny_http::Response::from_string(body).with_status_code(status))
                    .unwrap();
            }
        });
        Self {
            server,
            stop,
            worker: Some(worker),
        }
    }
}
impl Drop for NativeBackend {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        self.server.unblock();
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}

#[test]
#[ignore = "requires Podman and the Caddy container image"]
fn real_caddy_public_native_auth_and_private_denial_share_the_same_listener() {
    use std::time::{Duration, Instant};
    let root = tempfile::tempdir().unwrap();
    let _backend = NativeBackend::new(&root.path().join("backend.sock"));
    let (_container, address) = start_public_caddy(root.path());
    let base = format!("http://{address}");
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if client
            .get(format!("{base}/api/ping"))
            .header("Host", "notes.example.invalid")
            .send()
            .is_ok_and(|response| response.status().is_success())
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Caddy public fixture did not become ready"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let response = client
        .get(format!("{base}/api/users"))
        .header("Host", "notes.example.invalid")
        .header("X-Skillet-User", "alice@example.invalid")
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 401);
    assert!(response
        .text()
        .unwrap()
        .contains("native authentication required"));
    let response = client
        .post(format!("{base}/api/items?cursor=1&sync=2"))
        .header("Host", "notes.example.invalid")
        .header("X-API-AUTH", "fixture-native-session")
        .header("X-Skillet-User", "forged")
        .header("Tailscale-User", "forged")
        .body(vec![0_u8, 255, 128, 10, 42])
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().unwrap();
    assert_eq!(body["method"], "POST");
    assert_eq!(body["uri"], "/api/items?cursor=1&sync=2");
    assert_eq!(body["body"], serde_json::json!([0, 255, 128, 10, 42]));
    assert_eq!(body["identities"], 0);
    assert_eq!(body["host"], "notes.example.invalid");
    let response = client
        .get(&base)
        .header("Host", "files.server.custom.ui.example.invalid")
        .header("X-Forwarded-For", "100.64.0.1")
        .header("X-Skillet-User", "forged")
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 403);
    assert_eq!(
        response.text().unwrap(),
        "Access denied by Skillet tailnet policy"
    );
    verify_sharing(&client, &base);
    let response = client
        .get(&base)
        .header("Host", "unknown.example.invalid")
        .send()
        .unwrap();
    // An unmatched HTTP Host receives Caddy's empty default response, not an
    // upstream response. TLS SNI/cert selection is validated separately.
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(response.text().unwrap(), "");
}

fn start_public_caddy(root: &std::path::Path) -> (FixtureContainer, std::net::SocketAddr) {
    use std::process::Command;
    let mut private = public_service();
    private.name = "files".into();
    private.public_names.clear();
    private.tailnet_identity = true;
    private.sharing = Some(crate::sharing::Declaration {
        names: vec!["send".into()],
        policy: crate::sharing::Policy::Gokapi224,
    });
    let rendered = sites(&[public_service(), private]).render();
    // Only this loopback fixture replaces TLS/ACME with HTTP. Retain the actual
    // rendered access policy, proxy and identity-header handling unchanged.
    let body = rendered.split_once("\nnotes.example.invalid {").unwrap().1;
    let configuration =
        format!("{{\n admin off\n auto_https off\n}}\nhttp://notes.example.invalid:8080 {{{body}")
            .replace(
                "files.server.custom.ui.example.invalid {",
                "http://files.server.custom.ui.example.invalid:8080 {",
            )
            .replace("    tls {\n        resolvers 1.1.1.1 1.0.0.1\n    }\n", "")
            .replace(
                "send.example.invalid {",
                "http://send.example.invalid:8080 {",
            )
            .replace(
                "reverse_proxy joplin-server:22300",
                "reverse_proxy unix//fixture/backend.sock",
            );
    std::fs::write(root.join("Caddyfile"), configuration).unwrap();
    let container = FixtureContainer(format!(
        "skillet-public-{}-{}",
        std::process::id(),
        root.file_name().unwrap().to_string_lossy()
    ));
    let volume = format!("{}:/fixture:z", root.display());
    let output = Command::new("podman")
        .args([
            "run",
            "--detach",
            "--name",
            &container.0,
            "--publish",
            "127.0.0.1::8080",
            "--volume",
            &volume,
            "ghcr.io/caddybuilds/caddy-cloudflare:2",
            "caddy",
            "run",
            "--config",
            "/fixture/Caddyfile",
            "--adapter",
            "caddyfile",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new("podman")
        .args(["port", &container.0, "8080/tcp"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let address: std::net::SocketAddr = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(address.ip().is_loopback());
    (container, address)
}

fn verify_sharing(client: &reqwest::blocking::Client, base: &str) {
    for (method, path, expected) in [
        (reqwest::Method::GET, "/d?id=fixture", 200),
        (
            reqwest::Method::GET,
            "/publicUpload?id=fixture&key=guest",
            200,
        ),
        (reqwest::Method::POST, "/api/uploadrequest/chunk/add", 200),
        (reqwest::Method::GET, "/admin", 403),
        (reqwest::Method::GET, "/downloadPresigned", 403),
        (reqwest::Method::GET, "/api/files/list", 403),
        (reqwest::Method::POST, "/setup/setupResult", 403),
        (reqwest::Method::DELETE, "/downloadFile?id=fixture", 403),
        (reqwest::Method::GET, "/api/uploadrequest/chunk/add", 403),
        (reqwest::Method::GET, "/dangerous", 403),
    ] {
        let response = client
            .request(method, format!("{base}{path}"))
            .header("Host", "send.example.invalid")
            .header("Cookie", "pfixture=download-password-token")
            .header("Authorization", "Bearer forged")
            .header("X-API-AUTH", "fixture-native-session")
            .header("X-Skillet-User", "forged@example.invalid")
            .send()
            .unwrap();
        assert_eq!(response.status().as_u16(), expected, "{path}");
        if expected == 200 {
            let body = response.json::<serde_json::Value>().unwrap();
            assert_eq!(body["identities"], 0);
            assert_eq!(body["cookie"], true);
            assert_eq!(body["authorization"], false);
        }
    }
    for path in [
        "/%61dmin",
        "/d/%2e%2e/admin",
        "/assets/%2f..%2fadmin",
        "//admin",
    ] {
        let output = std::process::Command::new("curl")
            .args([
                "--silent",
                "--path-as-is",
                "--max-time",
                "10",
                "--output",
                "/dev/null",
                "--write-out",
                "%{http_code}",
                "--header",
                "Host: send.example.invalid",
                &format!("{base}{path}"),
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "403", "{path}");
    }
}
