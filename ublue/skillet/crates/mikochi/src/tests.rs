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
        String::from_utf8_lossy(&definitions["/etc/containers/systemd/mikochi.container"]);
    for directive in [
        "Image=docker.io/zer0tonin/mikochi:latest",
        "AutoUpdate=registry",
        "Network=example-files.network",
        "Environment=NO_AUTH=true",
        "User=1000:1000",
        "Volume=/var/lib/data/syncthing/data:/data:z",
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
        files.files.lock().unwrap()["/etc/containers/systemd/mikochi.container"]
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

#[test]
#[ignore = "requires Podman and the Mikochi image"]
fn real_mikochi_has_no_second_login_and_supports_owned_file_operations() {
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    std::fs::create_dir(&data).unwrap();
    let payload = vec![0x61; 3 * 1024 * 1024];
    std::fs::write(data.join("source file.txt"), &payload).unwrap();
    // A synchronized link must not expose a sibling host file outside the mount.
    let outside = temp.path().join("private-host-file");
    std::fs::write(&outside, b"outside-host-scope").unwrap();
    std::os::unix::fs::symlink(&outside, data.join("external-link")).unwrap();
    let (_container, client, base) = start_trial(temp.path(), &data);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if client
            .get(format!("{base}/ready"))
            .send()
            .is_ok_and(|response| response.status().is_success())
        {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "Mikochi did not become ready"
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    assert!(client.get(&base).send().unwrap().status().is_success());
    let listed = client.get(format!("{base}/api/browse/")).send().unwrap();
    assert!(listed.status().is_success());
    assert!(listed.text().unwrap().contains("source file.txt"));
    let download = client
        .get(format!("{base}/api/stream/source%20file.txt"))
        .send()
        .unwrap();
    assert!(download.status().is_success());
    assert_eq!(download.bytes().unwrap().as_ref(), payload.as_slice());
    let linked = client
        .get(format!("{base}/api/stream/external-link"))
        .send()
        .unwrap();
    assert!(!linked.status().is_success());
    assert!(!linked.text().unwrap().contains("outside-host-scope"));
    assert!(client
        .put(format!("{base}/api/mkdir/folder"))
        .send()
        .unwrap()
        .status()
        .is_success());
    let form = reqwest::blocking::multipart::Form::new().part(
        "file",
        reqwest::blocking::multipart::Part::bytes(payload.clone()).file_name("uploaded.txt"),
    );
    assert!(client
        .put(format!("{base}/api/upload/folder/uploaded.txt"))
        .multipart(form)
        .send()
        .unwrap()
        .status()
        .is_success());
    assert_eq!(
        std::fs::read(data.join("folder/uploaded.txt")).unwrap(),
        payload
    );
    assert!(client
        .put(format!("{base}/api/move/folder/uploaded.txt"))
        .json(&serde_json::json!({"newPath":"/folder/renamed.txt"}))
        .send()
        .unwrap()
        .status()
        .is_success());
    assert!(client
        .delete(format!("{base}/api/delete/folder/renamed.txt"))
        .send()
        .unwrap()
        .status()
        .is_success());
    assert!(!data.join("folder/renamed.txt").exists());
    // User namespace mapping must preserve workstation ownership on written files.
    assert_eq!(
        std::fs::metadata(data.join("folder")).unwrap().uid(),
        std::fs::metadata(&data).unwrap().uid()
    );
}

fn start_trial(
    temp: &std::path::Path,
    data: &std::path::Path,
) -> (TrialContainer, reqwest::blocking::Client, String) {
    let name = format!(
        "skillet-mikochi-trial-{}-{}",
        std::process::id(),
        temp.file_name().unwrap().to_string_lossy()
    );
    let volume = format!("{}:/data:z", data.display());
    let container = TrialContainer(name);
    let run = std::process::Command::new("podman")
        .args([
            "run",
            "--detach",
            "--pull=always",
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
            "--volume",
            &volume,
            "--env",
            "HOST=:8080",
            "--env",
            "NO_AUTH=true",
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
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap();
    (container, client, base)
}
