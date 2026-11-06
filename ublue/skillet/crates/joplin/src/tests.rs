use super::{apply, Config, DEFAULT_IMAGE};
use skillet_core::{
    system::AccountResource,
    test_utils::{MockFiles, MockSystem},
};
use std::path::Path;

#[test]
fn joplin_uses_private_postgres_and_does_not_mount_a_host_timezone_file() {
    let system = MockSystem::new();
    system
        .ensure_user("giacomo", Some(1000), Some(1000))
        .unwrap();
    let files = MockFiles::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    files.record_btrfs_subvolume(Path::new("/var/lib/data/syncthing"));
    let config = Config {
        image: DEFAULT_IMAGE,
        network_name: "donbot-joplin",
        database_path: "/var/lib/data/joplin/postgres",
        postgres_data_container_path: "/var/lib/postgresql",
        backup_path: "/var/lib/data/syncthing/data/backups/postgres/joplin",
        backup_subvolume: "/var/lib/data/syncthing",
        backup_owner: "giacomo",
        backup_group: "giacomo",
    };

    apply(
        &system,
        &files,
        "donbot",
        config,
        "https://joplin.example.invalid/",
        "fixture password",
    )
    .unwrap();

    let quadlets = files.files.lock().unwrap();
    let postgres = String::from_utf8_lossy(
        quadlets
            .get("/etc/containers/systemd/postgresql-joplin.container")
            .unwrap(),
    );
    let joplin = String::from_utf8_lossy(
        quadlets
            .get("/etc/containers/systemd/joplin-server.container")
            .unwrap(),
    );
    assert!(postgres.contains("ContainerName=postgresql-joplin"));
    assert!(postgres.contains("Network=donbot-joplin"));
    assert!(!postgres.contains("PublishPort="));
    assert!(joplin.contains("ContainerName=joplin-server"));
    assert!(joplin.contains("APP_BASE_URL=https://joplin.example.invalid/"));
    assert!(joplin.contains("TimeoutStartSec=10min"));
    assert!(joplin.contains("Network=donbot-joplin"));
    assert!(!joplin.contains("/etc/localtime"));
}
