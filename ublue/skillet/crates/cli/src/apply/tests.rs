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
    assert!(!args.no_deploy);
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

#[test]
fn deployment_can_only_be_skipped_for_remote_apply() {
    let args = parse(&[
        "skillet",
        "apply",
        "--host",
        "clamps",
        "--target",
        "admin@example.invalid",
        "--no-deploy",
    ]);
    assert!(args.no_deploy);
    assert!(
        crate::Args::try_parse_from(["skillet", "apply", "--host", "clamps", "--no-deploy"])
            .is_err()
    );
}

#[test]
fn standalone_deploy_requires_profile_and_target_and_has_ssh_defaults() {
    assert!(crate::Args::try_parse_from(["skillet", "deploy", "--host", "clamps"]).is_err());
    assert!(crate::Args::try_parse_from([
        "skillet",
        "deploy",
        "--target",
        "admin@example.invalid"
    ])
    .is_err());
    assert!(crate::Args::try_parse_from([
        "skillet",
        "deploy",
        "--host",
        "clamps",
        "--target",
        "admin@example.invalid"
    ])
    .is_ok());
}
