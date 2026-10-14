use crate::{
    files::{FileMutationResource, FileReadResource, LocalFileResource, Ownership},
    test_utils::MockFiles,
};
use std::{os::unix::fs::PermissionsExt, path::Path};

fn file_mutation_contract(resource: &(impl FileMutationResource + FileReadResource), path: &Path) {
    assert_eq!(resource.read_file(path).unwrap(), None);
    assert!(resource
        .ensure_file(path, b"initial", Some(0o640), &Ownership::default())
        .unwrap());
    assert_eq!(resource.read_file(path).unwrap(), Some(b"initial".to_vec()));
    assert!(!resource
        .ensure_file(path, b"initial", Some(0o640), &Ownership::default())
        .unwrap());
    assert!(resource
        .ensure_file(path, b"updated", Some(0o640), &Ownership::default())
        .unwrap());
    assert_eq!(resource.read_file(path).unwrap(), Some(b"updated".to_vec()));
    assert!(resource.delete_file(path).unwrap());
    assert_eq!(resource.read_file(path).unwrap(), None);
    assert!(!resource.delete_file(path).unwrap());
}

#[test]
fn local_file_resource_obeys_shared_mutation_contract() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("managed.conf");
    file_mutation_contract(&LocalFileResource::new(), &path);
}

#[test]
fn mock_file_resource_obeys_shared_mutation_contract() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("managed.conf");
    file_mutation_contract(&MockFiles::new(), &path);
}

#[test]
fn local_and_mock_file_resources_reject_file_directory_conflicts() {
    let directory = tempfile::tempdir().unwrap();
    let local_path = directory.path().join("local-conflict");
    let mock_path = directory.path().join("mock-conflict");
    let local = LocalFileResource::new();
    let mock = MockFiles::new();

    local
        .ensure_file(&local_path, b"file", None, &Ownership::default())
        .unwrap();
    mock.ensure_file(&mock_path, b"file", None, &Ownership::default())
        .unwrap();

    assert!(local
        .ensure_directory(&local_path, None, &Ownership::default())
        .is_err());
    assert!(mock
        .ensure_directory(&mock_path, None, &Ownership::default())
        .is_err());
}

fn missing_only_contract(resource: &(impl FileMutationResource + FileReadResource), path: &Path) {
    assert!(resource
        .create_file_if_absent(path, b"first", Some(0o600), &Ownership::default())
        .unwrap());
    assert!(!resource
        .create_file_if_absent(
            path,
            b"replacement",
            Some(0o777),
            &Ownership::named(Some("does-not-exist"), None)
        )
        .unwrap());
    assert_eq!(resource.read_file(path).unwrap(), Some(b"first".to_vec()));
}
#[test]
fn missing_only_native_and_fake_contract() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("seed");
    missing_only_contract(&LocalFileResource, &path);
    missing_only_contract(&MockFiles::new(), &path);
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}
#[test]
fn missing_only_rejects_symlinks_and_directories() {
    let temp = tempfile::tempdir().unwrap();
    let link = temp.path().join("link");
    std::os::unix::fs::symlink("absent", &link).unwrap();
    assert!(LocalFileResource
        .create_file_if_absent(&link, b"new", None, &Ownership::default())
        .is_err());
    assert!(std::fs::symlink_metadata(link).unwrap().is_symlink());
    for resource in [
        &LocalFileResource as &dyn FileMutationResource,
        &MockFiles::new(),
    ] {
        let dir = temp.path().join("directory");
        resource
            .ensure_directory(&dir, None, &Ownership::default())
            .unwrap();
        assert!(resource
            .create_file_if_absent(&dir, b"new", None, &Ownership::default())
            .is_err());
    }
}
#[test]
fn missing_only_concurrent_publish_has_one_winner() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("seed");
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let path = &path;
                scope.spawn(move || {
                    LocalFileResource
                        .create_file_if_absent(path, &[i; 4096], Some(0o600), &Ownership::default())
                        .unwrap()
                })
            })
            .collect();
        assert_eq!(
            handles
                .into_iter()
                .map(|h| h.join().unwrap())
                .filter(|won| *won)
                .count(),
            1
        );
    });
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(bytes.len(), 4096);
    assert!(bytes.iter().all(|b| *b == bytes[0]));
}
