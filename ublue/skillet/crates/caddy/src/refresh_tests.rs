//! Exercise the browser bootstrap response through real Caddy, without provider credentials.
use crate::{CaddySites, UiEnvironment, UiService};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

struct HttpStub {
    server: Arc<tiny_http::Server>,
    address: std::net::SocketAddr,
    allowed: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl HttpStub {
    fn new(authorization: bool) -> Self {
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let address = server.server_addr().to_ip().unwrap();
        let allowed = Arc::new(AtomicBool::new(true));
        let stopped = Arc::new(AtomicBool::new(false));
        let (thread_server, thread_allowed, thread_stopped) =
            (server.clone(), allowed.clone(), stopped.clone());
        let worker = std::thread::spawn(move || {
            while !thread_stopped.load(Ordering::SeqCst) {
                if let Some(request) = thread_server
                    .recv_timeout(Duration::from_millis(100))
                    .unwrap()
                {
                    let response = if authorization {
                        assert_eq!(request.method(), &tiny_http::Method::Get);
                        assert!(matches!(request.url(), "/auth" | "/auth?"));
                        let status = if thread_allowed.load(Ordering::SeqCst) {
                            204
                        } else {
                            403
                        };
                        tiny_http::Response::from_string("").with_status_code(status)
                    } else {
                        tiny_http::Response::from_string(format!(
                            "{} {}",
                            request.method(),
                            request.url()
                        ))
                    };
                    let _ = request.respond(response);
                }
            }
        });
        Self {
            server,
            address,
            allowed,
            stopped,
            worker: Some(worker),
        }
    }
}
impl Drop for HttpStub {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        self.server.unblock();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct Container(String);
impl Drop for Container {
    fn drop(&mut self) {
        let _ = std::process::Command::new("podman")
            .args(["rm", "--force", &self.0])
            .output();
    }
}

#[test]
#[ignore = "requires Podman and the Caddy container image"]
fn real_caddy_no_auth_refresh_stays_behind_identity_gate() {
    let temp = tempfile::tempdir().unwrap();
    let stub = HttpStub::new(true);
    let (_container, address) =
        start_caddy(temp.path(), "files", false, "127.0.0.1:9", stub.address);
    let base = format!("http://{address}");
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let response = loop {
        match client
            .get(format!("{base}/api/refresh?initial=1"))
            .header("Authorization", "Bearer expired")
            .send()
        {
            Ok(response) => break response,
            Err(error) => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "Caddy not ready: {error}"
                );
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    };
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(response.headers()["Content-Type"], "application/json");
    assert_eq!(response.headers()["Cache-Control"], "no-store");
    let body: serde_json::Value = response.json().unwrap();
    assert_eq!(body["token"], "skillet-tailnet-authenticated");
    stub.allowed.store(false, Ordering::SeqCst);
    let denied = client
        .get(format!("{base}/api/refresh"))
        .header("X-Skillet-User", "forged@example.invalid")
        .send()
        .unwrap();
    assert_eq!(denied.status().as_u16(), 403);
    assert!(!denied
        .text()
        .unwrap()
        .contains("skillet-tailnet-authenticated"));
    stub.allowed.store(true, Ordering::SeqCst);
    let cross_origin = client
        .get(format!("{base}/api/refresh"))
        .header("Origin", "https://evil.example.invalid")
        .send()
        .unwrap();
    assert_eq!(cross_origin.status().as_u16(), 403);
    for response in [
        client.post(format!("{base}/api/refresh")).send().unwrap(),
        client.get(format!("{base}/api/browse/")).send().unwrap(),
    ] {
        assert_eq!(
            response.status().as_u16(),
            502,
            "non-bootstrap requests must reach the deliberately absent backend"
        );
    }
}

#[test]
#[ignore = "requires Podman and the Caddy container image"]
fn real_caddy_authorizes_empty_queries_and_preserves_backend_uri() {
    let temp = tempfile::tempdir().unwrap();
    let auth = HttpStub::new(true);
    let backend = HttpStub::new(false);
    let (_container, address) = start_caddy(
        temp.path(),
        "files",
        false,
        &backend.address.to_string(),
        auth.address,
    );
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    for path in [
        "/api/browse/",
        "/api/browse/?",
        "/api/browse/?search=",
        "/api/browse/?search=ebooks",
    ] {
        let response = loop {
            match client.get(format!("http://{address}{path}")).send() {
                Ok(response) => break response,
                Err(error) => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "Caddy not ready: {error}"
                    );
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        };
        assert_eq!(response.status().as_u16(), 200, "{path}");
        assert_eq!(response.text().unwrap(), format!("GET {path}"));
    }
    auth.allowed.store(false, Ordering::SeqCst);
    let denied = client
        .get(format!("http://{address}/api/browse/?"))
        .send()
        .unwrap();
    assert_eq!(denied.status().as_u16(), 403);
}

#[test]
#[ignore = "requires Podman and the Caddy container image"]
fn real_caddy_homeassistant_navigation_retains_identity_and_origin_guards() {
    for with_homer in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let auth = HttpStub::new(true);
        let backend = HttpStub::new(false);
        let (_container, address) = start_caddy(
            temp.path(),
            "homeassistant",
            with_homer,
            &backend.address.to_string(),
            auth.address,
        );
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let base = format!("http://{address}");
        let path = "/_my_redirect/config_flow_start?domain=hue";
        let navigation = |method, path: &str, mode, destination| {
            client
                .request(method, format!("{base}{path}"))
                .header("Sec-Fetch-Site", "cross-site")
                .header("Sec-Fetch-Mode", mode)
                .header("Sec-Fetch-Dest", destination)
        };
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        let response = loop {
            match navigation(reqwest::Method::GET, path, "navigate", "document").send() {
                Ok(response) => break response,
                Err(error) => {
                    assert!(
                        std::time::Instant::now() < deadline,
                        "Caddy not ready: {error}"
                    );
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        };
        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(response.text().unwrap(), format!("GET {path}"));
        for (method, path, mode, destination) in [
            (reqwest::Method::POST, path, "navigate", "document"),
            (reqwest::Method::GET, path, "cors", "empty"),
            (reqwest::Method::GET, path, "navigate", "iframe"),
            (reqwest::Method::GET, path, "", ""),
            (reqwest::Method::GET, "/api/config", "navigate", "document"),
            (
                reqwest::Method::GET,
                "/_my_redirect/config_flow_start/extra",
                "navigate",
                "document",
            ),
        ] {
            assert_eq!(
                navigation(method, path, mode, destination)
                    .send()
                    .unwrap()
                    .status()
                    .as_u16(),
                403
            );
        }
        assert_eq!(
            navigation(reqwest::Method::GET, path, "navigate", "document")
                .header("Origin", "https://evil.example.invalid")
                .send()
                .unwrap()
                .status()
                .as_u16(),
            403
        );
        auth.allowed.store(false, Ordering::SeqCst);
        assert_eq!(
            navigation(reqwest::Method::GET, path, "navigate", "document")
                .header("X-Skillet-User", "forged@example.invalid")
                .send()
                .unwrap()
                .status()
                .as_u16(),
            403
        );
    }
}

fn start_caddy(
    root: &Path,
    service: &str,
    with_homer: bool,
    upstream: &str,
    authorization: std::net::SocketAddr,
) -> (Container, std::net::SocketAddr) {
    let mut services = vec![UiService {
        public_names: Vec::new(),
        cache: None,
        name: service.into(),
        upstream: "backend".into(),
        transport: crate::UpstreamTransport::Bridge,
        port: 8080,
        aliases: Vec::new(),
        tailnet_identity: true,
        anonymous_token_refresh: service == "files",
        sharing: None,
    }];
    if with_homer {
        let mut homer = services[0].clone();
        homer.name = "homer".into();
        homer.anonymous_token_refresh = false;
        services.push(homer);
    }
    let sites = CaddySites::from_host(
        "fixture",
        &UiEnvironment {
            ui_domain: "ui.example.invalid".into(),
            acme_staging: true,
        },
        &services,
    )
    .unwrap();
    let rendered = sites.render();
    // Loopback-only TCP fixtures avoid granting container processes access to
    // host-domain Unix sockets on enforcing hosts. This exercises HTTP guards;
    // production Unix transport and real WhoIs policy have separate acceptance.
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reservation.local_addr().unwrap();
    let site = rendered
        .split(&format!("\n{service}.fixture.ui.example.invalid {{"))
        .nth(1)
        .unwrap()
        .split("\nhomer.fixture.ui.example.invalid {")
        .next()
        .unwrap();
    let configuration = format!("{{\n admin off\n auto_https off\n}}\nhttp://{address} {{{site}")
        .replace("    tls {\n        resolvers 1.1.1.1 1.0.0.1\n    }\n", "")
        .replace(
            "not remote_ip 100.64.0.0/10 fd7a:115c:a1e0::/48",
            "not remote_ip 0.0.0.0/0 ::/0",
        )
        .replace(
            "unix//run/skillet-tailnet-ui/auth.sock",
            &format!("http://{authorization}"),
        )
        .replace(
            "reverse_proxy backend:8080",
            &format!("reverse_proxy {upstream}"),
        );
    std::fs::write(root.join("Caddyfile"), configuration).unwrap();
    drop(reservation);
    let container = Container(format!(
        "skillet-refresh-{}-{}",
        std::process::id(),
        root.file_name().unwrap().to_string_lossy()
    ));
    let volume = format!("{}:/fixture:z", root.display());
    let output = std::process::Command::new("podman")
        .args([
            "run",
            "--detach",
            "--name",
            &container.0,
            "--network=host",
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
    assert!(address.ip().is_loopback());
    (container, address)
}
