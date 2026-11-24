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

/// Stage the optional Zigbee companion for the Home Assistant apply workflow.
pub fn deliver_zigbee(
    host: &str,
    policy: ProvisioningPolicy,
    vault: &mut crate::vault::Vault,
    key_file: Option<&std::path::Path>,
    guest: &impl GuestTransport,
    activation: ActivationPolicy,
) -> Result<bool, CredentialDeliveryError> {
    use skillet_homeassistant::zigbee;

    if policy.environment() != Environment::Production {
        return Ok(false);
    }
    let Some(coordinator) = zigbee_coordinator(host, vault)? else {
        return Ok(false);
    };
    let mut store = crate::vault::VaultSecretStore::new(vault, key_file);
    let previous =
        crate::credential_delivery::remote_credential_present(host, zigbee::CREDENTIAL, guest)?;
    let mut password = |user: &str| -> Result<String, CredentialDeliveryError> {
        let key = format!("skillet/hosts/{host}/homeassistant/mqtt/{user}");
        match store.get(&key)? {
            Some(value) if zigbee::valid_password(&value) => Ok(value),
            Some(_) => Err(CredentialDeliveryError::Invalid(format!(
                "invalid MQTT password in {key}"
            ))),
            None if previous => Err(CredentialDeliveryError::Invalid(format!(
                "restore the missing MQTT password at {key} from KeePassXC recovery"
            ))),
            None => {
                store.ensure_unchanged()?;
                let generated = random_password()?;
                store.save_verified(&key, &generated)?;
                Ok(generated)
            }
        }
    };
    let settings = zigbee::Settings {
        coordinator,
        ha_password: password("homeassistant")?,
        zigbee_password: password("zigbee2mqtt")?,
    };
    let payload = Zeroizing::new(serde_json::to_vec(&settings).map_err(|_| {
        CredentialDeliveryError::Invalid("could not serialize Zigbee settings".into())
    })?);
    store.ensure_unchanged()?;
    credential::install(
        guest,
        host,
        zigbee::CREDENTIAL,
        zigbee::UNIT,
        activation,
        &payload,
    )?;
    Ok(true)
}

/// Read and validate the optional coordinator before provisioning services or DNS.
pub fn zigbee_coordinator(
    host: &str,
    vault: &crate::vault::Vault,
) -> Result<Option<std::net::SocketAddrV4>, CredentialDeliveryError> {
    use skillet_homeassistant::zigbee;
    let path = format!("skillet/hosts/{host}/network/zigbee2mqtt");
    let address = vault.get_field(&path, "address")?;
    let port = vault.get_field(&path, "port")?;
    let adapter = vault.get_field(&path, "adapter")?;
    let (address, port, adapter) = match (address, port, adapter) {
        (None, None, None) => return Ok(None),
        (Some(address), Some(port), Some(adapter)) => (address, port, adapter),
        _ => {
            return Err(CredentialDeliveryError::Invalid(format!(
                "incomplete Zigbee coordinator entry {path}"
            )))
        }
    };
    let endpoint = format!("{address}:{port}");
    if adapter != "zstack" || !zigbee::valid_address(&endpoint) {
        return Err(CredentialDeliveryError::Invalid(format!(
            "invalid Zigbee coordinator entry {path}"
        )));
    }
    let coordinator = endpoint.parse().map_err(|_| {
        CredentialDeliveryError::Invalid(format!("invalid Zigbee coordinator address in {path}"))
    })?;
    Ok(Some(coordinator))
}
