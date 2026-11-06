use super::*;

#[test]
fn activation_excludes_writers_and_finalization_gate_outlives_lock_owner() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let writer = lock(root, false).unwrap();
    assert!(lock(root, true).is_err());
    drop(writer);
    let activation = lock(root, true).unwrap();
    assert!(lock(root, false).is_err());
    fs::write(root.join("finalized"), b"reboot required").unwrap();
    drop(activation);
    assert!(lock(root, false).is_err());
    assert!(lock(root, true).is_ok());
}
