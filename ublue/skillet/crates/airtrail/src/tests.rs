use super::*;
use skillet_core::{
    system::AccountResource,
    test_utils::{MockFiles, MockSystem},
};
use std::path::Path;

fn config() -> Config {
    Config {
        image: DEFAULT_IMAGE,
        network_name: "donbot-airtrail",
        database_path: "/var/lib/data/airtrail/postgres",
        postgres_data_container_path: "/var/lib/postgresql",
        uploads_path: "/var/lib/data/airtrail/uploads",
        backup_path: "/var/lib/data/syncthing/data/backups/airtrail",
        backup_subvolume: "/var/lib/data/syncthing",
        backup_owner: "giacomo",
        backup_group: "giacomo",
    }
}

#[test]
fn config_requires_https_and_syncthing_backup_destination() {
    assert!(config()
        .validate("https://airtrail.example.invalid")
        .is_ok());
    assert!(config()
        .validate("http://airtrail.example.invalid")
        .is_err());
    let invalid = Config {
        backup_path: "/var/lib/data/backups/airtrail",
        ..config()
    };
    assert!(invalid
        .validate("https://airtrail.example.invalid")
        .is_err());
}

#[test]
fn optional_integrations_accept_absence_but_reject_empty_or_whitespace_values() {
    assert_eq!(Integrations::parse("{}").unwrap(), Integrations::default());
    assert!(Integrations::parse(r#"{"aerodatabox":"key"}"#).is_ok());
    assert!(Integrations::parse(r#"{"openaip":""}"#).is_err());
    assert!(Integrations::parse(r#"{"openaip":"bad key"}"#).is_err());
}

#[test]
fn quadlet_uses_private_secret_environment_and_mounts_uploads() {
    let system = MockSystem::new();
    system
        .ensure_user("giacomo", Some(1000), Some(1000))
        .unwrap();
    system.ensure_group("giacomo", Some(1000)).unwrap();
    let files = MockFiles::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    files.record_btrfs_subvolume(Path::new("/var/lib/data/syncthing"));
    let database_password = "a".repeat(32);
    let integrations = Integrations {
        aerodatabox: Some("provider-key".into()),
        openaip: None,
    };

    apply(
        &system,
        &files,
        "donbot",
        config(),
        "https://airtrail.example.invalid",
        &database_password,
        &integrations,
    )
    .unwrap();

    let units = files.files.lock().unwrap();
    let app = String::from_utf8_lossy(
        units
            .get("/etc/containers/systemd/airtrail.container")
            .unwrap(),
    );
    assert!(app.contains("Environment=ORIGIN=https://airtrail.example.invalid"));
    assert!(app.contains("Environment=ADDRESS_HEADER=X-Forwarded-For"));
    assert!(app.contains("Secret=airtrail_database_url,type=env,target=DB_URL"));
    assert!(app.contains(
        "Secret=airtrail_aerodatabox_key,type=env,target=INTEGRATIONS_AERO_DATA_BOX_KEY"
    ));
    assert!(!app.contains("provider-key"));
    assert!(!app.contains(&database_password));
    assert!(app.contains("Volume=/var/lib/data/airtrail/uploads:/app/uploads:z"));
    assert!(units.contains_key("/etc/systemd/system/skillet-airtrail-backup.timer"));
}
