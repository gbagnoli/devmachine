use super::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_ensure_file_creates_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    let content = b"hello world";
    let resource = LocalFileResource::new();

    let changed = resource
        .ensure_file(&file_path, content, None, None, None)
        .unwrap();
    assert!(changed);
    assert!(file_path.exists());
    assert_eq!(fs::read(&file_path).unwrap(), content);
}

#[test]
fn test_ensure_file_idempotent() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_idempotent.txt");
    let content = b"idempotent";
    let resource = LocalFileResource::new();

    // First write
    let changed = resource
        .ensure_file(&file_path, content, None, None, None)
        .unwrap();
    assert!(changed);

    // Second write (same content)
    let changed_again = resource
        .ensure_file(&file_path, content, None, None, None)
        .unwrap();
    assert!(!changed_again);
}

#[test]
fn test_ensure_file_updates_content() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_update.txt");
    let resource = LocalFileResource::new();

    resource
        .ensure_file(&file_path, b"initial", None, None, None)
        .unwrap();

    let changed = resource
        .ensure_file(&file_path, b"updated", None, None, None)
        .unwrap();
    assert!(changed);
    assert_eq!(fs::read(&file_path).unwrap(), b"updated");
}

#[test]
fn test_delete_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_delete.txt");
    fs::write(&file_path, b"delete me").unwrap();
    let resource = LocalFileResource::new();

    let changed = resource.delete_file(&file_path).unwrap();
    assert!(changed);
    assert!(!file_path.exists());

    let changed_again = resource.delete_file(&file_path).unwrap();
    assert!(!changed_again);
}
