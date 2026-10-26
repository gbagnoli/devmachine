//! Persist independent Arr keys before delivering a host/instance-bound payload.
use crate::{
    credential_delivery::{random_password, remote_credential_present, CredentialDeliveryError},
    provisioning_policy::{Environment, ProvisioningPolicy},
    vault::SecretStore,
};
use skillet_media::{App, Credentials, APPLY_UNIT, CREDENTIAL};
use skillet_vm::{
    credential::{self, ActivationPolicy},
    transport::{GuestCommand, GuestTransport},
};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Provider {
    putio_token: String,
}

pub fn deliver(
    host: &str,
    instance: Option<&str>,
    policy: ProvisioningPolicy,
    store: &mut impl SecretStore,
    guest: &impl GuestTransport,
    activation: ActivationPolicy,
) -> Result<(), CredentialDeliveryError> {
    let profile = skillet_hosts::profile_for_name(host)
        .filter(|p| p.media_config().is_some())
        .ok_or_else(|| invalid("host does not declare media"))?;
    let config = profile
        .media_config()
        .ok_or_else(|| invalid("missing media declaration"))?;
    let prefix = match (policy.environment(), instance) {
        (Environment::Production, None) => format!("skillet/environments/prod/hosts/{host}/media"),
        (Environment::Test, Some(instance))
            if skillet_vm::RunIdentity::new(host, instance).is_ok() =>
        {
            format!("skillet/environments/test/hosts/{host}/instances/{instance}/media")
        }
        _ => return Err(invalid("media environment and deployment identity differ")),
    };
    let rendered = Zeroizing::new(crate::configuration_templates::render_from_store(
        "media",
        host,
        policy.vault_name(),
        store,
    )?);
    let provider: Provider = serde_json::from_str(&rendered)
        .map_err(|_| invalid("invalid media provider configuration"))?;
    if provider.putio_token.is_empty()
        || provider
            .putio_token
            .bytes()
            .any(|b| b.is_ascii_whitespace())
    {
        return Err(invalid("invalid Put.io token"));
    }
    let mut keys = BTreeMap::new();
    for app in App::ARR {
        let path = format!("{prefix}/{}-api-key", app.name());
        let state_path = config.config_path(app);
        let key = use_or_create(&path, host, Some(&state_path), true, store, guest)?;
        keys.insert(app, key);
    }
    let password = use_or_create(
        &format!("{prefix}/putioarr-password"),
        host,
        Some(std::path::Path::new(config.config_root)),
        false,
        store,
        guest,
    )?;
    let input = Credentials {
        api_keys: keys,
        putio_password: password,
        putio_token: provider.putio_token,
    };
    input.validate()?;
    let payload = Zeroizing::new(
        serde_json::to_string(&input).map_err(|_| invalid("media serialization failed"))?,
    );
    store.ensure_unchanged()?;
    credential::install(
        guest,
        host,
        CREDENTIAL,
        APPLY_UNIT,
        activation,
        payload.as_bytes(),
    )?;
    Ok(())
}

fn use_or_create(
    path: &str,
    host: &str,
    state_path: Option<&std::path::Path>,
    key: bool,
    store: &mut impl SecretStore,
    guest: &impl GuestTransport,
) -> Result<String, CredentialDeliveryError> {
    if let Some(value) = store.get(path)? {
        if (key && !skillet_media::valid_key(&value))
            || (!key && !skillet_media::valid_password(&value))
        {
            return Err(invalid("invalid retained media credential"));
        }
        return Ok(value);
    }
    if remote_credential_present(host, CREDENTIAL, guest)? {
        return Err(invalid(
            "restore missing media keys from KeePassXC before reapplying",
        ));
    }
    if let Some(state_path) = state_path {
        let state = state_path
            .to_str()
            .ok_or_else(|| invalid("invalid media state path"))?;
        let output = guest.execute(
            &GuestCommand {
                program: "/usr/bin/sudo",
                arguments: &["-n", "test", "-e", state],
            },
            None,
        )?;
        match output.status.code() {
            Some(1) => {}
            Some(0) => {
                return Err(invalid(
                    "retained Arr configuration requires recovering its original key",
                ))
            }
            _ => return Err(invalid("media state inspection failed")),
        }
    }
    store.ensure_unchanged()?;
    let mut value = random_password()?;
    if key {
        value.truncate(32);
    }
    store.save_verified(path, &value)?;
    store.ensure_unchanged()?;
    Ok(value)
}

fn invalid(message: &str) -> CredentialDeliveryError {
    CredentialDeliveryError::Invalid(message.into())
}

#[cfg(test)]
#[path = "media_provisioning_tests.rs"]
mod tests;
