use super::{
    parse_vm_tailscale_status, read_vm_port, validate_delivery_service,
    validate_tailscale_unit_config,
};

#[test]
fn vm_port_requires_manifest_range() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("run.conf");
    std::fs::write(&file, "ssh_port=2201\n").unwrap();
    assert_eq!(read_vm_port(&file).unwrap(), 2201);
    std::fs::write(&file, "ssh_port=22\n").unwrap();
    assert!(read_vm_port(&file).is_err());
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
fn failed_tailscale_status_command_does_not_mean_no_existing_enrollment() {
    use std::os::unix::process::ExitStatusExt;

    let output = std::process::Output {
        status: std::process::ExitStatus::from_raw(1 << 8),
        stdout: br#"{"BackendState":"NeedsLogin"}"#.to_vec(),
        stderr: b"permission denied".to_vec(),
    };
    let error = parse_vm_tailscale_status(&output).unwrap_err();
    assert!(error
        .to_string()
        .contains("refusing to treat probe failure"));
}

#[test]
fn delivery_eligibility_uses_declared_service_capabilities() {
    assert!(validate_delivery_service("beezelbot", "pihole").is_err());
    assert!(validate_delivery_service("beezelbot", "tailscale").is_err());
    assert!(validate_delivery_service("beezelbot", "caddy").is_ok());
    assert!(validate_delivery_service("missing-host", "caddy").is_err());
    assert!(validate_delivery_service("clamps", "unknown").is_err());
}
