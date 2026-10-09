use super::*;
use skillet_core::{
    files::FileReadResource,
    test_utils::{MockFiles, MockSystem},
};
use std::sync::atomic::Ordering;

fn production() -> Input {
    Input::Production {
        host: "smtp.example.com".into(),
        port: 587,
        tls: "starttls".into(),
        username: "fixture-key".into(),
        password: "fixture-secret".into(),
        sender: "server@example.com".into(),
    }
}
#[test]
fn production_config_requires_verified_tls_and_loopback_submission() {
    let config = production().main_config().unwrap();
    for directive in [
        "relayhost = [smtp.example.com]:587",
        "smtp_tls_security_level = secure",
        "inet_interfaces = loopback-only",
        "smtp_sasl_auth_enable = yes",
        "sender_canonical_maps = static:server@example.com",
        "smtp_fallback_relay =\n",
    ] {
        assert!(config.contains(directive));
    }
    assert!(!config.contains("fixture-secret"));
    assert!(!config.contains("fixture-key"));
    assert!(!config.contains("smtp_generic_maps"));
}
#[test]
fn capture_has_no_external_route_authentication_or_sender_rewriting() {
    let config = Input::Capture {}.main_config().unwrap();
    assert!(config.contains("relayhost = [127.0.0.1]:1025"));
    assert!(config.contains("smtp_sasl_auth_enable = no"));
    assert!(config.contains("sender_canonical_maps = "));
}
#[test]
fn invalid_input_is_rejected_without_echoing_payload_or_mutating_files() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    for input in [
        "PRIVATE-INVALID",
        r#"{"mode":"PRIVATE-VALUE"}"#,
        r#"{"mode":"capture","password":"PRIVATE-VALUE"}"#,
    ] {
        assert_eq!(
            apply(&system, &files, input).unwrap_err().to_string(),
            SmtpError::Invalid.to_string()
        );
    }
    assert_eq!(files.files.lock().unwrap().len(), 0);
    for sender in [
        "bad\n@example.com",
        "name@bad domain",
        "name@example.com:25",
        "a@b@c",
        "",
    ] {
        assert!(!valid_sender(sender));
    }
}
#[test]
fn volatile_map_has_restricted_metadata_and_is_not_recorded() {
    let files = MockFiles::new();
    let contents = files.files.clone();
    let metadata = files.metadata.clone();
    let recorder = skillet_core::recorder::Recorder::new(files);
    prepare(&recorder, &production()).unwrap();
    assert_eq!(
        contents.lock().unwrap()[MAP_PATH],
        b"[smtp.example.com]:587 fixture-key:fixture-secret\n"
    );
    assert_eq!(metadata.lock().unwrap()[MAP_PATH].0, Some(0o640));
    let recording = serde_json::to_string(&recorder.get_ops()).unwrap();
    assert!(!recording.contains("fixture-secret"));
}
#[test]
fn repeat_apply_is_noop_and_failed_activation_is_retryable() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    let payload = Input::Capture {}.payload().unwrap();
    apply(&system, &files, &payload).unwrap();
    let restarts = system.restart_count.load(Ordering::SeqCst);
    apply(&system, &files, &payload).unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), restarts);
    assert!(apply(&system, &files, &production().payload().unwrap()).is_err());
    let files = MockFiles::new();
    let system = MockSystem::new();
    system.fail_restart_once.store(true, Ordering::SeqCst);
    assert!(apply(&system, &files, &payload).is_err());
    assert!(files
        .read_file(Path::new("/var/lib/skillet/smtp/applied"))
        .unwrap()
        .is_none());
    apply(&system, &files, &payload).unwrap();
    assert!(files
        .read_file(Path::new("/var/lib/skillet/smtp/applied"))
        .unwrap()
        .is_some());
}

#[test]
fn mismatched_credentials_cannot_prepare_a_retained_environment() {
    let files = MockFiles::new();
    apply(
        &MockSystem::new(),
        &files,
        &Input::Capture {}.payload().unwrap(),
    )
    .unwrap();
    assert!(matches!(
        prepare(&files, &production()),
        Err(SmtpError::EnvironmentChange)
    ));
    assert!(files.read_file(Path::new(MAP_PATH)).unwrap().is_none());
}
