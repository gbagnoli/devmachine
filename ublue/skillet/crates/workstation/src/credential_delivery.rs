//! Workstation-side persistence and delivery of host credentials.

use crate::{
    provisioning_policy::{DeviceClass, Environment, ProvisioningPolicy},
    tailscale::{self, AuthKey, OAuthCredentials, TailscaleError},
    vault::{SecretStore, VaultError},
};
use skillet_vm::{credential, transport::GuestTransport};
use std::{fs::File, io::Read};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CredentialDeliveryError {
    #[error(transparent)]
    Datadog(#[from] skillet_datadog::DatadogError),
    #[error("invalid host credential delivery: {0}")]
    Invalid(String),
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error(transparent)]
    Tailscale(#[from] TailscaleError),
    #[error(transparent)]
    Guest(#[from] skillet_vm::Error),
    #[error("operating system randomness is unavailable: {0}")]
    Randomness(#[from] std::io::Error),
}

pub fn deliver_datadog_credential(
    host: &str,
    policy: ProvisioningPolicy,
    input: &str,
    guest: &impl GuestTransport,
) -> Result<(), CredentialDeliveryError> {
    let profile = skillet_hosts::profile_for_name(host)
        .filter(|profile| profile.supports_service("datadog"))
        .ok_or_else(|| CredentialDeliveryError::Invalid("host does not declare Datadog".into()))?;
    let production_region = profile
        .services
        .iter()
        .find_map(|service| match service.config {
            skillet_hosts::ServiceConfig::Datadog {
                production_region, ..
            } => Some(production_region),
            _ => None,
        })
        .ok_or_else(|| CredentialDeliveryError::Invalid("host does not declare Datadog".into()))?;
    let input = skillet_datadog::Input::parse(input)?
        .render_for_environment(policy.vault_name(), production_region)?;
    credential::install(
        guest,
        profile.id.as_str(),
        skillet_datadog::CREDENTIAL,
        "skillet-datadog-apply.service",
        credential::ActivationPolicy::StartConsumer,
        input.as_bytes(),
    )?;
    Ok(())
}

pub trait ProductionAuthKeyProvider {
    fn create_auth_key(&self, tag: &str, description: &str) -> tailscale::Result<AuthKey>;
}

impl ProductionAuthKeyProvider for OAuthCredentials {
    fn create_auth_key(&self, tag: &str, description: &str) -> tailscale::Result<AuthKey> {
        tailscale::create_auth_key(self, tag, description)
    }
}

pub fn deliver_pihole_credential(
    host: &str,
    store: &mut impl SecretStore,
    guest: &impl GuestTransport,
) -> Result<(), CredentialDeliveryError> {
    let profile = skillet_hosts::profile_for_name(host)
        .ok_or_else(|| CredentialDeliveryError::Invalid(format!("unknown host profile {host}")))?;
    if !profile.supports_service("pihole") {
        return Err(CredentialDeliveryError::Invalid(format!(
            "host {host} does not declare Pi-hole credential delivery"
        )));
    }
    let path = format!("skillet/hosts/{host}/pihole/web-password");
    let password = if let Some(password) = store.get(&path)? {
        store.ensure_unchanged()?;
        password
    } else {
        if remote_credential_present(host, guest)? {
            return Err(CredentialDeliveryError::Invalid(
                "host already has a Pi-hole credential; restore the missing KeePassXC entry instead of creating a replacement".into(),
            ));
        }
        store.ensure_unchanged()?;
        let password = random_password()?;
        store.save_verified(&path, &password)?;
        password
    };
    credential::install(
        guest,
        host,
        "pihole_web_password",
        "skillet-full-apply.service",
        credential::ActivationPolicy::StartConsumer,
        password.as_bytes(),
    )?;
    Ok(())
}

pub fn deliver_tailscale_credential(
    host: &str,
    policy: ProvisioningPolicy,
    provider: &impl ProductionAuthKeyProvider,
    guest: &impl GuestTransport,
) -> Result<(), CredentialDeliveryError> {
    if policy.environment() != Environment::Production {
        return Err(CredentialDeliveryError::Invalid(
            "production Tailscale credential delivery requires production policy".into(),
        ));
    }
    let profile = skillet_hosts::profile_for_name(host)
        .ok_or_else(|| CredentialDeliveryError::Invalid(format!("unknown host profile {host}")))?;
    if !profile.supports_service("tailscale") {
        return Err(CredentialDeliveryError::Invalid(format!(
            "host {host} does not declare Tailscale credential delivery"
        )));
    }
    let tag = policy.tailscale_tag(DeviceClass::ProductionHost);
    let auth_key = provider.create_auth_key(tag, &format!("Skillet {host} production host"))?;
    credential::install(
        guest,
        host,
        "tailscale_auth_key",
        "skillet-full-apply.service",
        credential::ActivationPolicy::StartConsumer,
        auth_key.key.as_bytes(),
    )?;
    Ok(())
}

pub fn ensure_disposable_pihole_credential(
    host: &str,
    rotate: bool,
    guest: &impl GuestTransport,
) -> Result<(), CredentialDeliveryError> {
    let profile = skillet_hosts::profile_for_name(host)
        .ok_or_else(|| CredentialDeliveryError::Invalid(format!("unknown host profile {host}")))?;
    if !profile.supports_service("pihole") {
        return Err(CredentialDeliveryError::Invalid(format!(
            "host {host} does not declare Pi-hole"
        )));
    }
    let output = guest.execute(
        &skillet_vm::transport::GuestCommand {
            program: "/usr/bin/sudo",
            arguments: &[
                "-n",
                "test",
                "-s",
                "/etc/credstore.encrypted/skillet/pihole_web_password.cred",
            ],
        },
        None,
    )?;
    let present = match output.status.code() {
        Some(0) => true,
        Some(1) => false,
        _ => {
            return Err(CredentialDeliveryError::Invalid(
                "could not inspect disposable VM credential state".into(),
            ));
        }
    };
    if rotate || !present {
        credential::install(
            guest,
            host,
            "pihole_web_password",
            "skillet-full-apply.service",
            credential::ActivationPolicy::StartConsumer,
            random_password()?.as_bytes(),
        )?;
    } else {
        let output = guest.execute(
            &skillet_vm::transport::GuestCommand {
                program: "/usr/bin/sudo",
                arguments: &["-n", "systemctl", "start", "skillet-full-apply.service"],
            },
            None,
        )?;
        if !output.status.success() {
            return Err(CredentialDeliveryError::Invalid(format!(
                "starting host apply failed with status {}",
                output.status
            )));
        }
    }
    Ok(())
}

fn remote_credential_present(
    host: &str,
    guest: &impl GuestTransport,
) -> Result<bool, CredentialDeliveryError> {
    let program = format!("/var/usrlocal/bin/skillet-{host}");
    let output = guest.execute(
        &skillet_vm::transport::GuestCommand {
            program: "/usr/bin/sudo",
            arguments: &[
                "-n",
                program.as_str(),
                "credential",
                "state",
                "pihole_web_password",
            ],
        },
        None,
    )?;
    if !output.status.success() {
        return Err(CredentialDeliveryError::Invalid(
            "host credential state check failed; no production credential was generated".into(),
        ));
    }
    match output.stdout.as_slice() {
        b"present\n" => Ok(true),
        b"absent\n" => Ok(false),
        _ => Err(CredentialDeliveryError::Invalid(
            "host returned an unrecognized credential state".into(),
        )),
    }
}

fn random_password() -> Result<String, std::io::Error> {
    let mut bytes = [0_u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(hex::encode(bytes))
}

#[cfg(test)]
#[path = "credential_delivery/tests.rs"]
mod tests;
