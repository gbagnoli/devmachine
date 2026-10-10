use super::*;
#[test]
fn explicit_payload_round_trips_and_legacy_defaults_to_no_exit_node() {
    for enabled in [false, true] {
        let input = EnrollmentInput::new("fixture-key".into(), enabled);
        let parsed = EnrollmentInput::parse(&input.payload().unwrap()).unwrap();
        assert_eq!(parsed.auth_key, "fixture-key");
        assert_eq!(parsed.advertise_exit_node, enabled);
    }
    let legacy = EnrollmentInput::parse("fixture-key\n").unwrap();
    assert_eq!(legacy.auth_key, "fixture-key");
    assert!(!legacy.advertise_exit_node);
}
#[test]
fn malformed_versioned_input_is_rejected_without_echoing_values() {
    for input in [
        "",
        "{private-invalid",
        r#"{"version":2,"auth_key":"private-key","advertise_exit_node":true}"#,
        r#"{"version":1,"auth_key":"private-key","advertise_exit_node":true,"extra":1}"#,
    ] {
        assert_eq!(
            EnrollmentInput::parse(input).err().unwrap().to_string(),
            "invalid Tailscale enrollment payload"
        );
    }
}
