use super::*;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};

#[test]
fn retirement_preserves_bytes_metadata_and_is_retryable() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    let archive = dir.path().join("archive");
    fs::write(&source, b"owned").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
    let before = fs::metadata(&source).unwrap();
    assert!(retire(&source, b"owned", &archive).unwrap());
    assert!(!source.exists());
    assert_eq!(regular(&archive).unwrap().unwrap(), b"owned");
    let after = fs::metadata(&archive).unwrap();
    assert_eq!(
        (before.ino(), before.mode(), before.uid(), before.gid()),
        (after.ino(), after.mode(), after.uid(), after.gid())
    );
    assert!(!retire(&source, b"owned", &archive).unwrap());
    // Rollback recreated the legacy file; remigration preserves both copies.
    fs::write(&source, b"owned").unwrap();
    assert!(retire(&source, b"owned", &archive).unwrap());
    assert_eq!(fs::read(&archive).unwrap(), b"owned");
    assert!(!source.exists());
}

#[test]
fn unknown_source_or_archive_is_never_overwritten() {
    for (source_bytes, archive_bytes) in [
        (b"edited".as_slice(), b"owned".as_slice()),
        (b"owned", b"edited"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let archive = dir.path().join("archive");
        fs::write(&source, source_bytes).unwrap();
        fs::write(&archive, archive_bytes).unwrap();
        assert!(retire(&source, b"owned", &archive).is_err());
        assert_eq!(fs::read(&source).unwrap(), source_bytes);
        assert_eq!(fs::read(&archive).unwrap(), archive_bytes);
    }
}

#[test]
fn nonregular_sources_archives_and_cross_directory_moves_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    let archive = dir.path().join("archive");
    let target = dir.path().join("target");
    fs::write(&target, b"owned").unwrap();
    symlink(&target, &source).unwrap();
    assert!(regular(&source).is_err());
    assert!(retire(&source, b"owned", &archive).is_err());
    fs::remove_file(&source).unwrap();
    fs::write(&source, b"owned").unwrap();
    symlink(&target, &archive).unwrap();
    assert!(retire(&source, b"owned", &archive).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"owned");
    assert!(retire(&source, b"owned", &source).is_err());
    assert!(retire(&source, b"owned", &dir.path().join("nested/archive")).is_err());
    assert!(regular(dir.path()).is_err());
}
