use super::{
    create_entry, database_path_from, lookup, open_vault, read_vm_port, save_vault, OpenVault,
};
use keepass::Database;
use keepass::DatabaseKey;

#[test]
fn exact_vault_path_preserves_whitespace() {
    let mut db = Database::new();
    let mut root = db.root_mut();
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
        lookup(&db, "skillet/hosts/clamps/pihole/web-password").unwrap(),
        Some(" value\n".to_string())
    );
    assert_eq!(
        lookup(&db, "skillet/hosts/clamps/pihole/other").unwrap(),
        None
    );
    assert_eq!(
        lookup(&db, "skillet/hosts/other/pihole/web-password").unwrap(),
        None
    );
}

#[test]
fn vm_port_requires_manifest_range() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("run.conf");
    std::fs::write(&file, "ssh_port=2201\n").unwrap();
    assert_eq!(read_vm_port(&file).unwrap(), 2201);
    std::fs::write(&file, "ssh_port=22\n").unwrap();
    assert!(read_vm_port(&file).is_err());
}

#[test]
fn disposable_database_opens_with_correct_password_only() {
    let mut db = Database::new();
    let mut root = db.root_mut();
    let mut skillet = root.add_group();
    skillet.name = "skillet".to_string();
    let mut entry = skillet.add_entry();
    entry.set_unprotected("Title", "fixture");
    entry.set_protected("Password", "dummy only");
    let mut encrypted = Vec::new();
    db.save(
        &mut encrypted,
        DatabaseKey::new().with_password("fixture unlock"),
    )
    .unwrap();
    assert!(Database::open(
        &mut encrypted.as_slice(),
        DatabaseKey::new().with_password("wrong")
    )
    .is_err());
    let reopened = Database::open(
        &mut encrypted.as_slice(),
        DatabaseKey::new().with_password("fixture unlock"),
    )
    .unwrap();
    assert_eq!(
        lookup(&reopened, "skillet/fixture").unwrap(),
        Some("dummy only".to_string())
    );
}

#[test]
fn vault_path_uses_xdg_data_home_with_home_fallback() {
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
    assert!(database_path_from(None, None).is_err());
}

#[test]
fn missing_vault_fails_before_password_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.kdbx");
    let error = open_vault(&path, None).err().unwrap();
    assert!(error.to_string().contains("missing or unreadable"));
    assert!(error.to_string().contains("secrets.kdbx"));
}

#[test]
fn creates_and_reopens_encrypted_entry_through_symlink_target() {
    let dir = tempfile::tempdir().unwrap();
    let database_path = dir.path().join("synced.kdbx");
    let link_path = dir.path().join("secrets.kdbx");
    let db = Database::new();
    let password = "fixture unlock";
    let mut original = Vec::new();
    db.save(&mut original, DatabaseKey::new().with_password(password))
        .unwrap();
    std::fs::write(&database_path, &original).unwrap();
    std::os::unix::fs::symlink(&database_path, &link_path).unwrap();
    let path = std::fs::canonicalize(&link_path).unwrap();
    let mut vault = OpenVault {
        path,
        original,
        database: db,
        password: password.to_string(),
    };
    let entry_path = "skillet/hosts/clamps/pihole/web-password";
    create_entry(&mut vault.database, entry_path, "generated test value").unwrap();
    save_vault(&vault, None, entry_path, "generated test value").unwrap();
    assert!(link_path.is_symlink());
    let changed = std::fs::read(&database_path).unwrap();
    let reopened = Database::open(
        &mut changed.as_slice(),
        DatabaseKey::new().with_password(password),
    )
    .unwrap();
    assert_eq!(
        lookup(&reopened, entry_path).unwrap(),
        Some("generated test value".to_string())
    );
    assert!(std::fs::read_dir(dir.path()).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".bak")));
}
