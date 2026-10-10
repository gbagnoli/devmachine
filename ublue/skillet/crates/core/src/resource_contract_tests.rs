use crate::{
    files::{FileMutationResource, FileReadResource, LocalFileResource, Ownership},
    test_utils::MockFiles,
};
use std::path::Path;

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
