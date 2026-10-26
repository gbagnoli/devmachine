use super::*;
pub const CONFIG: Config = Config {
    config_root: "/var/lib/data/media/config",
    files_path: "/var/lib/data/media/files",
    music_path: "/var/lib/data/syncthing/data/music",
    network: "fixture-media",
    uid: 1000,
    gid: 1000,
};
pub fn credentials() -> Credentials {
    Credentials {
        api_keys: App::ARR
            .into_iter()
            .enumerate()
            .map(|(n, app)| (app, format!("{n:032x}")))
            .collect(),
        putio_password: "p".repeat(64),
        putio_token: "fixture-provider-token".into(),
    }
}

#[test]
fn malformed_credentials_do_not_retain_values_in_errors() {
    let mut credentials = credentials();
    credentials
        .api_keys
        .insert(App::Lidarr, "private-value".into());
    let result = Credentials::parse(&serde_json::to_string(&credentials).unwrap());
    assert!(!result.err().unwrap().to_string().contains("private-value"));
}

#[test]
fn containers_are_private_share_download_paths_and_hide_secrets() {
    use skillet_core::test_utils::{MockFiles, MockSystem};
    let system = MockSystem::new();
    let files = MockFiles::new();
    files.record_btrfs_mount(std::path::Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(
        std::path::Path::new("/var/lib/data"),
        "/dev/test",
        "/data",
        "btrfs",
    );
    let networks = App::ARR
        .into_iter()
        .map(|app| (app, format!("fixture-{}", app.name())))
        .collect();
    apply(
        &system,
        &files,
        CONFIG,
        &networks,
        &credentials(),
        "fixture",
    )
    .unwrap();
    let state = files.files.lock().unwrap();
    for app in App::ALL {
        let quadlet = String::from_utf8_lossy(
            state
                .get(&format!("/etc/containers/systemd/{}.container", app.name()))
                .unwrap(),
        );
        assert!(!quadlet.contains("PublishPort="));
        assert!(!quadlet.contains("fixture-provider-token"));
        assert!(quadlet.contains("Network=fixture-media.network"));
        if app != App::Prowlarr {
            assert!(quadlet.contains("/var/lib/data/media/files:/data:z"));
        }
    }
    let putio = String::from_utf8_lossy(
        state
            .get("/etc/containers/systemd/putioarr.container")
            .unwrap(),
    );
    assert!(putio.contains("Entrypoint=/usr/bin/putioarr"));
    assert!(putio.contains("Secret=media_putioarr_config"));
}
