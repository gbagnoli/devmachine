//! Exercises the pinned consumer, not a mocked cache renderer.
use std::{
    process::Command,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

struct Fixture {
    container: String,
    root: tempfile::TempDir,
    stop: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
    requests: Arc<AtomicUsize>,
    unavailable: Arc<AtomicBool>,
    address: String,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = Command::new("podman")
            .args(["rm", "--force", &self.container])
            .output();
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        validate_tls(root.path());
        let server = tiny_http::Server::http_unix(&root.path().join("backend.sock")).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(AtomicUsize::new(0));
        let unavailable = Arc::new(AtomicBool::new(false));
        let worker_unavailable = unavailable.clone();
        let (worker_stop, worker_requests) = (stop.clone(), requests.clone());
        let worker = std::thread::spawn(move || {
            while !worker_stop.load(Ordering::SeqCst) {
                let Some(request) = server.recv_timeout(Duration::from_millis(100)).unwrap() else {
                    continue;
                };
                worker_requests.fetch_add(1, Ordering::SeqCst);
                let body = if request.url().contains("large") {
                    vec![b'x'; 3 * 1024 * 1024]
                } else {
                    b"fixture-static-content".to_vec()
                };
                let mut response = tiny_http::Response::from_data(body);
                let (status, control) =
                    response_policy(request.url(), worker_unavailable.load(Ordering::SeqCst));
                response = response
                    .with_status_code(status)
                    .with_header(tiny_http::Header::from_bytes("Cache-Control", control).unwrap());
                if request.url().contains("cookie") {
                    response = response.with_header(
                        tiny_http::Header::from_bytes("Set-Cookie", "session=fixture; Secure")
                            .unwrap(),
                    );
                }
                let _ = request.respond(response);
            }
        });
        let config = fixture_config();
        std::fs::write(root.path().join("Caddyfile"), config).unwrap();
        let container = format!(
            "skillet-cache-{}-{}",
            std::process::id(),
            root.path().file_name().unwrap().to_string_lossy()
        );
        let volume = format!("{}:/fixture:z", root.path().display());
        let mut fixture = Self {
            container,
            root,
            stop,
            worker: Some(worker),
            requests,
            unavailable,
            address: String::new(),
        };
        let output = Command::new("podman")
            .args([
                "run",
                "--detach",
                "--name",
                &fixture.container,
                "--publish",
                "127.0.0.1::8080",
                "--memory",
                "512m",
                "--tmpfs",
                "/var/cache/skillet/jellyfin:rw,size=128m,mode=0700",
                "--volume",
                &volume,
                super::IMAGE,
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
            .args(["port", &fixture.container, "8080/tcp"])
            .output()
            .unwrap();
        fixture.address = String::from_utf8(output.stdout).unwrap().trim().into();
        fixture
    }

    fn get(&self, path: &str, headers: &[(&str, &str)]) -> reqwest::blocking::Response {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let mut request = client
            .get(format!("http://{}{path}", self.address))
            .header("Host", "video.example.invalid");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        request.send().unwrap()
    }

    fn bypassed(&self, path: &str, headers: &[(&str, &str)]) {
        let count = self.requests.load(Ordering::SeqCst);
        for _ in 0..2 {
            drop(self.get(path, headers).bytes().unwrap());
        }
        assert_eq!(
            self.requests.load(Ordering::SeqCst),
            count + 2,
            "unexpected cached response: {path}"
        );
    }
}

fn validate_tls(root: &std::path::Path) {
    std::fs::write(root.join("TLS-Caddyfile"), super::tests::sites().render()).unwrap();
    let volume = format!("{}:/fixture:z", root.display());
    let output = Command::new("podman")
        .args([
            "run",
            "--rm",
            "--volume",
            &volume,
            "--tmpfs",
            "/var/cache/skillet/jellyfin:rw,size=128m,mode=0700",
            "--env",
            "CF_API_TOKEN=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            super::IMAGE,
            "caddy",
            "validate",
            "--config",
            "/fixture/TLS-Caddyfile",
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
}

#[test]
#[ignore = "requires the pinned cache image built from image/Containerfile and Podman"]
fn real_caddy_static_cache_hits_expire_and_never_cache_credentials_or_media() {
    let mut fixture = Fixture::new();
    // Consumer parsing/startup is bounded and failures retain a readable log.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        let output = Command::new("podman")
            .args(["logs", &fixture.container])
            .output()
            .unwrap();
        if String::from_utf8_lossy(&output.stderr).contains("serving initial configuration") {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(fixture.root.path().join("Caddyfile").exists());
    drop(fixture.get("/web/fixture.js", &[]).bytes().unwrap());
    let before = fixture.requests.load(Ordering::SeqCst);
    let response = fixture.get("/web/fixture.js", &[]);
    assert!(response
        .headers()
        .get("Cache-Status")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("hit"));
    drop(response.bytes().unwrap());
    assert_eq!(fixture.requests.load(Ordering::SeqCst), before);
    drop(fixture.get("/web/expiry.js", &[]).bytes().unwrap());
    let before = fixture.requests.load(Ordering::SeqCst);
    std::thread::sleep(Duration::from_secs(2));
    drop(fixture.get("/web/expiry.js", &[]).bytes().unwrap());
    assert!(fixture.requests.load(Ordering::SeqCst) > before);
    verify_bypasses(&fixture);
    verify_hard_storage_limit(&fixture);
    verify_origin_failure_and_redacted_logs(&mut fixture);
}

fn verify_bypasses(fixture: &Fixture) {
    for (path, headers) in [
        ("/web/large.js", vec![]),
        ("/web/error.js", vec![]),
        ("/web/partial.js", vec![]),
        ("/web/private.js", vec![]),
        ("/web/no-store.js", vec![]),
        ("/web/cookie.js", vec![]),
        ("/web/fixture.js?api_key=fixture", vec![]),
        ("/web/fixture.js", vec![("Authorization", "Bearer fixture")]),
        ("/web/fixture.js", vec![("X-Emby-Token", "fixture")]),
        ("/web/fixture.js", vec![("Cookie", "session=fixture")]),
        ("/web/fixture.js", vec![("Range", "bytes=0-10")]),
        (
            "/Items/fixture/Images/Primary",
            vec![("X-Emby-Token", "revoked")],
        ),
        ("/Videos/fixture/stream", vec![]),
        ("/Users/AuthenticateByName", vec![]),
    ] {
        fixture.bypassed(path, &headers);
    }
}

fn verify_hard_storage_limit(fixture: &Fixture) {
    let output = Command::new("podman")
        .args(["inspect", &fixture.container])
        .output()
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value[0]["HostConfig"]["Memory"], 512 * 1024 * 1024);
    let output = Command::new("podman")
        .args([
            "exec",
            &fixture.container,
            "dd",
            "if=/dev/zero",
            "of=/var/cache/skillet/jellyfin/limit-probe",
            "bs=1048576",
            "count=129",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("No space left on device"));
    let output = Command::new("podman")
        .args([
            "exec",
            &fixture.container,
            "rm",
            "/var/cache/skillet/jellyfin/limit-probe",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(fixture.get("/web/recovered.js", &[]).status().is_success());
}

fn verify_origin_failure_and_redacted_logs(fixture: &mut Fixture) {
    drop(fixture.get("/web/stale.js", &[]).bytes().unwrap());
    fixture.unavailable.store(true, Ordering::SeqCst);
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(fixture.get("/web/stale.js", &[]).status().as_u16(), 503);
    assert_eq!(
        fixture
            .get("/__fixture-error?api_key=do-not-log-this-fixture", &[])
            .status()
            .as_u16(),
        502
    );
    let output = Command::new("podman")
        .args(["logs", &fixture.container])
        .output()
        .unwrap();
    for log in [&output.stdout, &output.stderr] {
        assert!(!String::from_utf8_lossy(log).contains("do-not-log-this-fixture"));
    }
}

fn fixture_config() -> String {
    let mut config = super::tests::sites().render();
    // Only this isolated fixture adds an unavailable upstream to exercise error-log redaction.
    config = config.replace("        @static_cache", "        handle /__fixture-error {\n            reverse_proxy unix//fixture/missing.sock\n        }\n        @static_cache");
    config = config
        .replace(
            "    acme_ca https://acme-staging-v02.api.letsencrypt.org/directory\n",
            "    auto_https off\n",
        )
        .replace("    acme_dns cloudflare {env.CF_API_TOKEN}\n", "")
        .replace(
            "video.example.invalid {",
            "http://video.example.invalid:8080 {",
        )
        .replace(
            "media.example.invalid {",
            "http://media.example.invalid:8080 {",
        )
        .replace("    tls {\n        resolvers 1.1.1.1 1.0.0.1\n    }\n", "")
        .replace(
            "reverse_proxy jellyfin:8096",
            "reverse_proxy unix//fixture/backend.sock",
        );

    config
}

fn response_policy(path: &str, unavailable: bool) -> (u16, &'static str) {
    if unavailable {
        (503, "no-store")
    } else if path.contains("error") {
        (404, "public,max-age=60")
    } else if path.contains("partial") {
        (206, "public,max-age=60")
    } else if path.starts_with("/Items/") {
        (401, "no-store")
    } else if path.contains("private") {
        (200, "private,max-age=60")
    } else if path.contains("no-store") {
        (200, "no-store")
    } else if path.contains("expiry") || path.contains("stale") {
        (200, "public,max-age=1")
    } else {
        (200, "public,max-age=60")
    }
}
