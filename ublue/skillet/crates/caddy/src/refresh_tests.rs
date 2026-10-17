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

struct AuthStub {
    server: Arc<tiny_http::Server>,
    allowed: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl AuthStub {
    fn new(path: &Path) -> Self {
        let server = Arc::new(tiny_http::Server::http_unix(path).unwrap());
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
                    assert_eq!(request.method(), &tiny_http::Method::Get);
                    assert_eq!(request.url(), "/auth");
                    let status = if thread_allowed.load(Ordering::SeqCst) {
                        200
                    } else {
                        403
                    };
                    let _ = request.respond(tiny_http::Response::empty(status));
                }
            }
        });
        Self {
            server,
            allowed,
            stopped,
            worker: Some(worker),
        }
    }
}
impl Drop for AuthStub {
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
    let stub = AuthStub::new(&temp.path().join("auth.sock"));
    let (_container, address) = start_caddy(temp.path());
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

fn start_caddy(root: &Path) -> (Container, std::net::SocketAddr) {
    let sites = CaddySites::from_host(
        "fixture",
        &UiEnvironment {
            ui_domain: "ui.example.invalid".into(),
            acme_staging: true,
        },
        &[UiService {
            name: "files".into(),
            upstream: "backend".into(),
            transport: crate::UpstreamTransport::Bridge,
            port: 8080,
            aliases: Vec::new(),
            tailnet_identity: true,
            anonymous_token_refresh: true,
        }],
    )
    .unwrap();
    let rendered = sites.render();
    // Use plain HTTP and allow the container ingress only in this loopback-published
    // fixture. Production rendering and the actual identity gate are unchanged.
    let site = rendered
        .split("\nfiles.fixture.ui.example.invalid {")
        .nth(1)
        .unwrap();
    let configuration = format!("{{\n admin off\n auto_https off\n}}\nhttp://:8080 {{{site}")
        .replace("    tls {\n        resolvers 1.1.1.1 1.0.0.1\n    }\n", "")
        .replace(
            "not remote_ip 100.64.0.0/10 fd7a:115c:a1e0::/48",
            "not remote_ip 0.0.0.0/0 ::/0",
        )
        .replace(
            "unix//run/skillet-tailnet-ui/auth.sock",
            "unix//fixture/auth.sock",
        )
        .replace("reverse_proxy backend:8080", "reverse_proxy 127.0.0.1:9");
    std::fs::write(root.join("Caddyfile"), configuration).unwrap();
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
    let output = std::process::Command::new("podman")
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
