use super::has_interactive_session;

#[test]
fn loginctl_classes_distinguish_interactive_from_background_sessions() {
    for class in [
        "user",
        "user-early",
        "user-incomplete",
        "greeter",
        "lock-screen",
    ] {
        let sessions = serde_json::json!([{"class": class}]);
        assert!(has_interactive_session(&sessions).unwrap(), "{class}");
    }

    for class in ["background", "manager"] {
        let sessions = serde_json::json!([{"class": class}]);
        assert!(!has_interactive_session(&sessions).unwrap(), "{class}");
    }

    assert!(!has_interactive_session(&serde_json::json!([])).unwrap());
}

#[test]
fn malformed_loginctl_sessions_fail_closed() {
    for sessions in [
        serde_json::json!({"sessions": []}),
        serde_json::json!([{}]),
        serde_json::json!([{"class": null}]),
    ] {
        assert!(has_interactive_session(&sessions).is_err());
    }
}

#[test]
fn normal_reboot_explicitly_disables_kexec_and_soft_reboot_fallbacks() {
    let command = super::normal_reboot_command();
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        ["--check-inhibitors=yes", "reboot"]
    );
    let environment = command
        .get_envs()
        .collect::<std::collections::BTreeMap<_, _>>();
    for key in [
        "SYSTEMCTL_SKIP_AUTO_KEXEC",
        "SYSTEMCTL_SKIP_AUTO_SOFT_REBOOT",
    ] {
        assert_eq!(
            environment[std::ffi::OsStr::new(key)],
            Some(std::ffi::OsStr::new("1"))
        );
    }
}
