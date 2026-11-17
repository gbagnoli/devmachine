use super::*;
#[test]
fn initial_settings_are_paused_keyed_and_category_scoped() {
    let ini = initial_configuration(&"a".repeat(32), &["sabnzbd.fixture.invalid"]).unwrap();
    assert!(ini.contains("start_paused = 1"));
    assert!(ini.contains("inet_exposure = 0"));
    assert!(!ini.contains("[servers]"));
    assert_eq!(retained_key(ini.as_bytes()).unwrap(), "a".repeat(32));
    for category in ["sonarr", "radarr", "lidarr"] {
        assert!(ini.contains(&format!("[[{category}]]")));
    }
    assert!(initial_configuration("private-bad-key", &["valid.invalid"]).is_err());
    assert!(initial_configuration(&"a".repeat(32), &["bad\nkey = exploit"]).is_err());
}

struct NativeContainer(String);
impl Drop for NativeContainer {
    fn drop(&mut self) {
        let _ = std::process::Command::new("podman")
            .args(["rm", "-f", &self.0])
            .output();
    }
}
#[test]
#[ignore = "requires Podman and the LSIO SABnzbd image"]
fn real_sabnzbd_is_paused_keyed_and_preserves_native_configuration() {
    use std::process::Command;
    let root = tempfile::tempdir().unwrap();
    let state = root.path().join("config");
    let downloads = root.path().join("downloads");
    std::fs::create_dir_all(&state).unwrap();
    std::fs::create_dir_all(&downloads).unwrap();
    let key = "a".repeat(32);
    std::fs::write(
        state.join("sabnzbd.ini"),
        initial_configuration(&key, &["sabnzbd.fixture.invalid"]).unwrap(),
    )
    .unwrap();
    let container = NativeContainer(format!("skillet-sabnzbd-{}", std::process::id()));
    let output = Command::new("podman")
        .args([
            "run",
            "--detach",
            "--name",
            &container.0,
            "--network",
            "none",
            "--env",
            "PUID=0",
            "--env",
            "PGID=0",
            "--volume",
            &format!("{}:/config:Z", state.display()),
            "--volume",
            &format!("{}:/data/downloads/usenet:Z", downloads.display()),
            IMAGE,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for attempt in 0..2 {
        wait_until_paused(&container.0, &key);
        for suffix in ["", "&apikey=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"] {
            let output = Command::new("podman")
                .args([
                    "exec",
                    &container.0,
                    "curl",
                    "--silent",
                    "--max-time",
                    "5",
                    "--write-out",
                    "\n%{http_code}",
                    &format!("http://127.0.0.1:8080/api?mode=queue&output=json{suffix}"),
                ])
                .output()
                .unwrap();
            assert!(output.status.success());
            let response = String::from_utf8_lossy(&output.stdout);
            assert!(
                response.ends_with("401") || response.ends_with("403"),
                "unkeyed API was not rejected"
            );
        }
        let browser = Command::new("podman")
            .args([
                "exec",
                &container.0,
                "curl",
                "--silent",
                "--max-time",
                "5",
                "--write-out",
                "\n%{http_code}",
                "--header",
                "Host: sabnzbd.fixture.invalid",
                "--header",
                "X-Forwarded-For: 100.100.1.2",
                "http://127.0.0.1:8080/",
            ])
            .output()
            .unwrap();
        let browser = String::from_utf8_lossy(&browser.stdout);
        assert!(
            browser.ends_with("200") || browser.ends_with("302") || browser.ends_with("303"),
            "private browser request was rejected"
        );
        let ini = std::fs::read(state.join("sabnzbd.ini")).unwrap();
        assert_eq!(retained_key(&ini).unwrap(), key);
        if attempt == 0 {
            assert!(Command::new("podman")
                .args(["restart", &container.0])
                .status()
                .unwrap()
                .success());
        }
    }
}

fn wait_until_paused(container: &str, key: &str) {
    use std::{
        process::Command,
        time::{Duration, Instant},
    };
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let output = Command::new("podman")
            .args([
                "exec",
                container,
                "curl",
                "--fail",
                "--silent",
                "--max-time",
                "5",
                &format!("http://127.0.0.1:8080/api?mode=queue&output=json&apikey={key}"),
            ])
            .output()
            .unwrap();
        if output.status.success() {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
                if value["queue"]["paused"].as_bool() == Some(true) {
                    break;
                }
            }
        }
        assert!(
            Instant::now() < deadline,
            "native SABnzbd did not become paused and API-ready"
        );
        std::thread::sleep(Duration::from_secs(1));
    }
}

#[test]
fn reapply_preserves_native_settings_and_refuses_key_replacement() {
    use skillet_core::{
        files::FileMutationResource as _,
        system::AccountResource as _,
        test_utils::{MockFiles, MockSystem},
    };
    let system = MockSystem::new();
    system
        .ensure_user("fixture", Some(2001), Some(3001))
        .unwrap();
    system.ensure_group("fixture", Some(3001)).unwrap();
    let files = MockFiles::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    let config = Config {
        state: Path::new("/var/lib/data/test/config"),
        downloads: Path::new("/var/lib/data/test/downloads"),
        owner: "fixture",
        group: "fixture",
        network: "media",
        ui_network: "private-sab",
        hostnames: &["sabnzbd.fixture.invalid"],
    };
    let key = "a".repeat(32);
    apply(&system, &files, &config, &key).unwrap();
    let mut changed = initial_configuration(&key, config.hostnames).unwrap();
    changed.push_str("[custom]\noperator_setting = keep\n");
    let path = config.state.join("sabnzbd.ini");
    files
        .ensure_file(&path, changed.as_bytes(), None, &Ownership::default())
        .unwrap();
    apply(&system, &files, &config, &key).unwrap();
    assert_eq!(files.read_file(&path).unwrap().unwrap(), changed.as_bytes());
    assert!(apply(&system, &files, &config, &"b".repeat(32)).is_err());
    assert_eq!(files.read_file(&path).unwrap().unwrap(), changed.as_bytes());
}
