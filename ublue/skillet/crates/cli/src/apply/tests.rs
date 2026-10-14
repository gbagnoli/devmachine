use super::*;
use clap::Parser;
fn parse(arguments: &[&str]) -> Args {
    let crate::Commands::Apply(args) = crate::Args::try_parse_from(arguments).unwrap().command
    else {
        panic!("expected apply");
    };
    args
}
#[test]
fn remote_apply_requires_positional_host_and_defaults_to_ssh_alias() {
    let args = parse(&["skillet", "apply", "clamps"]);
    assert_eq!(args.host.as_deref(), Some("clamps"));
    assert!(args.target.is_none());
    assert!(args.port.is_none());
    assert!(!args.local);
    assert!(!args.no_deploy);
    assert_eq!(args.phase, ApplyPhase::Full);
    assert!(matches!(args.environment, UiEnvironmentName::Production));
    assert!(args.identity.is_none());
    assert!(args.known_hosts.is_none());
    assert!(crate::Args::try_parse_from(["skillet", "apply"]).is_err());
    assert!(crate::Args::try_parse_from(["skillet", "apply", "--host", "clamps"]).is_err());
    for target in ["alternate", "user@example.invalid"] {
        assert_eq!(
            parse(&["skillet", "apply", "clamps", "--target", target])
                .target
                .as_deref(),
            Some(target)
        );
    }
}
#[test]
fn guest_host_file_and_explicit_local_modes_remain_available() {
    let args = parse(&[
        "skillet",
        "apply",
        "--host-file",
        "/etc/skillet/host",
        "--phase",
        "base",
    ]);
    assert!(args.host_file.is_some());
    assert_eq!(args.phase, ApplyPhase::Base);
    assert!(parse(&["skillet", "apply", "clamps", "--local", "--phase", "base"]).local);
    assert!(crate::Args::try_parse_from([
        "skillet",
        "apply",
        "--host-file",
        "/etc/skillet/host",
        "--record",
        "/tmp/record"
    ])
    .is_ok());
    for args in [
        vec!["skillet", "apply", "--target", "alias"],
        vec!["skillet", "apply", "clamps", "--host-file", "/tmp/host"],
        vec!["skillet", "apply", "clamps", "--local", "--target", "alias"],
        vec!["skillet", "apply", "clamps", "--local", "--no-deploy"],
        vec!["skillet", "apply", "clamps", "--record", "/tmp/record"],
    ] {
        assert!(crate::Args::try_parse_from(args).is_err());
    }
}
#[test]
fn invalid_remote_profile_or_policy_fails_before_connection_and_unlock() {
    assert!(dispatch(&parse(&["skillet", "apply", "unknown"]))
        .unwrap_err()
        .to_string()
        .contains("unknown host profile"));
    for option in [["--environment", "test"], ["--phase", "caddy"]] {
        let mut args = vec!["skillet", "apply", "clamps"];
        args.extend(option);
        assert!(dispatch(&parse(&args))
            .unwrap_err()
            .to_string()
            .contains("all production services"));
    }
}
#[test]
fn no_deploy_and_ssh_overrides_do_not_require_explicit_target() {
    assert!(parse(&["skillet", "apply", "clamps", "--no-deploy"]).no_deploy);
    let args = parse(&[
        "skillet",
        "apply",
        "clamps",
        "--port",
        "2227",
        "--identity",
        "/tmp/key",
    ]);
    assert_eq!(args.port, Some(2227));
    assert!(args.identity.is_some());
}
#[test]
fn deploy_requires_positional_host_but_target_is_an_optional_override() {
    for args in [
        vec!["skillet", "deploy", "clamps"],
        vec!["skillet", "deploy", "clamps", "--target", "alternate"],
        vec![
            "skillet",
            "deploy",
            "clamps",
            "--target",
            "user@example.invalid",
        ],
    ] {
        assert!(crate::Args::try_parse_from(args).is_ok());
    }
    assert!(crate::Args::try_parse_from(["skillet", "deploy"]).is_err());
    assert!(crate::Args::try_parse_from(["skillet", "deploy", "--host", "clamps"]).is_err());
}
