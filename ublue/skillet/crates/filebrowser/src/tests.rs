use super::*;
use skillet_core::{
    system::{GroupIdentity, UserIdentity},
    test_utils::{MockFiles, MockSystem},
};
use std::os::unix::fs::MetadataExt as _;

#[test]
fn private_scope_and_namespace_match_syncthing_without_publishing_a_port() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    system.user_identities.lock().unwrap().insert(
        "owner".into(),
        UserIdentity {
            name: "owner".into(),
            uid: 1042,
            primary_gid: 2047,
        },
    );
    system.group_identities.lock().unwrap().insert(
        "owner".into(),
        GroupIdentity {
            name: "owner".into(),
            gid: 2047,
        },
    );
    files
        .files
        .lock()
        .unwrap()
        .insert("/etc/subuid".into(), b"owner:100000:65536\n".to_vec());
    files
        .files
        .lock()
        .unwrap()
        .insert("/etc/subgid".into(), b"owner:200000:65536\n".to_vec());
    let config = Config {
        files_path: "/var/lib/data/syncthing/data",
        owner: "owner",
        group: "owner",
        container_uid: 1000,
        container_gid: 1000,
        private_network: "example-files",
    };
    apply(&system, &files, &config).unwrap();
    let definitions = files.files.lock().unwrap();
    let quadlet =
        String::from_utf8_lossy(&definitions["/etc/containers/systemd/filebrowser.container"]);
    for directive in [
        "Image=ghcr.io/gtsteffaniak/filebrowser:2.0.2-stable",
        "Network=example-files.network",
        "Environment=FILEBROWSER_CONFIG=/etc/filebrowser/config.yaml",
        "User=1000:1000",
        "Volume=/var/lib/data/syncthing/data:/files:z",
        "WantedBy=multi-user.target default.target",
    ] {
        assert!(quadlet.contains(directive), "missing {directive}");
    }
    assert!(!quadlet.contains("PublishPort"));
    assert!(!quadlet.contains("config:/"));
    let initial = quadlet.to_string();
    drop(definitions);
    apply(&system, &files, &config).unwrap();
    assert_eq!(
        initial.as_bytes(),
        files.files.lock().unwrap()["/etc/containers/systemd/filebrowser.container"]
    );
}

struct TrialContainer(String);
impl Drop for TrialContainer {
    fn drop(&mut self) {
        let _ = std::process::Command::new("podman")
            .args(["rm", "--force", &self.0])
            .output();
    }
}
#[track_caller]
fn successful(response: reqwest::blocking::Response) -> reqwest::blocking::Response {
    response.error_for_status().unwrap()
}

