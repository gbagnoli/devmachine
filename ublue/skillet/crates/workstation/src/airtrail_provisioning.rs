//! Use-or-create delivery for `AirTrail` database and optional provider credentials.
use crate::{
    credential_delivery::{random_password, remote_credential_present, CredentialDeliveryError},
    provisioning_policy::{Environment, ProvisioningPolicy},
    vault::SecretStore,
};
use skillet_vm::{
    credential::{self, ActivationPolicy},
    transport::GuestTransport,
};
use zeroize::Zeroizing;

const DATABASE_CREDENTIAL: &str = skillet_airtrail::DATABASE_PASSWORD_CREDENTIAL;
const INTEGRATIONS_CREDENTIAL: &str = skillet_airtrail::INTEGRATIONS_CREDENTIAL;

pub fn deliver_production(
    host: &str,
    store: &mut impl SecretStore,
    guest: &impl GuestTransport,
    activation: ActivationPolicy,
) -> Result<(), CredentialDeliveryError> {
    if !skillet_hosts::profile_for_name(host)
        .is_some_and(|profile| profile.supports_service("airtrail"))
    {
        return Err(invalid("host does not declare AirTrail"));
    }
    let prefix = format!("skillet/environments/prod/hosts/{host}/airtrail");
    let password = use_or_create(&format!("{prefix}/database-password"), host, store, guest)?;
    let integrations = integrations_payload(store, &prefix)?;
    store.ensure_unchanged()?;
    credential::install_set(
        guest,
        host,
        "skillet-airtrail-apply.service",
        activation,
        &[
            (DATABASE_CREDENTIAL, password.as_bytes()),
            (INTEGRATIONS_CREDENTIAL, integrations.as_bytes()),
        ],
    )?;
    Ok(())
}

pub fn deliver_disposable(
    host: &str,
    instance: &str,
    policy: ProvisioningPolicy,
    store: &mut impl SecretStore,
    guest: &impl GuestTransport,
) -> Result<(), CredentialDeliveryError> {
    if policy.environment() != Environment::Test
        || !skillet_hosts::profile_for_name(host)
            .is_some_and(|profile| profile.supports_service("airtrail"))
        || !valid_component(instance)
    {
        return Err(invalid("invalid disposable AirTrail identity"));
    }
    let prefix = format!("skillet/environments/test/hosts/{host}/instances/{instance}/airtrail");
    let password = use_or_create(&format!("{prefix}/database-password"), host, store, guest)?;
    // External provider keys are deliberately not copied into smoke VMs.
    store.ensure_unchanged()?;
    credential::install_set(
        guest,
        host,
        "skillet-airtrail-apply.service",
        ActivationPolicy::DeferConsumer,
        &[
            (DATABASE_CREDENTIAL, password.as_bytes()),
            (INTEGRATIONS_CREDENTIAL, b"{}"),
        ],
    )?;
    Ok(())
}

fn integrations_payload(
    store: &mut impl SecretStore,
    prefix: &str,
) -> Result<Zeroizing<String>, CredentialDeliveryError> {
    let aerodatabox = store.get(&format!("{prefix}/aerodatabox-api-key"))?;
    let openaip = store.get(&format!("{prefix}/openaip-api-key"))?;
    for value in [aerodatabox.as_deref(), openaip.as_deref()]
        .into_iter()
        .flatten()
    {
        if value.is_empty() || value.bytes().any(|byte| byte.is_ascii_whitespace()) {
            return Err(invalid("invalid AirTrail integration credential"));
        }
    }
    let payload = serde_json::to_string(&skillet_airtrail::Integrations {
        aerodatabox,
        openaip,
    })
    .map_err(|_| invalid("invalid AirTrail integration settings"))?;
    skillet_airtrail::Integrations::parse(&payload)
        .map_err(|_| invalid("invalid AirTrail integration settings"))?;
    Ok(Zeroizing::new(payload))
}

fn use_or_create(
    path: &str,
    host: &str,
    store: &mut impl SecretStore,
    guest: &impl GuestTransport,
) -> Result<Zeroizing<String>, CredentialDeliveryError> {
    if let Some(password) = store.get(path)? {
        if !valid_password(&password) {
            return Err(invalid("invalid AirTrail database password"));
        }
        return Ok(Zeroizing::new(password));
    }
    if remote_credential_present(host, DATABASE_CREDENTIAL, guest)? {
        return Err(invalid(
            "restore the missing AirTrail database password from KeePassXC",
        ));
    }
    store.ensure_unchanged()?;
    let password = Zeroizing::new(random_password()?);
    store.save_verified(path, &password)?;
    store.ensure_unchanged()?;
    Ok(password)
}

fn valid_password(value: &str) -> bool {
    (32..=256).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 63
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn invalid(message: &str) -> CredentialDeliveryError {
    CredentialDeliveryError::Invalid(message.into())
}

#[cfg(test)]
#[path = "airtrail_provisioning_tests.rs"]
mod tests;
