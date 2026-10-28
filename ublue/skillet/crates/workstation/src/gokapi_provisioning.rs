//! Delivery of explicitly selected Gokapi account ownership, never app data.
use crate::{
    credential_delivery::CredentialDeliveryError, provisioning_policy::ProvisioningPolicy,
    vault::SecretStore,
};
use skillet_vm::{
    credential::{self, ActivationPolicy},
    transport::GuestTransport,
};
pub fn deliver(
    host: &str,
    policy: ProvisioningPolicy,
    store: &impl SecretStore,
    guest: &impl GuestTransport,
    activation: ActivationPolicy,
) -> Result<(), CredentialDeliveryError> {
    let invalid = |message: &str| CredentialDeliveryError::Invalid(message.into());
    if !skillet_hosts::profile_for_name(host)
        .is_some_and(|profile| profile.supports_service("gokapi"))
    {
        return Err(invalid("host does not declare Gokapi"));
    }
    let path = format!(
        "skillet/environments/{}/hosts/{host}/gokapi/admin-user",
        policy.vault_name()
    );
    let admin=store.get(&path)?.ok_or_else(||invalid(&format!("KeePassXC entry missing: {path}; Password must contain one allowed Tailscale LoginName for Gokapi's initial super-admin")))?;
    let admin = admin.trim();
    let users = store
        .get(skillet_caddy::auth::USERS_ENTRY)?
        .ok_or_else(|| invalid("Tailscale UI allowlist missing"))?;
    let users = skillet_caddy::auth::Policy::parse_users(&users)
        .map_err(|_| invalid("invalid Tailscale UI allowlist"))?;
    if !skillet_gokapi::valid_user(admin) || !users.iter().any(|user| user == admin) {
        return Err(invalid("Gokapi initial admin must be one complete lower-case identity from the Tailscale UI allowlist"));
    }
    credential::install(
        guest,
        host,
        skillet_gokapi::ADMIN_CREDENTIAL,
        skillet_gokapi::APPLY_UNIT,
        activation,
        admin.as_bytes(),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests;
