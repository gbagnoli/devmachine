use super::*;
use skillet_core::system::AccountResource as _;
pub const CONFIG: Config = Config {
    download_backends: &[DownloadBackend::Putioarr],
    config_root: "/var/lib/data/media/config",
    files_path: "/var/lib/data/media/files",
    music_path: "/var/lib/data/syncthing/data/music",
    network: "fixture-media",
    data_owner: "fixture-data",
    data_group: "fixture-data",
};
pub fn credentials() -> Credentials {
    Credentials {
        version: None,
        sabnzbd_api_key: None,
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
    system
        .ensure_user("fixture-data", Some(2001), Some(3001))
        .unwrap();
    system.ensure_group("fixture-data", Some(3001)).unwrap();
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
    for app in CONFIG.apps() {
        let quadlet = String::from_utf8_lossy(
            state
                .get(&format!("/etc/containers/systemd/{}.container", app.name()))
                .unwrap(),
        );
        assert!(!quadlet.contains("PublishPort="));
        assert!(!quadlet.contains("fixture-provider-token"));
        assert!(quadlet.contains("Network=fixture-media.network"));
        if app != App::Putioarr {
            assert!(quadlet.contains("Environment=PUID=2001"));
            assert!(quadlet.contains("Environment=PGID=3001"));
        }
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
    assert!(putio.contains("User=2001:3001"));
}

#[test]
fn selected_backends_control_units_and_reject_extra_credentials() {
    use skillet_core::test_utils::{MockFiles, MockSystem};
    for backends in [
        &[DownloadBackend::Putioarr][..],
        &[DownloadBackend::Sabnzbd][..],
        &[DownloadBackend::Putioarr, DownloadBackend::Sabnzbd][..],
    ] {
        let config = Config {
            download_backends: backends,
            ..CONFIG
        };
        let system = MockSystem::new();
        system
            .ensure_user("fixture-data", Some(2001), Some(3001))
            .unwrap();
        system.ensure_group("fixture-data", Some(3001)).unwrap();
        let files = MockFiles::new();
        files.record_btrfs_mount(std::path::Path::new("/var"), "/dev/test", "/", "btrfs");
        files.record_btrfs_mount(
            std::path::Path::new("/var/lib/data"),
            "/dev/test",
            "/data",
            "btrfs",
        );
        let mut input = credentials();
        if !config.has_backend(DownloadBackend::Putioarr) {
            input.version = Some(2);
            input.putio_password.clear();
            input.putio_token.clear();
        }
        if config.has_backend(DownloadBackend::Sabnzbd) {
            input.version = Some(2);
            input.sabnzbd_api_key = Some("e".repeat(32));
        }
        let networks = config
            .apps()
            .filter(|app| *app != App::Putioarr)
            .map(|app| (app, format!("fixture-{}", app.name())))
            .collect();
        apply_with_hostnames(
            &system,
            &files,
            config,
            &networks,
            &input,
            "fixture",
            &["sabnzbd.fixture.invalid"],
        )
        .unwrap();
        let state = files.files.lock().unwrap();
        for (backend, app) in [
            (DownloadBackend::Putioarr, App::Putioarr),
            (DownloadBackend::Sabnzbd, App::Sabnzbd),
        ] {
            assert_eq!(
                state.contains_key(&format!("/etc/containers/systemd/{}.container", app.name())),
                config.has_backend(backend)
            );
            let unit = String::from_utf8_lossy(
                &state["/etc/systemd/system/skillet-media-reconcile.service"],
            );
            assert_eq!(
                unit.contains(&format!("{}.service", app.name())),
                config.has_backend(backend)
            );
        }
        if config.has_backend(DownloadBackend::Sabnzbd) {
            let unit = String::from_utf8_lossy(&state["/etc/containers/systemd/sabnzbd.container"]);
            assert!(!unit.contains("PublishPort="));
            assert!(!unit.contains("/media/music"));
            assert!(!unit.contains("/library"));
            assert!(!unit.contains(&"e".repeat(32)));
            assert!(unit.contains("Environment=PUID=2001"));
        }
    }
    let legacy = credentials();
    assert!(!serde_json::to_string(&legacy).unwrap().contains("version"));
    let mut malformed = credentials();
    malformed.sabnzbd_api_key = Some("e".repeat(32));
    assert!(malformed.validate().is_err());
}
