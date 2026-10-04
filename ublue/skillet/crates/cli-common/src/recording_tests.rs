use super::*;
use skillet_core::resource_op::{EffectResult, RecordedOperation, ResourceOp};
use skillet_core::test_utils::{MockFiles, MockSystem};
use std::sync::atomic::Ordering;

#[test]
fn diagnostic_recording_is_versioned_and_contains_only_sanitized_results() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nested/apply.yml");
    let diagnostic = DiagnosticRecording {
        format_version: 1,
        host: "fixture-host",
        outcome: "failed",
        operations: vec![RecordedOperation {
            operation: ResourceOp::EnsurePodmanSecret {
                name: "fixture-secret".to_string(),
            },
            result: EffectResult::Failed,
        }],
    };
    persist_recording(&path, &diagnostic).unwrap();
    let output = fs::read_to_string(path).unwrap();
    assert!(output.contains("format_version: 1"));
    assert!(output.contains("outcome: failed"));
    assert!(output.contains("status: failed"));
    assert!(!output.contains("payload"));
}

#[test]
fn failed_apply_is_recorded_without_secret_value() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("apply.yml");
    let secret = "distinctive-failed-apply-secret";
    let system = MockSystem::new();
    system.fail_restart_once.store(true, Ordering::SeqCst);
    let result = handle_recorded_apply(
        "fixture-host",
        &path,
        system,
        MockFiles::new(),
        &CredentialInputs::default(),
        |system, _, _| {
            system
                .ensure_podman_secret("fixture-secret", secret)
                .map_err(|error| error.to_string())?;
            system
                .service_restart("fixture.service")
                .map_err(|error| error.to_string())
        },
    );
    assert!(
        matches!(result, Err(CliCommonError::Config(message)) if message.contains("injected restart failure"))
    );
    let output = fs::read_to_string(path).unwrap();
    assert!(output.contains("outcome: failed"));
    assert!(output.contains("changed: true"));
    assert!(output.contains("status: failed"));
    assert!(!output.contains(secret));
}

#[test]
fn recording_failure_preserves_apply_cause() {
    let directory = tempfile::tempdir().unwrap();
    let parent_file = directory.path().join("parent-file");
    fs::write(&parent_file, "block directory creation").unwrap();
    let path = parent_file.join("apply.yml");
    let result = handle_recorded_apply(
        "fixture-host",
        &path,
        MockSystem::new(),
        MockFiles::new(),
        &CredentialInputs::default(),
        |_, _, _| Err("original apply cause".to_string()),
    );
    assert!(
        matches!(result, Err(CliCommonError::ApplyAndRecord { apply, .. }) if apply == "original apply cause")
    );
}
