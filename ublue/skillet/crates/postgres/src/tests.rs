use super::{apply, Config, DEFAULT_IMAGE};
use skillet_core::test_utils::{MockFiles, MockSystem};
use std::path::Path;

fn joplin() -> Config {
    Config {
        instance: "joplin",
        database: "joplin",
        username: "joplin",
        data_path: "/var/lib/data/joplin/postgres",
        network_name: "donbot-joplin",
        image: DEFAULT_IMAGE,
        data_container_path: "/var/lib/postgresql",
    }
}

#[test]
fn host_service_network_names_accept_hyphens() {
    assert!(joplin().validate().is_ok());
}

#[test]
fn creates_parent_directory_before_application_database_subvolume() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");

    apply(&system, &files, joplin(), "fixture password").unwrap();

    assert!(files
        .directories
        .lock()
        .unwrap()
        .contains("/var/lib/data/joplin"));
    assert!(files
        .btrfs_subvolumes
        .lock()
        .unwrap()
        .contains("/var/lib/data/joplin/postgres"));
}

#[test]
fn database_storage_must_remain_inside_data_mount() {
    let mut config = joplin();
    config.data_path = "/var/lib/postgresql";
    assert!(config.validate().is_err());
}
