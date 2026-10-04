use super::*;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

#[test]
fn data_mount_requires_the_expected_btrfs_subvolume() {
    let mountinfo = "1 0 0:31 /ostree/deploy/fedora-coreos/var /var rw - btrfs /dev/vda4 rw\n\
                     2 1 0:31 /data /var/lib/data rw - btrfs /dev/vda4 rw\n";
    let data = Path::new("/var/lib/data");
    let backing = Path::new("/var");

    assert!(require_btrfs_mount_in(mountinfo, data, backing, "/data").is_ok());
    assert!(matches!(
        require_btrfs_mount_in(mountinfo, data, backing, "/other"),
        Err(FileError::WrongMount(_))
    ));
    assert!(matches!(
        require_btrfs_mount_in(mountinfo, Path::new("/missing"), backing, "/data"),
        Err(FileError::MountMissing(_))
    ));
}

#[test]
fn data_mount_rejects_an_unrelated_filesystem() {
    let mountinfo = "1 0 0:31 /ostree/deploy/fedora-coreos/var /var rw - btrfs /dev/vda4 rw\n\
                     2 1 0:44 /data /var/lib/data rw - btrfs /dev/vdb1 rw\n";
    assert!(matches!(
        require_btrfs_mount_in(
            mountinfo,
            Path::new("/var/lib/data"),
            Path::new("/var"),
            "/data"
        ),
        Err(FileError::WrongMount(_))
    ));
}

#[test]
fn test_ensure_file_creates_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.txt");
    let content = b"hello world";
    let resource = LocalFileResource::new();

    let changed = resource
        .ensure_file(&file_path, content, None, &Ownership::default())
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
        .ensure_file(&file_path, content, None, &Ownership::default())
        .unwrap();
    assert!(changed);

    // Second write (same content)
    let changed_again = resource
        .ensure_file(&file_path, content, None, &Ownership::default())
        .unwrap();
    assert!(!changed_again);
}

#[test]
fn test_ensure_file_updates_content() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_update.txt");
    let resource = LocalFileResource::new();

    resource
        .ensure_file(&file_path, b"initial", None, &Ownership::default())
        .unwrap();

    let changed = resource
        .ensure_file(&file_path, b"updated", None, &Ownership::default())
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
        .ensure_file(&file_path, content, None, &Ownership::default())
        .unwrap();

    // 2. Change mode
    let changed = resource
        .ensure_file(&file_path, content, Some(0o644), &Ownership::default())
        .unwrap();
    assert!(changed);
    let meta = fs::metadata(&file_path).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, 0o644);

    // 3. Idempotent mode change
    let changed_again = resource
        .ensure_file(&file_path, content, Some(0o644), &Ownership::default())
        .unwrap();
    assert!(!changed_again);

    // Note: Testing owner/group change typically requires root, so we skip it in unit tests
    // or we would need to mock the underlying chown call.
}

#[test]
fn test_ensure_file_replaces_symlink() {
    let dir = tempdir().unwrap();
    let target_path = dir.path().join("target.txt");
    let link_path = dir.path().join("link.txt");
    fs::write(&target_path, b"target").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target_path, &link_path).unwrap();

    let resource = LocalFileResource::new();
    let content = b"new content";
    let changed = resource
        .ensure_file(&link_path, content, None, &Ownership::default())
        .unwrap();

    assert!(changed);
    assert!(link_path.exists());
    assert!(fs::symlink_metadata(&link_path).unwrap().is_file());
    assert_eq!(fs::read(&link_path).unwrap(), content);
}

#[test]
fn test_ensure_directory_creates_dir() {
    let dir = tempdir().unwrap();
    let sub_dir = dir.path().join("subdir");
    let resource = LocalFileResource::new();

    let changed = resource
        .ensure_directory(&sub_dir, Some(0o755), &Ownership::default())
        .unwrap();
    assert!(changed);
    assert!(sub_dir.exists());
    assert!(sub_dir.is_dir());
}

#[test]
fn directory_numeric_owner_is_idempotent() {
    use super::{OwnerIdentity, Ownership};
    use std::os::unix::fs::MetadataExt;

    let dir = tempdir().unwrap();
    let path = dir.path().join("numeric-owner");
    let parent = fs::metadata(dir.path()).unwrap();
    let resource = LocalFileResource::new();

    assert!(resource
        .ensure_directory(
            &path,
            Some(0o750),
            &Ownership {
                uid: Some(OwnerIdentity::Id(parent.uid())),
                gid: Some(OwnerIdentity::Id(parent.gid())),
            },
        )
        .unwrap());
    assert!(!resource
        .ensure_directory(
            &path,
            Some(0o750),
            &Ownership {
                uid: Some(OwnerIdentity::Id(parent.uid())),
                gid: Some(OwnerIdentity::Id(parent.gid())),
            },
        )
        .unwrap());

    let metadata = fs::metadata(path).unwrap();
    assert_eq!(
        (metadata.uid(), metadata.gid()),
        (parent.uid(), parent.gid())
    );
}

#[test]
fn file_numeric_owner_is_idempotent_and_does_not_need_name_lookup() {
    use std::os::unix::fs::MetadataExt;

    let dir = tempdir().unwrap();
    let path = dir.path().join("numeric-owner-file");
    let parent = fs::metadata(dir.path()).unwrap();
    let resource = LocalFileResource::new();
    let ownership = Ownership {
        uid: Some(OwnerIdentity::Id(parent.uid())),
        gid: Some(OwnerIdentity::Id(parent.gid())),
    };

    assert!(resource
        .ensure_file(&path, b"numeric owner", Some(0o640), &ownership)
        .unwrap());
    assert!(!resource
        .ensure_file(&path, b"numeric owner", Some(0o640), &ownership)
        .unwrap());

    let metadata = fs::metadata(path).unwrap();
    assert_eq!(
        (metadata.uid(), metadata.gid()),
        (parent.uid(), parent.gid())
    );
}

#[test]
fn numeric_file_owner_comparison_does_not_require_an_nss_entry() {
    use std::os::unix::fs::MetadataExt;

    let dir = tempdir().unwrap();
    let path = dir.path().join("unmapped-numeric-owner");
    fs::write(&path, b"fixture").unwrap();
    let metadata = fs::metadata(&path).unwrap();
    let unmatched_uid = metadata.uid().wrapping_add(1);

    assert!(LocalFileResource::check_metadata(
        &path,
        None,
        &Ownership {
            uid: Some(OwnerIdentity::Id(unmatched_uid)),
            gid: None,
        },
    )
    .unwrap());
}

#[test]
fn test_ensure_directory_fails_if_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("file.txt");
    fs::write(&file_path, b"not a dir").unwrap();
    let resource = LocalFileResource::new();

    let result = resource.ensure_directory(&file_path, None, &Ownership::default());
    assert!(result.is_err());
    match result {
        Err(FileError::NotADirectory(p)) => assert_eq!(p, file_path.display().to_string()),
        _ => panic!("Expected NotADirectory error, got {result:?}"),
    }
}

#[test]
fn test_ensure_directory_follows_symlink() {
    let dir = tempdir().unwrap();
    let target_dir = dir.path().join("target_dir");
    let link_path = dir.path().join("link_dir");
    fs::create_dir(&target_dir).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target_dir, &link_path).unwrap();

    let resource = LocalFileResource::new();
    let changed = resource
        .ensure_directory(&link_path, None, &Ownership::default())
        .unwrap();

    // target_dir already exists, and we follow the symlink link_path to it.
    // So no change should be reported.
    assert!(!changed);
    assert!(link_path.exists());
    assert!(link_path.is_dir());
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
