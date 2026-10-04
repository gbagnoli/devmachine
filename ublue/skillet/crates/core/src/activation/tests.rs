use super::*;
use crate::test_utils::{MockFiles, MockSystem};
use std::path::Path;

fn run(
    system: &MockSystem,
    files: &MockFiles,
    request: &ActivationRequest,
) -> Result<ActivationOutcome, ActivationError> {
    activate(system, files, request)
}

fn request(revision: &[u8], changed: bool) -> ActivationRequest {
    ActivationRequest {
        service: "fixture.service".to_string(),
        state_path: PathBuf::from("/var/lib/skillet/activation/fixture.applied"),
        revision: revision.to_vec(),
        definition_changed: changed,
        reload_daemon: true,
        consumer_kind: ConsumerKind::Persistent,
    }
}

#[test]
fn tracks_definition_change_then_a_true_noop() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    let first = run(&system, &files, &request(b"v1", true)).unwrap();
    assert_eq!(first.action, ActivationAction::Restarted);
    let state_path = Path::new("/var/lib/skillet/activation/fixture.applied");
    assert_eq!(
        files.read_file(state_path).unwrap().as_deref(),
        Some(b"v1".as_slice())
    );
    let repeated = run(&system, &files, &request(b"v1", false)).unwrap();
    assert_eq!(repeated.action, ActivationAction::None);
    assert!(!repeated.recovered_pending_activation);
}

#[test]
fn stopped_persistent_service_is_started_without_definition_changes() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    run(&system, &files, &request(b"v1", true)).unwrap();
    system.service_stop("fixture.service").unwrap();
    let outcome = run(&system, &files, &request(b"v1", false)).unwrap();
    assert_eq!(outcome.action, ActivationAction::Started);
}

#[test]
fn reload_and_restart_failures_leave_revision_pending_for_recovery() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    let state_path = Path::new("/var/lib/skillet/activation/fixture.applied");
    system
        .fail_reload_once
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(run(&system, &files, &request(b"v1", true)).is_err());
    assert!(files.read_file(state_path).unwrap().is_none());
    run(&system, &files, &request(b"v1", true)).unwrap();

    system
        .fail_restart_once
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(run(&system, &files, &request(b"v2", true)).is_err());
    assert_eq!(
        files.read_file(state_path).unwrap().as_deref(),
        Some(b"v1".as_slice())
    );
    let retry = run(&system, &files, &request(b"v2", false)).unwrap();
    assert!(retry.recovered_pending_activation);
    assert_eq!(retry.action, ActivationAction::Restarted);
    assert_eq!(
        files.read_file(state_path).unwrap().as_deref(),
        Some(b"v2".as_slice())
    );
}

#[test]
fn failed_start_does_not_commit_revision_and_retries() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    let state_path = Path::new("/var/lib/skillet/activation/fixture.applied");
    let mut first = request(b"v1", true);
    first.consumer_kind = ConsumerKind::OneShot;
    system
        .fail_start_once
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(run(&system, &files, &first).is_err());
    assert!(files.read_file(state_path).unwrap().is_none());
    let mut retry = request(b"v1", false);
    retry.consumer_kind = ConsumerKind::OneShot;
    let result = run(&system, &files, &retry).unwrap();
    assert!(result.recovered_pending_activation);
    assert_eq!(result.action, ActivationAction::Started);
    assert_eq!(
        files.read_file(state_path).unwrap().as_deref(),
        Some(b"v1".as_slice())
    );
}

#[test]
fn completed_oneshot_is_not_started_again_when_inactive() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    let mut initial = request(b"v1", true);
    initial.consumer_kind = ConsumerKind::OneShot;
    run(&system, &files, &initial).unwrap();
    system.service_stop("fixture.service").unwrap();
    let mut repeat = request(b"v1", false);
    repeat.consumer_kind = ConsumerKind::OneShot;
    let outcome = run(&system, &files, &repeat).unwrap();
    assert_eq!(outcome.action, ActivationAction::None);
    assert_eq!(
        system.start_count.load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}
