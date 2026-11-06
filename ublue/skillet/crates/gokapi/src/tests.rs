use super::*;
use skillet_core::{
    system::AccountResource,
    test_utils::{MockFiles, MockSystem},
};
#[test]
fn definition_uses_pinned_image_private_network_and_reuses_data_identity() {
    let system = MockSystem::new();
    system.ensure_group("data", Some(3001)).unwrap();
    system.ensure_user("data", Some(2001), Some(3001)).unwrap();
    let files = MockFiles::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    files.record_btrfs_subvolume(Path::new("/var/lib/data/syncthing"));
    let config = Config {
        state_path: "/var/lib/data/gokapi",
        backup_path: "/var/lib/data/syncthing/data/backups/gokapi",
        backup_subvolume: "/var/lib/data/syncthing",
        owner: "data",
        group: "data",
    };
    let settings = Settings {
        public_url: "https://send.example.invalid/".into(),
        admin: "admin@example.invalid".into(),
        users: vec!["admin@example.invalid".into()],
    };
    apply(&system, &files, config, "fixture-gokapi", &settings).unwrap();
    let before = system
        .restart_count
        .load(std::sync::atomic::Ordering::SeqCst);
    apply(&system, &files, config, "fixture-gokapi", &settings).unwrap();
    assert_eq!(
        before,
        system
            .restart_count
            .load(std::sync::atomic::Ordering::SeqCst)
    );
    let state = files.files.lock().unwrap();
    let unit = String::from_utf8_lossy(&state["/etc/containers/systemd/gokapi.container"]);
    for expected in [
        IMAGE,
        "Network=fixture-gokapi.network",
        "User=2001:3001",
        "EnvironmentFile=/run/skillet/gokapi/proxy.env",
        "Requires=skillet-gokapi-initialize.service",
    ] {
        assert!(unit.contains(expected), "{expected}");
    }
    assert!(!unit.contains("PublishPort="));
    assert!(!unit.contains("AutoUpdate="));
    assert!(!unit.contains(&settings.admin));
}
