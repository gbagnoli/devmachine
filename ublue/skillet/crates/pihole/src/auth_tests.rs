//! Verify retained-password migration against the actual Pi-hole consumer.
use std::{
    path::Path,
    time::{Duration, Instant},
};
struct Container(String);
impl Drop for Container {
    fn drop(&mut self) {
        let _ = std::process::Command::new("podman")
            .args(["rm", "--force", &self.0])
            .output();
    }
}
fn start(root: &Path, password: &str, suffix: &str) -> (Container, String) {
    let container = Container(format!(
        "skillet-pihole-auth-{}-{suffix}",
        std::process::id()
    ));
    let volume = format!("{}:/etc/pihole:z", root.display());
    let auth = format!("FTLCONF_webserver_api_password={password}");
    let output = std::process::Command::new("podman")
        .args([
            "run",
            "--detach",
            "--name",
            &container.0,
            "--publish",
            "127.0.0.1::8088",
            "--volume",
            &volume,
            "--env",
            &auth,
            "--env",
            "FTLCONF_webserver_port=8088o",
            "--env",
            "FTLCONF_dns_listeningMode=ALL",
            "docker.io/pihole/pihole:latest",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = std::process::Command::new("podman")
        .args(["port", &container.0, "8088/tcp"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let address: std::net::SocketAddr = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(address.ip().is_loopback());
    (container, format!("http://{address}"))
}
fn wait_status(client: &reqwest::blocking::Client, url: &str, wanted: u16) {
    let end = Instant::now() + Duration::from_secs(90);
    loop {
        if client
            .get(url)
            .send()
            .is_ok_and(|response| response.status().as_u16() == wanted)
        {
            return;
        }
        assert!(
            Instant::now() < end,
            "Pi-hole endpoint did not reach expected status {wanted}"
        );
        std::thread::sleep(Duration::from_millis(250));
    }
}
#[test]
#[ignore = "requires Podman and the Pi-hole container image"]
fn real_pihole_proxy_mode_overrides_retained_password_and_survives_restart() {
    let temp = tempfile::tempdir().unwrap();
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let (native, base) = start(temp.path(), "fixture-password", "native");
    wait_status(&client, &format!("{base}/api/stats/summary"), 401);
    let output = std::process::Command::new("podman")
        .args([
            "exec",
            &native.0,
            "touch",
            "/etc/pihole/skillet-preserved-marker",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    drop(native);
    let (proxy, base) = start(temp.path(), "", "proxy");
    wait_status(&client, &format!("{base}/api/stats/summary"), 200);
    let output = std::process::Command::new("podman")
        .args(["restart", &proxy.0])
        .output()
        .unwrap();
    assert!(output.status.success());
    wait_status(&client, &format!("{base}/api/stats/summary"), 200);
    let output = std::process::Command::new("podman")
        .args([
            "exec",
            &proxy.0,
            "test",
            "-f",
            "/etc/pihole/skillet-preserved-marker",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
#[ignore = "requires Podman and the Pi-hole container image"]
fn real_pihole_v6_private_records_preserve_state_and_survive_restart() {
    let temp = tempfile::tempdir().unwrap();
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let (container, base) = start(temp.path(), "", "dns");
    wait_status(&client, &format!("{base}/api/stats/summary"), 200);
    let invoke = |args: &[&str]| {
        let output = std::process::Command::new("podman")
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    let original = invoke(&["exec", &container.0, "cat", "/etc/pihole/pihole.toml"]);
    let input = crate::dns::Input::parse(r#"{"version":1,"records":[{"names":["printer.home.example.invalid","laser.home.example.invalid"],"addresses":["192.0.2.20","fd12:3456:789a::20"]}]}"#).unwrap();
    let (text, changed) = crate::dns::render(&original, &input.hosts().unwrap()).unwrap();
    assert!(changed);
    let before = std::str::from_utf8(&original)
        .unwrap()
        .parse::<toml_edit::DocumentMut>()
        .unwrap();
    let after = text.parse::<toml_edit::DocumentMut>().unwrap();
    assert_eq!(
        before["webserver"].to_string(),
        after["webserver"].to_string()
    );
    invoke(&["stop", &container.0]);
    let staged = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(staged.path(), text).unwrap();
    let source = staged.path().to_str().unwrap();
    let destination = format!("{}:/etc/pihole/pihole.toml", container.0);
    invoke(&["cp", source, &destination]);
    for _ in 0..2 {
        invoke(&["start", &container.0]);
        wait_status(&client, &format!("{base}/api/stats/summary"), 200);
        for (family, answer) in [("A", "192.0.2.20"), ("AAAA", "fd12:3456:789a::20")] {
            for transport in ["+notcp", "+tcp"] {
                for resolver in ["@127.0.0.1", "@::1"] {
                    let answer_bytes = invoke(&[
                        "exec",
                        &container.0,
                        "dig",
                        "+short",
                        "+time=3",
                        "+tries=1",
                        transport,
                        resolver,
                        "printer.home.example.invalid",
                        family,
                    ]);
                    assert_eq!(String::from_utf8(answer_bytes).unwrap().trim(), answer);
                }
            }
        }
        let current = invoke(&["exec", &container.0, "cat", "/etc/pihole/pihole.toml"]);
        assert!(
            !crate::dns::render(&current, &input.hosts().unwrap())
                .unwrap()
                .1
        );
        invoke(&["stop", &container.0]);
    }
}
