use super::{validate_delivery_service, validate_tailscale_unit_config};
use clap::Parser;

#[test]
fn plural_secrets_commands_parse_and_unknown_check_host_fails_before_unlock() {
    for command in ["unlock", "lock", "check"] {
        assert!(crate::Args::try_parse_from(["skillet", "secrets", command]).is_ok());
        assert!(crate::Args::try_parse_from(["skillet", "secret", command]).is_ok());
    }
    let parsed = crate::Args::try_parse_from([
        "skillet",
        "secrets",
        "check",
        "--host",
        "unknown",
        "--environment",
        "test",
        "--database",
        "/nonexistent/secrets.kdbx",
        "--key-file",
        "/nonexistent/key",
    ])
    .unwrap();
    let crate::Commands::Secret {
        command: crate::SecretCommands::Check(args),
    } = parsed.command
    else {
        panic!("wrong command")
    };
    assert!(super::check_vault(&args)
        .unwrap_err()
        .to_string()
        .contains("unknown host profile"));
}

#[test]
fn ddns_commands_parse_and_test_delivery_refuses_before_vault_access() {
    let parsed = crate::Args::try_parse_from([
        "skillet",
        "secret",
        "deliver",
        "clamps",
        "ddns",
        "--environment",
        "test",
        "--database",
        "/nonexistent/skillet-test.kdbx",
        "--target",
        "user@localhost",
        "--identity",
        "/nonexistent/key",
        "--known-hosts",
        "/nonexistent/known_hosts",
    ])
    .unwrap();
    let crate::Commands::Secret {
        command: crate::SecretCommands::Deliver(args),
    } = parsed.command
    else {
        panic!("wrong parsed command");
    };
    let error = super::deliver_from_vault(&args).unwrap_err();
    assert!(error
        .to_string()
        .contains("test DDNS must be provisioned through an owned disposable VM"));
    assert!(crate::Args::try_parse_from([
        "skillet", "apply", "clamps", "--local", "--phase", "ddns"
    ])
    .is_ok());
}

#[test]
fn rejects_vm_without_tailscale_systemd_credential() {
    let old_unit = "LoadCredentialEncrypted=pihole_web_password:/etc/credstore.encrypted/skillet/pihole_web_password.cred";
    let error = validate_tailscale_unit_config(old_unit).unwrap_err();
    assert!(error.to_string().contains("recreate the smoke VM"));

    let current_unit = "LoadCredentialEncrypted=tailscale_auth_key:/etc/credstore.encrypted/skillet/tailscale_auth_key.cred";
    assert!(validate_tailscale_unit_config(current_unit).is_ok());
}

#[test]
fn delivery_eligibility_uses_declared_service_capabilities() {
    assert!(validate_delivery_service("agent", "datadog").is_err());
    assert!(validate_delivery_service("clamps", "datadog").is_ok());
    assert!(validate_delivery_service("agent", "pihole").is_err());
    assert!(validate_delivery_service("agent", "tailscale").is_err());
    assert!(validate_delivery_service("joey", "caddy").is_ok());
    assert!(validate_delivery_service("agent", "ddns").is_err());
    assert!(validate_delivery_service("clamps", "ddns").is_ok());
    for service in ["pihole", "tailscale", "caddy", "ddns", "datadog", "smtp"] {
        assert!(validate_delivery_service("joey", service).is_ok());
    }
    assert!(validate_delivery_service("missing-host", "caddy").is_err());
    assert!(validate_delivery_service("clamps", "unknown").is_err());
}

#[test]
fn datadog_commands_parse_and_test_delivery_refuses_before_vault_access() {
    assert!(crate::Args::try_parse_from([
        "skillet", "apply", "clamps", "--local", "--phase", "datadog"
    ])
    .is_ok());
    assert!(crate::Args::try_parse_from([
        "skillet",
        "test",
        "vm",
        "provision",
        "clamps",
        "monitoring",
        "--with-datadog"
    ])
    .is_ok());
    let parsed = crate::Args::try_parse_from([
        "skillet",
        "secret",
        "deliver",
        "clamps",
        "datadog",
        "--environment",
        "test",
        "--database",
        "/nonexistent/skillet-test.kdbx",
        "--target",
        "user@localhost",
        "--identity",
        "/nonexistent/key",
        "--known-hosts",
        "/nonexistent/known_hosts",
    ])
    .unwrap();
    let crate::Commands::Secret {
        command: crate::SecretCommands::Deliver(args),
    } = parsed.command
    else {
        panic!("wrong parsed command");
    };
    assert!(super::deliver_from_vault(&args)
        .unwrap_err()
        .to_string()
        .contains("test Datadog must be provisioned through an owned disposable VM"));
}

