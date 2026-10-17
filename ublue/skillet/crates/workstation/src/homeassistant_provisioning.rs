//! Optional post-onboarding Home Assistant backup credentials.
use crate::{
    credential_delivery::{random_password, CredentialDeliveryError},
    provisioning_policy::{Environment, ProvisioningPolicy},
    vault::SecretStore,
};
use skillet_homeassistant::backup::{self, Settings};
use skillet_vm::{
    credential::{self, ActivationPolicy},
    transport::GuestTransport,
};
use zeroize::Zeroizing;

/// Absence means onboarding has not supplied an API token yet, not a failed host apply.
pub fn deliver(
    host: &str,
    policy: ProvisioningPolicy,
    store: &mut impl SecretStore,
    guest: &impl GuestTransport,
    activation: ActivationPolicy,
) -> Result<bool, CredentialDeliveryError> {
    if policy.environment() != Environment::Production
        || !skillet_hosts::profile_for_name(host)
            .is_some_and(|p| p.supports_service("homeassistant"))
    {
        return Err(CredentialDeliveryError::Invalid("Home Assistant backup delivery requires a declared production host; disposable credentials must be instance-owned".into()));
    }
    let prefix = format!("skillet/environments/prod/hosts/{host}/homeassistant");
    let Some(token) = store.get(&format!("{prefix}/api-token"))? else {
        return Ok(false);
    };
    let token = Zeroizing::new(token);
    if !backup::valid_token(&token) {
        return Err(CredentialDeliveryError::Invalid(
            "invalid Home Assistant API token".into(),
        ));
    }
    let key_path = format!("{prefix}/backup-password");
    let password = if let Some(value) = store.get(&key_path)? {
        value
    } else {
        if crate::credential_delivery::remote_credential_present(host, backup::CREDENTIAL, guest)? {
            return Err(CredentialDeliveryError::Invalid("restore the missing Home Assistant backup password from KeePassXC recovery; an installed credential already exists".into()));
        }
        store.ensure_unchanged()?;
        let generated = random_password()?;
        store.save_verified(&key_path, &generated)?;
        generated
    };
    let input = Settings {
        version: 1,
        host: host.into(),
        runtime_hostname: host.into(),
        api_token: token.to_string(),
        backup_password: password,
    };
    let payload = Zeroizing::new(serde_json::to_string(&input).map_err(|_| {
        CredentialDeliveryError::Invalid("invalid Home Assistant backup settings".into())
    })?);
    Settings::parse(&payload).map_err(|_| {
        CredentialDeliveryError::Invalid("invalid Home Assistant backup credential fields".into())
    })?;
    store.ensure_unchanged()?;
    credential::install(
        guest,
        host,
        backup::CREDENTIAL,
        backup::UNIT,
        activation,
        payload.as_bytes(),
    )?;
    Ok(true)
}
