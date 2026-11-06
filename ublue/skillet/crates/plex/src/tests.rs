use super::*;
use skillet_core::{
    system::AccountResource,
    test_utils::{MockFiles, MockSystem},
};
use std::sync::atomic::Ordering;

fn config() -> Config {
    Config {
        state_path: "/var/lib/data/plex/config",
        transcode_path: "/var/lib/data/plex/transcode",
        files_path: "/var/lib/data/media/files",
        music_path: "/var/lib/data/syncthing/data/music",
        data_owner: "media",
        data_group: "media",
    }
}

#[test]
fn plex_advertises_explicit_https_port_preserves_identity_and_reads_arr_libraries() {
    let system = MockSystem::new();
    system.ensure_group("media", Some(3001)).unwrap();
    system.ensure_user("media", Some(2001), Some(3001)).unwrap();
    let files = MockFiles::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    apply(
        &system,
        &files,
        config(),
        "fixture",
        "https://plex.example.invalid/",
    )
    .unwrap();
    let state = files.files.lock().unwrap();
    let unit = String::from_utf8_lossy(&state["/etc/containers/systemd/plex.container"]);
    for directive in [
        "Environment=PLEX_UID=2001",
        "Environment=PLEX_GID=3001",
        "Environment=ADVERTISE_IP=https://plex.example.invalid:443",
        "Environment=CHANGE_CONFIG_DIR_OWNERSHIP=false",
        "PublishPort=127.0.0.1:32400:32400/tcp",
        "PublishPort=[::1]:32400:32400/tcp",
    ] {
        assert!(unit.contains(directive), "missing {directive}");
    }
    assert!(!unit.contains("ALLOWED_NETWORKS"));
    assert!(!unit.contains("0.0.0.0"));
    assert!(!unit.contains("PLEX_CLAIM"));
    for (source, target) in config().libraries() {
        assert!(unit.contains(&format!("{}:{target}:ro,z", source.display())));
    }
    drop(state);
    let before = system.restart_count.load(Ordering::SeqCst);
    apply(
        &system,
        &files,
        config(),
        "fixture",
        "https://plex.example.invalid/",
    )
    .unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), before);
}

#[test]
fn advertised_origin_rejects_credentials_paths_and_insecure_schemes() {
    for value in [
        "http://plex.example.invalid",
        "https://user:password@plex.example.invalid/",
        "https://plex.example.invalid/web",
        "https://plex.example.invalid:32400/",
        "https://plex.example.invalid/?token=private",
    ] {
        assert!(advertised_url(value).is_err());
    }
    assert_eq!(
        advertised_url("https://plex.example.invalid/").unwrap(),
        "https://plex.example.invalid:443"
    );
}

#[test]
fn public_readiness_requires_a_native_claim_and_never_prints_the_account_token() {
    let files = MockFiles::new();
    assert!(!claimed(&files, config()).unwrap());
    let path =
        "/var/lib/data/plex/config/Library/Application Support/Plex Media Server/Preferences.xml";
    files
        .files
        .lock()
        .unwrap()
        .insert(path.into(), b"<Preferences/>".to_vec());
    assert!(!claimed(&files, config()).unwrap());
    files.files.lock().unwrap().insert(
        path.into(),
        b"<Preferences PlexOnlineToken=\"synthetic-account-token\"/>".to_vec(),
    );
    assert!(claimed(&files, config()).unwrap());
    files
        .files
        .lock()
        .unwrap()
        .insert(path.into(), b"not XML synthetic-account-token".to_vec());
    let error = claimed(&files, config()).unwrap_err().to_string();
    assert!(!error.contains("synthetic-account-token"));
}
