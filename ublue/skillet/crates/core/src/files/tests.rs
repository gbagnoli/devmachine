use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
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
fn test_ensure_file_metadata() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_meta.txt");
    let resource = LocalFileResource::new();
    let content = b"metadata test";

    // 1. Create with default meta
    resource
        .ensure_file(&file_path, content, None, None, None)
        .unwrap();

    // 2. Change mode
    let changed = resource
        .ensure_file(&file_path, content, Some(0o644), None, None)
        .unwrap();
    assert!(changed);
    let meta = fs::metadata(&file_path).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, 0o644);

    // 3. Idempotent mode change
    let changed_again = resource
        .ensure_file(&file_path, content, Some(0o644), None, None)
        .unwrap();
    assert!(!changed_again);

    // Note: Testing owner/group change typically requires root, so we skip it in unit tests
    // or we would need to mock the underlying chown call.
}

#[test]
fn test_ensure_directory_creates_dir() {
    let dir = tempdir().unwrap();
    let sub_dir = dir.path().join("subdir");
    let resource = LocalFileResource::new();

    let changed = resource
        .ensure_directory(&sub_dir, Some(0o755), None, None)
        .unwrap();
    assert!(changed);
    assert!(sub_dir.exists());
    assert!(sub_dir.is_dir());
}

#[test]
fn test_ensure_directory_fails_if_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    fs::write(&file_path, b"not a dir").unwrap();
    let resource = LocalFileResource::new();

    let result = resource.ensure_directory(&file_path, None, None, None);
    assert!(result.is_err());
    match result {
        Err(FileError::NotADirectory(p)) => assert_eq!(p, file_path.display().to_string()),
        _ => panic!("Expected NotADirectory error, got {result:?}"),
    }
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
