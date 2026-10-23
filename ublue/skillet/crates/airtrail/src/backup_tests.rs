use super::*;

#[test]
fn backup_paths_stay_under_the_selected_syncthing_subvolume() {
    let config = Config {
        image: crate::DEFAULT_IMAGE,
        network_name: "donbot-airtrail",
        database_path: "/var/lib/data/airtrail/postgres",
        postgres_data_container_path: "/var/lib/postgresql",
        uploads_path: "/var/lib/data/airtrail/uploads",
        backup_path: "/var/lib/data/syncthing/data/backups/airtrail",
        backup_subvolume: "/var/lib/data/syncthing",
        backup_owner: "giacomo",
        backup_group: "giacomo",
    };
    assert!(validate(&config).is_ok());
    let invalid = Config {
        backup_path: "/var/lib/data/backups/airtrail",
        ..config
    };
    assert!(validate(&invalid).is_err());
}
