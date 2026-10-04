use super::*;
use crate::tailscale::DeviceRecord;
use std::{
    collections::BTreeSet,
    fs,
    os::unix::fs::{symlink, PermissionsExt},
};

fn ownership() -> CloudflareVmOwnership {
    CloudflareVmOwnership {
        environment: "test".to_string(),
        identity: Some(identity()),
        marker: "skillet:test:host:instance".to_string(),
        zone_id: "zone-id".to_string(),
        ui_domain: "ui.example.invalid".to_string(),
        token_name: "skillet:test:host-instance".to_string(),
        token_id: Some("token-id".to_string()),
        expires_on: Some("2030-01-01T00:00:00Z".to_string()),
        record_ids: vec!["record-id".to_string()],
    }
}

fn identity() -> ProvisioningIdentity {
    ProvisioningIdentity {
        host: "host".to_string(),
        environment: "test".to_string(),
        instance: "instance".to_string(),
    }
}

#[test]
fn cloudflare_ownership_is_private_atomic_json_state() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cloudflare.json");
    let value = ownership();

    save_cloudflare_ownership(&path, &value).unwrap();

    assert_eq!(load_cloudflare_ownership(&path).unwrap(), value);
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        PRIVATE_FILE_MODE
    );
    validate_cloudflare_identity(&value, &identity()).unwrap();
    let mut wrong = identity();
    wrong.instance = "other".to_string();
    assert!(matches!(
        validate_cloudflare_identity(&value, &wrong),
        Err(ProvisioningStateError::IdentityMismatch)
    ));
}

#[test]
fn legacy_cloudflare_journal_remains_readable_but_new_identity_mismatches_fail() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cloudflare.json");
    let mut legacy = ownership();
    legacy.identity = None;
    fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();

    let loaded = load_cloudflare_ownership(&path).unwrap();
    assert_eq!(loaded, legacy);
    validate_cloudflare_identity(&loaded, &identity()).unwrap();
}

#[test]
fn tailscale_record_is_private_and_symlinks_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let record = DeviceRecord {
        id: "device-id".to_string(),
        hostname: "host-instance".to_string(),
        addresses: BTreeSet::from(["100.64.0.10".to_string()]),
    };
    save_tailscale_record(directory.path(), &identity(), &record).unwrap();
    assert_eq!(
        load_tailscale_record(
            &directory.path().join("tailscale.json"),
            &identity(),
            "host-instance"
        )
        .unwrap(),
        record
    );
    let mut wrong = identity();
    wrong.host = "other".to_string();
    assert!(matches!(
        load_tailscale_record(
            &directory.path().join("tailscale.json"),
            &wrong,
            "host-instance"
        ),
        Err(ProvisioningStateError::IdentityMismatch)
    ));

    let target = directory.path().join("target.json");
    fs::write(&target, b"{}").unwrap();
    let link = directory.path().join("linked.json");
    symlink(target, &link).unwrap();
    assert!(matches!(
        load_tailscale_record(&link, &identity(), "host-instance"),
        Err(ProvisioningStateError::InvalidPath(_))
    ));
    assert!(matches!(
        save_cloudflare_ownership(&link, &ownership()),
        Err(ProvisioningStateError::InvalidPath(_))
    ));
}

#[test]
fn legacy_tailscale_record_is_accepted_only_for_its_expected_hostname() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("tailscale.json");
    let legacy = DeviceRecord {
        id: "legacy-device-id".to_string(),
        hostname: "host-instance".to_string(),
        addresses: BTreeSet::new(),
    };
    fs::write(&path, serde_json::to_vec(&legacy).unwrap()).unwrap();

    assert_eq!(
        load_tailscale_record(&path, &identity(), "host-instance").unwrap(),
        legacy
    );
    assert!(matches!(
        load_tailscale_record(&path, &identity(), "other-instance"),
        Err(ProvisioningStateError::IdentityMismatch)
    ));
}

#[test]
fn pending_tailscale_marker_is_repeatable_and_identity_bound() {
    let directory = tempfile::tempdir().unwrap();
    mark_tailscale_pending(directory.path(), &identity(), "host-instance").unwrap();
    mark_tailscale_pending(directory.path(), &identity(), "host-instance").unwrap();
    assert!(matches!(
        mark_tailscale_pending(directory.path(), &identity(), "another-instance"),
        Err(ProvisioningStateError::PendingIdentityMismatch)
    ));
    let mut wrong = identity();
    wrong.instance = "other".to_string();
    assert!(matches!(
        mark_tailscale_pending(directory.path(), &wrong, "host-instance"),
        Err(ProvisioningStateError::PendingIdentityMismatch)
    ));
    remove_tailscale_pending(directory.path()).unwrap();
    remove_tailscale_pending(directory.path()).unwrap();
    assert!(!directory.path().join("tailscale-pending").exists());
}

#[test]
fn legacy_pending_marker_remains_usable_when_hostname_matches() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(directory.path().join("tailscale-pending"), "host-instance").unwrap();

    mark_tailscale_pending(directory.path(), &identity(), "host-instance").unwrap();
    assert!(matches!(
        mark_tailscale_pending(directory.path(), &identity(), "other-instance"),
        Err(ProvisioningStateError::PendingIdentityMismatch)
    ));
}