#[test]
#[ignore = "requires Podman and the pinned Quantum image"]
fn real_quantum_proxy_auth_supports_owned_file_operations() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("files");
    let state = temp.path().join("state");
    std::fs::create_dir(&data).unwrap();
    std::fs::create_dir(&state).unwrap();
    let config = temp.path().join("config.yaml");
    std::fs::write(&config, CONFIG).unwrap();
    let payload = vec![0x61; 3 * 1024 * 1024];
    std::fs::write(data.join("source file.txt"), &payload).unwrap();
    std::fs::write(
        data.join("sample.pdf"),
        include_bytes!("../tests/sample-multipage.pdf"),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        "/home/filebrowser/data/database.sqlite",
        data.join("outside-link"),
    )
    .unwrap();
    let (_container, client, base) = start_trial(temp.path(), &data, &state, &config);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !client
        .get(format!("{base}/health"))
        .send()
        .is_ok_and(|r| r.status().is_success())
    {
        assert!(
            std::time::Instant::now() < deadline,
            "Quantum did not become ready"
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    check_proxy_access(&client, &base, &payload);
    let resource = format!("{base}/api/resources");
    successful(
        client
            .post(&resource)
            .header("X-Skillet-User", "reader@example.invalid")
            .query(&[("source", "Files"), ("path", "/folder"), ("isDir", "true")])
            .send()
            .unwrap(),
    );
    successful(
        client
            .post(&resource)
            .header("X-Skillet-User", "reader@example.invalid")
            .query(&[("source", "Files"), ("path", "/folder/uploaded.txt")])
            .body(payload.clone())
            .send()
            .unwrap(),
    );
    assert_eq!(
        std::fs::read(data.join("folder/uploaded.txt")).unwrap(),
        payload
    );
    successful(client.patch(&resource).header("X-Skillet-User", "reader@example.invalid").json(&serde_json::json!({"action":"rename","items":[{"fromSource":"Files","fromPath":"/folder/uploaded.txt","toSource":"Files","toPath":"/folder/renamed.txt"}]})).send().unwrap());
    assert!(data.join("folder/renamed.txt").exists());
    successful(
        client
            .delete(&resource)
            .header("X-Skillet-User", "reader@example.invalid")
            .query(&[("source", "Files"), ("path", "/folder/renamed.txt")])
            .send()
            .unwrap(),
    );
    assert!(!data.join("folder/renamed.txt").exists());
    assert_eq!(
        std::fs::metadata(data.join("folder")).unwrap().uid(),
        std::fs::metadata(&data).unwrap().uid()
    );
    // State persists independently of the synchronized source files.
    assert!(state.join("database.sqlite").is_file());
}

fn start_trial(
    temp: &std::path::Path,
    data: &std::path::Path,
    state: &std::path::Path,
    config: &std::path::Path,
) -> (TrialContainer, reqwest::blocking::Client, String) {
    let container = TrialContainer(format!(
        "skillet-quantum-{}-{}",
        std::process::id(),
        temp.file_name().unwrap().to_string_lossy()
    ));
    let run = std::process::Command::new("podman")
        .args([
            "run",
            "--detach",
            "--pull=missing",
            "--name",
            &container.0,
            "--userns",
            "keep-id:uid=1000,gid=1000",
            "--user",
            "1000:1000",
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--publish",
            "127.0.0.1::8080",
            "--env",
            "FILEBROWSER_CONFIG=/etc/filebrowser.yaml",
            "--volume",
            &format!("{}:/files:z", data.display()),
            "--volume",
            &format!("{}:/home/filebrowser/data:Z", state.display()),
            "--volume",
            &format!("{}:/etc/filebrowser.yaml:ro,z", config.display()),
            IMAGE,
        ])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let port = std::process::Command::new("podman")
        .args(["port", &container.0, "8080/tcp"])
        .output()
        .unwrap();
    assert!(port.status.success());
    let address: std::net::SocketAddr = String::from_utf8(port.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(address.ip().is_loopback());
    let base = format!("http://{address}");
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap();
    (container, client, base)
}

fn check_proxy_access(client: &reqwest::blocking::Client, base: &str, payload: &[u8]) {
    // Native proxy login creates an ordinary account, with no separate app password.
    let login = successful(
        client
            .post(format!("{base}/api/auth/login"))
            .header("X-Skillet-User", "reader@example.invalid")
            .send()
            .unwrap(),
    );
    assert!(login.headers().contains_key("set-cookie"));
    let resource = format!("{base}/api/resources");
    assert!(!client
        .get(&resource)
        .query(&[("source", "Files"), ("path", "/")])
        .send()
        .unwrap()
        .status()
        .is_success());
    let listed = successful(
        client
            .get(&resource)
            .header("X-Skillet-User", "reader@example.invalid")
            .query(&[("source", "Files"), ("path", "/")])
            .send()
            .unwrap(),
    );
    assert!(listed.text().unwrap().contains("source file.txt"));
    let download = format!("{resource}/download");
    let reply = successful(
        client
            .get(&download)
            .header("X-Skillet-User", "reader@example.invalid")
            .query(&[("source", "Files"), ("file", "/source file.txt")])
            .send()
            .unwrap(),
    );
    assert_eq!(reply.bytes().unwrap().as_ref(), payload);
    let pdf = successful(
        client
            .get(&download)
            .header("X-Skillet-User", "reader@example.invalid")
            .header("Range", "bytes=0-127")
            .query(&[
                ("source", "Files"),
                ("file", "/sample.pdf"),
                ("inline", "true"),
            ])
            .send()
            .unwrap(),
    );
    assert_eq!(pdf.status(), reqwest::StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        &pdf.bytes().unwrap()[..],
        &include_bytes!("../tests/sample-multipage.pdf")[..128]
    );
    let escaped = client
        .get(&download)
        .header("X-Skillet-User", "reader@example.invalid")
        .query(&[("source", "Files"), ("file", "/outside-link")])
        .send()
        .unwrap();
    assert!(
        !escaped.status().is_success(),
        "source symlink escaped into private app state"
    );
}
