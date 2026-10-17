//! Read-only authoritative zone lookup and private bootstrap delivery.
use crate::{
    cloudflare::Cloudflare,
    provisioning_policy::ProvisioningPolicy,
    vault::{Vault, VaultError},
};
use skillet_syncthing::initial::{self, Settings};
use skillet_vm::{
    credential::{self, ActivationPolicy},
    transport::GuestTransport,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("host does not select Syncthing")]
    Host,
    #[error("missing shared Cloudflare zone or token-creator entry")]
    Missing,
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error(transparent)]
    Cloudflare(#[from] crate::cloudflare::CloudflareError),
    #[error(transparent)]
    Input(#[from] initial::Error),
    #[error(transparent)]
    Guest(#[from] skillet_vm::Error),
}

pub fn deliver(
    host: &str,
    runtime_hostname: &str,
    policy: ProvisioningPolicy,
    vault: &Vault,
    guest: &impl GuestTransport,
    activation: ActivationPolicy,
) -> Result<(), Error> {
    let zone_id = vault
        .get(&policy.cloudflare_zone_entry())?
        .ok_or(Error::Missing)?;
    let token = vault
        .get("skillet/cloudflare/token-creator")?
        .ok_or(Error::Missing)?;
    let zone = Cloudflare::new().zone(&token, &zone_id)?;
    if zone.id != zone_id {
        return Err(Error::Missing);
    }
    let settings = settings(host, runtime_hostname, policy, &zone.name)?;
    deliver_settings(&settings, guest, activation)?;
    Ok(())
}

pub fn settings(
    host: &str,
    runtime_hostname: &str,
    policy: ProvisioningPolicy,
    zone: &str,
) -> Result<Settings, Error> {
    skillet_hosts::profile_for_name(host)
        .filter(|p| p.supports_service("syncthing"))
        .ok_or(Error::Host)?;
    let input = Settings {
        host: host.into(),
        environment: policy.vault_name().into(),
        runtime_hostname: runtime_hostname.into(),
        device_name: format!("{runtime_hostname}.{zone}"),
    };
    input.validate()?;
    Ok(input)
}

pub fn deliver_settings(
    input: &Settings,
    guest: &impl GuestTransport,
    activation: ActivationPolicy,
) -> Result<(), Error> {
    input.validate()?;
    credential::install(
        guest,
        &input.host,
        initial::CREDENTIAL,
        initial::UNIT,
        activation,
        &input.payload()?,
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "syncthing_provisioning/tests.rs"]
mod tests;
