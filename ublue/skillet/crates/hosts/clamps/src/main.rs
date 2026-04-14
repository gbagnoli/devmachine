use anyhow::Result;
use skillet_cli_common::run_host;
use skillet_core::credentials::CredentialManager;
use skillet_podman::{QuadletSecret, SecretTarget};

fn main() -> Result<()> {
    run_host("clamps", |system, files| {
        skillet_hardening::apply(system, files).map_err(|e| e.to_string())?;

        // 1. Ingest secret from systemd
        let cred_manager = CredentialManager::new()
            .map_err(|e: skillet_core::credentials::CredentialError| e.to_string())?;
        let secret_payload = cred_manager
            .read_secret("test_secret")
            .map_err(|e: skillet_core::credentials::CredentialError| e.to_string())?;

        // 2. Provision to Podman
        system
            .ensure_podman_secret("pihole_web_password", &secret_payload)
            .map_err(|e| e.to_string())?;

        // 3. Apply pihole with the secret
        let secrets = vec![QuadletSecret {
            secret_name: "pihole_web_password".to_string(),
            target: SecretTarget::File {
                target_path: "/etc/pihole/webpassword".to_string(),
                mode: Some("0400".to_string()),
                uid: None,
                gid: None,
            },
        }];

        skillet_pihole::apply(
            system,
            files,
            skillet_pihole::PiholeUser {
                uid: Some(0),
                gid: Some(0),
                name: "pihole".to_string(),
                group_name: "pihole".to_string(),
            },
            secrets,
        )
        .map_err(|e| e.to_string())?;

        Ok(())
    })?;
    Ok(())
}
