use super::*;
use skillet_core::{
    system::AccountResource,
    test_utils::{MockFiles, MockSystem},
};
use std::sync::atomic::Ordering;

fn config() -> Config {
    Config {
        state_path: "/var/lib/data/jellyfin/config",
        cache_path: "/var/lib/data/jellyfin/cache",
        files_path: "/var/lib/data/media/files",
        music_path: "/var/lib/data/syncthing/data/music",
        data_owner: "media",
        data_group: "media",
    }
}

#[test]
fn shared_numeric_identity_read_only_libraries_and_repeat_apply() {
    let system = MockSystem::new();
    system.ensure_group("media", Some(3001)).unwrap();
    system.ensure_user("media", Some(2001), Some(3001)).unwrap();
    let files = MockFiles::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    apply(&system, &files, config(), "fixture").unwrap();
    let state = files.files.lock().unwrap();
    let quadlet = String::from_utf8_lossy(&state["/etc/containers/systemd/jellyfin.container"]);
    assert!(quadlet.contains("User=2001:3001"));
    assert!(quadlet.contains("PublishPort=127.0.0.1:18096:8096/tcp"));
    assert!(!quadlet.contains("PublishPort=0.0.0.0"));
    assert!(!quadlet.contains("/downloads:"));
    for (source, destination) in config().libraries() {
        assert!(quadlet.contains(&format!("Volume={}:{}:ro,z", source.display(), destination)));
    }
    drop(state);
    let restarts = system.restart_count.load(Ordering::SeqCst);
    apply(&system, &files, config(), "fixture").unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), restarts);
}

#[test]
fn incompatible_storage_and_missing_accounts_fail_before_container_mutation() {
    let mut value = config();
    value.state_path = value.files_path;
    assert!(value.validate().is_err());
    value = config();
    value.cache_path = "/etc/cache";
    assert!(value.validate().is_err());
    assert!(apply(&MockSystem::new(), &MockFiles::new(), config(), "fixture").is_err());
}

#[test]
fn initial_proxy_configuration_is_dual_stack_and_preserves_application_edits() {
    use skillet_core::files::{FileMutationResource, FileReadResource};
    let system = MockSystem::new();
    system.ensure_group("media", Some(3001)).unwrap();
    system.ensure_user("media", Some(2001), Some(3001)).unwrap();
    let files = MockFiles::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    apply_with_proxies(
        &system,
        &files,
        config(),
        "fixture",
        &["172.27.27.0/24", "fd00:1234::/64"],
    )
    .unwrap();
    let path = Path::new("/var/lib/data/jellyfin/config/config/network.xml");
    let bytes = files.read_file(path).unwrap().unwrap();
    let xml = String::from_utf8(bytes).unwrap();
    assert!(xml.contains("<KnownProxies><string>172.27.27.0/24</string><string>fd00:1234::/64</string></KnownProxies>"));
    assert!(xml.contains("<EnableUPnP>false</EnableUPnP>"));
    assert!(!xml.contains("<RequireHttps>true"));
    let edited = b"operator-owned network configuration";
    files
        .ensure_file(path, edited, Some(0o640), &Ownership::default())
        .unwrap();
    apply_with_proxies(&system, &files, config(), "fixture", &["172.27.27.0/24"]).unwrap();
    assert_eq!(files.read_file(path).unwrap().unwrap(), edited);
    assert!(apply_with_proxies(&system, &files, config(), "fixture", &["0.0.0.0/0"]).is_err());
}

#[test]
fn public_readiness_requires_native_completed_wizard_and_rejects_bad_xml() {
    use skillet_core::files::FileMutationResource;
    let files = MockFiles::new();
    let path = Path::new("/var/lib/data/jellyfin/config/config/system.xml");
    assert!(!setup_completed(&files, config()).unwrap());
    for (xml, expected) in [
        ("<ServerConfiguration/>", false),
        ("<ServerConfiguration><IsStartupWizardCompleted>false</IsStartupWizardCompleted></ServerConfiguration>", false),
        ("<ServerConfiguration><IsStartupWizardCompleted>true</IsStartupWizardCompleted></ServerConfiguration>", true),
    ] {
        files.ensure_file(path, xml.as_bytes(), None, &Ownership::default()).unwrap();
        assert_eq!(setup_completed(&files, config()).unwrap(), expected);
    }
    files
        .ensure_file(path, b"<wrong/>", None, &Ownership::default())
        .unwrap();
    assert!(setup_completed(&files, config()).is_err());
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
#[ignore = "requires Podman and the official Jellyfin image"]
fn real_jellyfin_consumes_initial_configuration_and_keeps_native_setup_private() {
    use std::{
        process::Command,
        time::{Duration, Instant},
    };
    let root = tempfile::tempdir().unwrap();
    let state = root.path().join("state");
    std::fs::create_dir(&state).unwrap();
    let mut config = config();
    config.state_path = Box::leak(state.to_string_lossy().into_owned().into_boxed_str());
    initialize_network(
        &skillet_core::files::LocalFileResource::new(),
        config,
        "fixture",
        &["172.27.27.0/24", "fd00:1234::/64"],
        &Ownership::default(),
    )
    .unwrap();
    let container = NativeContainer(format!(
        "skillet-jellyfin-init-{}-{}",
        std::process::id(),
        root.path().file_name().unwrap().to_string_lossy()
    ));
    let output = Command::new("podman")
        .args([
            "run",
            "--detach",
            "--name",
            &container.0,
            "--network",
            "none",
            "--volume",
            &format!("{}:/config:Z", state.display()),
            IMAGE,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let deadline = Instant::now() + Duration::from_secs(120);
    let info = loop {
        let output = Command::new("podman")
            .args([
                "exec",
                &container.0,
                "curl",
                "--fail",
                "--silent",
                "--max-time",
                "5",
                "http://127.0.0.1:8096/System/Info/Public",
            ])
            .output()
            .unwrap();
        if output.status.success() {
            let info: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            if info["ServerName"].is_string() {
                break info;
            }
            assert!(
                Instant::now() < deadline,
                "native info is not ready: {info}"
            );
        }
        assert!(
            Instant::now() < deadline,
            "native Jellyfin failed to become ready"
        );
        std::thread::sleep(Duration::from_secs(1));
    };
    assert_eq!(info["ServerName"], "fixture");
    assert_eq!(info["StartupWizardCompleted"], false);
    assert!(!setup_completed(&skillet_core::files::LocalFileResource::new(), config).unwrap());
    let network = std::fs::read_to_string(state.join("config/network.xml")).unwrap();
    assert!(network.contains("172.27.27.0/24"));
    assert!(network.contains("fd00:1234::/64"));
}
