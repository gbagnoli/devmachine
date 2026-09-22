use super::*;
use std::env;

fn manager_with_secret(name: &str, payload: &str) -> (CredentialManager, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().expect("tempdir");
    std::fs::write(dir.path().join(name), payload).expect("write secret");
    env::set_var("CREDENTIALS_DIRECTORY", dir.path());
    let manager = CredentialManager::new().expect("manager");
    (manager, dir)
}

#[test]
fn read_secret_happy_path() {
    let (manager, _dir) = manager_with_secret("pihole_web_password", "s3cret\n");
    assert_eq!(manager.read_secret("pihole_web_password").unwrap(), "s3cret");
}

#[test]
fn read_secret_rejects_traversal() {
    let (manager, _dir) = manager_with_secret("pihole_web_password", "s3cret");
    for evil in [
        "../pihole_web_password",
        "sub/dir",
        "..\\pihole_web_password",
        "/etc/passwd",
        "..",
        "",
    ] {
        let err = manager.read_secret(evil).unwrap_err();
        assert!(
            matches!(err, CredentialError::InvalidName(_)),
            "expected InvalidName for {evil:?}, got {err:?}"
        );
    }
}

#[test]
fn read_secret_missing_file_is_read_error() {
    let (manager, _dir) = manager_with_secret("pihole_web_password", "s3cret");
    let err = manager.read_secret("does_not_exist").unwrap_err();
    assert!(
        matches!(err, CredentialError::ReadError(_, _)),
        "expected ReadError, got {err:?}"
    );
}
