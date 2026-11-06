//! Use-or-create delivery of each host or disposable instance's own Joplin DB password.
use crate::{
    credential_delivery::{random_password, remote_credential_present, CredentialDeliveryError},
    provisioning_policy::{Environment, ProvisioningPolicy},
    vault::SecretStore,
};
use skillet_vm::{
    credential::{self, ActivationPolicy},
    transport::GuestTransport,
};
const CREDENTIAL: &str = skillet_joplin::DATABASE_PASSWORD_CREDENTIAL;

pub fn deliver_production(
    host: &str,
    store: &mut impl SecretStore,
    guest: &impl GuestTransport,
    activation: ActivationPolicy,
) -> Result<(), CredentialDeliveryError> {
    if !skillet_hosts::profile_for_name(host)
        .is_some_and(|profile| profile.supports_service("joplin"))
    {
        return Err(invalid("host does not declare Joplin"));
    }
    let path = format!("skillet/environments/prod/hosts/{host}/joplin/database-password");
    let password = use_or_create(&path, host, store, guest)?;
    credential::install(
        guest,
        host,
        CREDENTIAL,
        "skillet-joplin-apply.service",
        activation,
        password.as_bytes(),
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
            .is_some_and(|profile| profile.supports_service("joplin"))
        || !valid_component(instance)
    {
        return Err(invalid("invalid disposable Joplin identity"));
    }
    let path = format!(
        "skillet/environments/test/hosts/{host}/instances/{instance}/joplin/database-password"
    );
    let password = use_or_create(&path, host, store, guest)?;
    credential::install(
        guest,
        host,
        CREDENTIAL,
        "skillet-joplin-apply.service",
        ActivationPolicy::DeferConsumer,
        password.as_bytes(),
    )?;
    Ok(())
}

fn use_or_create(
    path: &str,
    host: &str,
    store: &mut impl SecretStore,
    guest: &impl GuestTransport,
) -> Result<String, CredentialDeliveryError> {
    if let Some(password) = store.get(path)? {
        if !valid_password(&password) {
            return Err(invalid("invalid Joplin database password"));
        }
        return Ok(password);
    }
    if remote_credential_present(host, CREDENTIAL, guest)? {
        return Err(invalid(
            "restore the missing Joplin database password from KeePassXC",
        ));
    }
    store.ensure_unchanged()?;
    let password = random_password()?;
    store.save_verified(path, &password)?;
    store.ensure_unchanged()?;
    Ok(password)
}

fn valid_password(value: &str) -> bool {
    (32..=256).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_graphic())
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
