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
        "skillet", "apply", "--host", "clamps", "--phase", "ddns"
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
    assert!(validate_delivery_service("beezelbot", "datadog").is_err());
    assert!(validate_delivery_service("clamps", "datadog").is_ok());
    assert!(validate_delivery_service("beezelbot", "pihole").is_err());
    assert!(validate_delivery_service("beezelbot", "tailscale").is_err());
    assert!(validate_delivery_service("beezelbot", "caddy").is_ok());
    assert!(validate_delivery_service("beezelbot", "ddns").is_err());
    assert!(validate_delivery_service("clamps", "ddns").is_ok());
    assert!(validate_delivery_service("missing-host", "caddy").is_err());
    assert!(validate_delivery_service("clamps", "unknown").is_err());
}

#[test]
fn datadog_commands_parse_and_test_delivery_refuses_before_vault_access() {
    assert!(crate::Args::try_parse_from([
        "skillet", "apply", "--host", "clamps", "--phase", "datadog"
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
        "beezelbot",
        "smtp",
        "--smtp-only"
    ])
    .is_ok());
    assert!(super::validate_delivery_service("agent", "smtp").is_err());
    assert!(super::validate_delivery_service("beezelbot", "smtp").is_ok());
}
