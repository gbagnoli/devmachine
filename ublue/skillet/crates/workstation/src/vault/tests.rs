use super::{
    cache_clear, cache_read, cache_store, database_path_from, lookup, Vault, VaultError,
    CACHE_LIFETIME,
};
use keepass::{Database, DatabaseKey};

#[test]
fn entry_inventory_returns_only_paths_without_password_fields() {
    let mut database = Database::new();
    super::create_entry(
        &mut database,
        "skillet/environments/test/datadog/api-key",
        "private-do-not-display",
    )
    .unwrap();
    super::create_entry(
        &mut database,
        "personal/account/password",
        "another-private-value",
    )
    .unwrap();
    let vault = Vault {
        path: std::path::PathBuf::new(),
        original: Vec::new(),
        database,
        password: "unlock-value".into(),
        password_cached: false,
    };
    assert_eq!(
        vault.entry_paths(),
        [
            "personal/account/password",
            "skillet/environments/test/datadog/api-key"
        ]
    );
}

#[test]
fn exact_entry_lookup_preserves_whitespace_and_reports_absence() {
    let mut database = Database::new();
    let mut root = database.root_mut();
    let mut skillet = root.add_group();
    skillet.name = "skillet".to_string();
    let mut hosts = skillet.add_group();
    hosts.name = "hosts".to_string();
    let mut clamps = hosts.add_group();
    clamps.name = "clamps".to_string();
    let mut pihole = clamps.add_group();
    pihole.name = "pihole".to_string();
    let mut entry = pihole.add_entry();
    entry.set_unprotected("Title", "web-password");
    entry.set_protected("Password", " value\n");
    assert_eq!(
        lookup(&database, "skillet/hosts/clamps/pihole/web-password")
            .unwrap()
            .as_deref(),
        Some(" value\n")
    );
    assert_eq!(
        lookup(&database, "skillet/hosts/other/pihole/web-password").unwrap(),
        None
    );
}

#[test]
fn xdg_vault_path_uses_data_home_and_absolute_home_fallback() {
    let xdg = std::path::Path::new("/tmp/xdg-data");
    let home = std::path::Path::new("/tmp/user-home");
    assert_eq!(
        database_path_from(Some(xdg), Some(home)).unwrap(),
        xdg.join("skillet/secrets.kdbx")
    );
    assert_eq!(
        database_path_from(Some(std::path::Path::new("relative")), Some(home)).unwrap(),
        home.join(".local/share/skillet/secrets.kdbx")
    );
    assert!(matches!(
        database_path_from(None, None),
        Err(VaultError::Invalid(_))
    ));
}

#[test]
fn kdbx_round_trip_requires_the_correct_password() {
    let mut database = Database::new();
    let mut root = database.root_mut();
    let mut group = root.add_group();
    group.name = "skillet".to_string();
    let mut entry = group.add_entry();
    entry.set_unprotected("Title", "fixture");
    entry.set_protected("Password", "dummy only");
    let mut encrypted = Vec::new();
    database
        .save(
            &mut encrypted,
            DatabaseKey::new().with_password("fixture unlock"),
        )
        .unwrap();
    assert!(Database::open(
        &mut encrypted.as_slice(),
        DatabaseKey::new().with_password("wrong")
    )
    .is_err());
    let opened = Database::open(
        &mut encrypted.as_slice(),
        DatabaseKey::new().with_password("fixture unlock"),
    )
    .unwrap();
    assert_eq!(
        lookup(&opened, "skillet/fixture").unwrap().as_deref(),
        Some("dummy only")
    );
}

#[test]
fn verified_save_updates_symlink_target_and_keeps_recovery_copy() {
    let dir = tempfile::tempdir().unwrap();
    let database_path = dir.path().join("synced.kdbx");
    let link_path = dir.path().join("secrets.kdbx");
    let database = Database::new();
    let password = "fixture unlock";
    let mut original = Vec::new();
    database
        .save(&mut original, DatabaseKey::new().with_password(password))
        .unwrap();
    std::fs::write(&database_path, &original).unwrap();
    std::os::unix::fs::symlink(&database_path, &link_path).unwrap();
    let mut vault = Vault {
        path: std::fs::canonicalize(&link_path).unwrap(),
        original,
        database,
        password: password.to_string(),
        password_cached: false,
    };
    let entry_path = "skillet/hosts/clamps/pihole/web-password";
    vault.insert(entry_path, "generated test value").unwrap();
    vault
        .save_verified(None, entry_path, "generated test value")
        .unwrap();
    assert!(link_path.is_symlink());
    let changed = std::fs::read(&database_path).unwrap();
    let reopened = Database::open(
        &mut changed.as_slice(),
        DatabaseKey::new().with_password(password),
    )
    .unwrap();
    assert_eq!(
        lookup(&reopened, entry_path).unwrap().as_deref(),
        Some("generated test value")
    );
    assert!(std::fs::read_dir(dir.path()).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".bak")));
}

#[test]
fn verified_save_refuses_a_concurrent_vault_change() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.kdbx");
    let database = Database::new();
    let password = "fixture unlock";
    let mut original = Vec::new();
    database
        .save(&mut original, DatabaseKey::new().with_password(password))
        .unwrap();
    std::fs::write(&path, &original).unwrap();
    let mut vault = Vault {
        path: path.clone(),
        original,
        database,
        password: password.to_string(),
        password_cached: false,
    };
    vault.insert("skillet/new-token", "dummy").unwrap();
    std::fs::write(&path, b"external update").unwrap();
    assert!(vault
        .save_verified(None, "skillet/new-token", "dummy")
        .is_err());
}

#[test]
fn session_password_cache_uses_three_hour_expiry_and_can_be_cleared() {
    assert_eq!(CACHE_LIFETIME, std::time::Duration::from_hours(3));
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("secrets.kdbx");
    let cached = cache_store(&database, "dummy vault password").unwrap();
    assert_eq!(
        cache_read(&database).unwrap().as_deref(),
        cached.then_some("dummy vault password")
    );
    cache_clear(&database).unwrap();
    assert_eq!(cache_read(&database).unwrap(), None);
}

#[test]
fn missing_vault_fails_before_password_prompt() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("secrets.kdbx");
    let Err(error) = Vault::open(&path, None) else {
        panic!("missing vault unexpectedly opened");
    };
    assert!(error.to_string().contains("missing or unreadable"));
    assert!(error.to_string().contains("secrets.kdbx"));
}
