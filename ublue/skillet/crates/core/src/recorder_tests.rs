use super::*;
use crate::files::{FileMutationResource, OwnerIdentity, Ownership};
use crate::resource_op::EffectResult;
use crate::test_utils::{MockFiles, MockSystem};
use sha2::Digest as _;
use std::sync::atomic::Ordering;

#[test]
fn records_results_without_payloads_or_fingerprints() {
    let system = MockSystem::new();
    let recorder = Recorder::new(system);
    let secret = "distinctive-private-payload-for-recording-test";
    assert!(recorder
        .ensure_podman_secret("fixture-secret", secret)
        .unwrap());
    recorder
        .inner
        .fail_restart_once
        .store(true, Ordering::SeqCst);
    assert!(recorder.service_restart("fixture.service").is_err());

    let operations = recorder.get_ops();
    assert_eq!(operations[0].result, EffectResult::Changed(true));
    assert_eq!(operations[1].result, EffectResult::Failed);
    let yaml = serde_yml::to_string(&operations).unwrap();
    assert!(!yaml.contains(secret));
    assert!(!yaml.contains(&hex::encode(sha2::Sha256::digest(secret.as_bytes()))));
}

#[test]
fn file_recording_preserves_numeric_ownership() {
    let files = MockFiles::new();
    let recorder = Recorder::new(files);
    let ownership = Ownership {
        uid: Some(OwnerIdentity::Id(1234)),
        gid: Some(OwnerIdentity::Id(2345)),
    };
    recorder
        .ensure_file(
            std::path::Path::new("/tmp/recorded-file"),
            b"content",
            Some(0o640),
            &ownership,
        )
        .unwrap();

    let operations = recorder.get_ops();
    assert!(matches!(
        &operations[0].operation,
        ResourceOp::EnsureFile {
            ownership: recorded_ownership,
            ..
        } if recorded_ownership == &ownership
    ));
}
