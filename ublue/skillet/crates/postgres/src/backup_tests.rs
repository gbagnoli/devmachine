use super::*;

fn config() -> Config {
    Config {
        instance: "joplin",
        database: "joplin",
        username: "joplin",
        container: "postgresql-joplin",
        application_unit: "joplin-server.service",
        backup_path: "/var/lib/data/syncthing/data/backups/postgres/joplin",
        backup_subvolume: "/var/lib/data/syncthing",
        owner: "giacomo",
        group: "giacomo",
        keep: 7,
    }
}

#[test]
fn backup_config_requires_a_safe_subdirectory_and_bounded_retention() {
    assert!(config().validate().is_ok());

    let mut outside = config();
    outside.backup_path = "/var/lib/data/postgres";
    assert!(outside.validate().is_err());

    let mut escaped = config();
    escaped.backup_path = "/var/lib/data/syncthing/../postgres";
    assert!(escaped.validate().is_err());

    let mut unbounded = config();
    unbounded.keep = KEEP_ARCHIVES + 1;
    assert!(unbounded.validate().is_err());
}

#[test]
fn backup_installation_creates_a_daily_instance_owned_timer() {
    use skillet_core::{
        system::{GroupIdentity, UserIdentity},
        test_utils::{MockFiles, MockSystem},
    };

    let system = MockSystem::new();
    system.user_identities.lock().unwrap().insert(
        "giacomo".into(),
        UserIdentity {
            name: "giacomo".into(),
            uid: 1000,
            primary_gid: 1000,
        },
    );
    system.group_identities.lock().unwrap().insert(
        "giacomo".into(),
        GroupIdentity {
            name: "giacomo".into(),
            gid: 1000,
        },
    );
    let files = MockFiles::new();
    install(&system, &files, "donbot", &config()).unwrap();
    let files = files.files.lock().unwrap();
    let service = String::from_utf8(
        files["/etc/systemd/system/skillet-postgres-joplin-backup.service"].clone(),
    )
    .unwrap();
    let timer = String::from_utf8(
        files["/etc/systemd/system/skillet-postgres-joplin-backup.timer"].clone(),
    )
    .unwrap();
    assert!(service.contains("skillet-donbot postgres backup --instance joplin"));
    assert!(service.contains("RequiresMountsFor=/var/lib/data"));
    assert!(timer.contains("OnCalendar=daily"));
    assert!(timer.contains("RandomizedDelaySec=4h"));
    assert!(timer.contains("Persistent=true"));
    assert!(system
        .services
        .lock()
        .unwrap()
        .contains_key("skillet-postgres-joplin-backup.timer"));
}
