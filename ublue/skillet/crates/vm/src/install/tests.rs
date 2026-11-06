use super::*;
fn source() -> Value {
    serde_yml::from_str("storage:\n  filesystems:\n    - device: /dev/disk/by-partlabel/root\n      format: btrfs\n      label: root\n      wipe_filesystem: true\n").unwrap()
}
#[test]
fn unencrypted_is_preserved_and_tpm_replaces_raw_root() {
    let mut value = source();
    let original = value.clone();
    configure_root(&mut value, RootProfile::Unencrypted).unwrap();
    assert_eq!(value, original);
    configure_root(&mut value, RootProfile::Tpm).unwrap();
    assert_eq!(
        value["storage"]["filesystems"][0]["device"].as_str(),
        Some("/dev/mapper/root")
    );
    assert_eq!(
        value["storage"]["luks"][0]["clevis"]["custom"]["config"].as_str(),
        Some("{\"pcr_bank\":\"sha256\",\"pcr_ids\":\"7\"}")
    );
    assert!(configure_root(&mut value, RootProfile::Tpm).is_err());
}
#[test]
fn missing_root_is_an_error_not_unencrypted_fallback() {
    let mut value = serde_yml::from_str("storage: {}").unwrap();
    assert!(configure_root(&mut value, RootProfile::Tpm).is_err());
}

#[test]
fn conflicting_filesystems_and_preexisting_encryption_are_rejected() {
    for extra in [
        "device: /dev/disk/by-partlabel/root\nlabel: other\nformat: xfs",
        "device: /dev/mapper/root\nlabel: other\nformat: btrfs",
        "device: /dev/other\nlabel: root\nformat: btrfs",
    ] {
        let mut value = source();
        value["storage"]["filesystems"]
            .as_sequence_mut()
            .unwrap()
            .push(serde_yml::from_str(extra).unwrap());
        assert!(configure_root(&mut value, RootProfile::Tpm).is_err());
    }
    let mut value = source();
    value["storage"]["luks"] = Value::Sequence(Vec::new());
    assert!(configure_root(&mut value, RootProfile::Tpm).is_err());
}
