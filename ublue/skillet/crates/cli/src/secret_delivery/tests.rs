use super::{validate_delivery_service, validate_tailscale_unit_config};
use clap::Parser;

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
    assert!(error.to_string().contains("disposable DDNS is not enabled"));
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
    assert!(validate_delivery_service("beezelbot", "pihole").is_err());
    assert!(validate_delivery_service("beezelbot", "tailscale").is_err());
    assert!(validate_delivery_service("beezelbot", "caddy").is_ok());
    assert!(validate_delivery_service("beezelbot", "ddns").is_err());
    assert!(validate_delivery_service("clamps", "ddns").is_ok());
    assert!(validate_delivery_service("missing-host", "caddy").is_err());
    assert!(validate_delivery_service("clamps", "unknown").is_err());
}
