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
fn remote_apply_has_simple_defaults_and_production_policy() {
    let args = parse(&[
        "skillet",
        "apply",
        "--host",
        "clamps",
        "--target",
        "admin@example.invalid",
    ]);
    assert_eq!(args.phase, ApplyPhase::Full);
    assert!(matches!(args.environment, UiEnvironmentName::Production));
    assert_eq!(args.port, 22);
    assert!(args.identity.is_none());
    assert!(args.known_hosts.is_none());
}

#[test]
fn guest_arguments_remain_compatible_and_remote_host_is_explicit() {
    let args = parse(&[
        "skillet",
        "apply",
        "--host-file",
        "/etc/skillet/host",
        "--phase",
        "base",
    ]);
    assert!(args.target.is_none());
    assert_eq!(args.phase, ApplyPhase::Base);
    for arguments in [
        vec!["skillet", "apply", "--target", "admin@example.invalid"],
        vec!["skillet", "apply", "--identity", "/tmp/key"],
        vec![
            "skillet",
            "apply",
            "--host",
            "clamps",
            "--target",
            "admin@example.invalid",
            "--host-file",
            "/tmp/host",
        ],
        vec![
            "skillet",
            "apply",
            "--host",
            "clamps",
            "--target",
            "admin@example.invalid",
            "--record",
            "/tmp/record",
        ],
    ] {
        assert!(crate::Args::try_parse_from(arguments).is_err());
    }
}

#[test]
fn invalid_remote_profile_or_policy_fails_before_connection_and_unlock() {
    let args = parse(&[
        "skillet",
        "apply",
        "--host",
        "unknown",
        "--target",
        "admin@example.invalid",
        "--database",
        "/nonexistent/vault",
    ]);
    assert!(dispatch(&args)
        .unwrap_err()
        .to_string()
        .contains("unknown host profile"));
    for option in [["--environment", "test"], ["--phase", "caddy"]] {
        let mut arguments = vec![
            "skillet",
            "apply",
            "--host",
            "clamps",
            "--target",
            "admin@example.invalid",
        ];
        arguments.extend(option);
        assert!(dispatch(&parse(&arguments))
            .unwrap_err()
            .to_string()
            .contains("all production services"));
    }
}