#[test]
fn smtp_delivery_and_capture_only_cli_are_supported() {
    use clap::Parser;
    assert!(super::super::Args::try_parse_from([
        "skillet",
        "secrets",
        "deliver",
        "clamps",
        "smtp",
        "--target",
        "root@example.invalid",
        "--identity",
        "/tmp/key",
        "--known-hosts",
        "/tmp/hosts"
    ])
    .is_ok());
    assert!(super::super::Args::try_parse_from([
        "skillet",
        "test",
        "vm",
        "provision",
        "joey",
        "smtp",
        "--smtp-only"
    ])
    .is_ok());
    assert!(super::validate_delivery_service("agent", "smtp").is_err());
    assert!(super::validate_delivery_service("joey", "smtp").is_ok());
}

#[test]
fn live_smtp_requires_explicit_smtp_only_selection() {
    use clap::Parser;
    let base = ["skillet", "test", "vm", "provision", "joey", "mailjet"];
    assert!(super::super::Args::try_parse_from(base.into_iter().chain(["--live-smtp"])).is_err());
    let parsed =
        super::super::Args::try_parse_from(base.into_iter().chain(["--smtp-only", "--live-smtp"]))
            .unwrap();
    let crate::Commands::Test {
        command:
            crate::TestCommands::Vm {
                command: crate::VmCommands::Provision(args),
            },
    } = parsed.command
    else {
        panic!("wrong parsed command");
    };
    assert!(args.smtp_only && args.live_smtp);
    let parsed =
        super::super::Args::try_parse_from(base.into_iter().chain(["--smtp-only"])).unwrap();
    let crate::Commands::Test {
        command:
            crate::TestCommands::Vm {
                command: crate::VmCommands::Provision(args),
            },
    } = parsed.command
    else {
        panic!("wrong parsed command");
    };
    assert!(!args.live_smtp);
}

#[test]
fn delivery_defaults_to_staging_with_openssh_identity_selection() {
    for activate in [false, true] {
        let mut arguments = vec![
            "skillet",
            "secrets",
            "deliver",
            "clamps",
            "tailscale",
            "--target",
            "user@example.invalid",
        ];
        if activate {
            arguments.push("--activate");
        }
        let parsed = crate::Args::try_parse_from(arguments).unwrap();
        let crate::Commands::Secret {
            command: crate::SecretCommands::Deliver(args),
        } = parsed.command
        else {
            panic!("wrong command");
        };
        assert_eq!(args.activate, activate);
        assert!(args.identity.is_none());
        assert!(args.known_hosts.is_none());
        assert!(args.port.is_none());
    }
}

#[test]
fn all_test_deliveries_require_owned_vm_before_any_vault_or_ssh_access() {
    for service in ["pihole", "tailscale", "caddy", "smtp", "ddns", "datadog"] {
        let parsed = crate::Args::try_parse_from([
            "skillet",
            "secrets",
            "deliver",
            "clamps",
            service,
            "--target",
            "user@example.invalid",
            "--environment",
            "test",
            "--database",
            "/nonexistent/vault",
        ])
        .unwrap();
        let crate::Commands::Secret {
            command: crate::SecretCommands::Deliver(args),
        } = parsed.command
        else {
            panic!("wrong command");
        };
        assert!(super::deliver_from_vault(&args)
            .unwrap_err()
            .to_string()
            .contains("owned disposable VM"));
    }
}

#[test]
fn production_recovery_ssh_overrides_are_optional() {
    let parsed = crate::Args::try_parse_from([
        "skillet",
        "secrets",
        "root-recovery-key",
        "clamps",
        "--target",
        "user@example.invalid",
    ])
    .unwrap();
    let crate::Commands::Secret {
        command: crate::SecretCommands::RootRecoveryKey(args),
    } = parsed.command
    else {
        panic!("wrong command");
    };
    assert!(args.identity.is_none());
    assert!(args.known_hosts.is_none());
}

#[test]
fn protected_pihole_delivery_does_not_open_vault_or_connect_to_a_guest() {
    let parsed = crate::Args::try_parse_from([
        "skillet",
        "secrets",
        "deliver",
        "clamps",
        "pihole",
        "--activate",
        "--database",
        "/nonexistent/vault.kdbx",
        "--target",
        "nobody@localhost",
        "--identity",
        "/nonexistent/key",
        "--known-hosts",
        "/nonexistent/known_hosts",
    ])
    .unwrap();
    let crate::Commands::Secret {
        command: crate::SecretCommands::Deliver(args),
    } = parsed.command
    else {
        panic!("wrong command");
    };
    super::deliver_from_vault(&args).unwrap();
}
