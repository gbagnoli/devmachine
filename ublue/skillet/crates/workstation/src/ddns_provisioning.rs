//! Persistent DDNS credential issuance and encrypted guest delivery.
use crate::{
    cloudflare::{Cloudflare, CloudflareError, IssuedToken, RecordRef, Zone},
    provisioning_policy::{Environment, ProvisioningPolicy},
    vault::{SecretStore, VaultError},
};
use skillet_ddns::config::{ConfigError, PrivateConfig};
use skillet_vm::{credential, transport::GuestTransport};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DdnsProvisioningError {
    #[error("invalid DDNS provisioning: {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error(transparent)]
    Cloudflare(#[from] CloudflareError),
    #[error(transparent)]
    Token(#[from] crate::zone_token::TokenError),
    #[error(transparent)]
    Guest(#[from] skillet_vm::Error),
    #[error(transparent)]
    Caddy(#[from] skillet_caddy::CaddyError),
}

/// Provider operations needed for persistent public-address updates.
/// DNS is observed here; the guest updater is the sole continuing writer.
pub trait DdnsProvider {
    fn zone(&self, token: &str, zone: &str) -> Result<Zone, CloudflareError>;
    fn records(&self, token: &str, zone: &str) -> Result<Vec<RecordRef>, CloudflareError>;
    fn issue(
        &self,
        creator: &str,
        zone: &str,
        account: &str,
        name: &str,
    ) -> Result<IssuedToken, CloudflareError>;
    fn revoke(&self, creator: &str, account: &str, id: &str) -> Result<(), CloudflareError>;
}

impl DdnsProvider for Cloudflare {
    fn zone(&self, token: &str, zone: &str) -> Result<Zone, CloudflareError> {
        Self::zone(self, token, zone)
    }
    fn records(&self, token: &str, zone: &str) -> Result<Vec<RecordRef>, CloudflareError> {
        self.list_records(token, zone)
    }
    fn issue(
        &self,
        creator: &str,
        zone: &str,
        account: &str,
        name: &str,
    ) -> Result<IssuedToken, CloudflareError> {
        self.replace_named_zone_token(creator, zone, account, name, None)
    }
    fn revoke(&self, creator: &str, account: &str, id: &str) -> Result<(), CloudflareError> {
        self.revoke_token(creator, account, id)
    }
}

pub struct DdnsDelivery<'a> {
    pub host: &'a str,
    pub policy: ProvisioningPolicy,
    pub zone_id: &'a str,
    pub relative_ui_domain: Option<&'a str>,
    pub config: &'a str,
    pub creator_token: &'a str,
}

pub fn deliver_persistent_ddns(
    request: &DdnsDelivery<'_>,
    store: &mut impl SecretStore,
    provider: &impl DdnsProvider,
    guest: &impl GuestTransport,
) -> Result<(), DdnsProvisioningError> {
    if request.policy.environment() != Environment::Production {
        return Err(DdnsProvisioningError::Invalid(
            "disposable DDNS requires its own ownership and cleanup scenario; not enabled yet",
        ));
    }
    skillet_hosts::profile_for_name(request.host)
        .filter(|profile| profile.supports_service("ddns"))
        .ok_or(DdnsProvisioningError::Invalid("host does not declare DDNS"))?;
    let config = PrivateConfig::parse(request.config)?;
    crate::cloudflare::validate_zone_id(request.zone_id)?;
    // Validate credentials before issuance as well as at the receiving host.
    config.payload(request.zone_id, "validation-placeholder")?;
    let zone = provider.zone(request.creator_token, request.zone_id)?;
    if zone.id != request.zone_id {
        return Err(DdnsProvisioningError::Invalid(
            "provider zone identity mismatch",
        ));
    }
    let ui_domain = skillet_caddy::resolve_ui_domain(&zone.name, request.relative_ui_domain)?;
    config.validate_zone(&zone.name, &[])?;
    // Reserve the complete UI namespace, including aliases and other hosts.
    // Its naming rules remain owned by Caddy, rather than copied here.
    if config.records.iter().any(|record| {
        let name = format!("{}.{name}", record.name, name = zone.name);
        name == ui_domain || name.ends_with(&format!(".{ui_domain}"))
    }) {
        return Err(ConfigError::Overlap.into());
    }
    let account = Cloudflare::account_id(&zone)?;
    let token_path = format!(
        "skillet/environments/{}/hosts/{}/cloudflare/ddns-token",
        request.policy.vault_name(),
        request.host
    );
    let token_name = format!("skillet:{}:{}:ddns", request.policy.name(), request.host);
    let token = crate::zone_token::use_or_create(
        store,
        &token_path,
        || provider.issue(request.creator_token, request.zone_id, account, &token_name),
        |id| provider.revoke(request.creator_token, account, id),
    )?;
    let child_zone = provider.zone(&token, request.zone_id)?;
    if child_zone.id != zone.id || child_zone.name != zone.name {
        return Err(DdnsProvisioningError::Invalid(
            "child token zone identity mismatch",
        ));
    }
    let existing = provider.records(&token, request.zone_id)?;
    let marker = format!("skillet-ddns:{}:{}", request.policy.name(), request.host);
    validate_takeover(&config, &zone.name, &marker, &existing)?;
    store.ensure_unchanged()?;
    let payload = config
        .payload(request.zone_id, &token)?
        .with_record_comment(&marker)?
        .render()?;
    credential::install(
        guest,
        request.host,
        skillet_ddns::CREDENTIAL,
        "skillet-ddns-apply.service",
        credential::ActivationPolicy::StartConsumer,
        payload.as_bytes(),
    )?;
    Ok(())
}

fn validate_takeover(
    config: &PrivateConfig,
    zone: &str,
    marker: &str,
    existing: &[RecordRef],
) -> Result<(), DdnsProvisioningError> {
    // The pinned legacy updater fetches one page of at most 100 A records.
    // Refuse a zone it cannot fully observe instead of risking duplicates.
    let existing_a = existing
        .iter()
        .filter(|record| record.record_type == "A")
        .count();
    let new_a = config
        .records
        .iter()
        .filter(|record| {
            let name = format!("{}.{zone}", record.name);
            !existing
                .iter()
                .any(|entry| entry.record_type == "A" && entry.name.eq_ignore_ascii_case(&name))
        })
        .count();
    if existing_a + new_a > 100 {
        return Err(DdnsProvisioningError::Invalid(
            "selected updater supports at most 100 zone A records",
        ));
    }
    for record in &config.records {
        let name = format!("{}.{zone}", record.name);
        let matches = existing
            .iter()
            .filter(|entry| entry.name.eq_ignore_ascii_case(&name))
            .collect::<Vec<_>>();
        // Require explicit adoption on first activation and every retry. A UI
        // ownership marker or conflicting record type must never be adopted.
        if !matches.is_empty()
            && ((!config.takeover_existing && matches[0].comment.as_deref() != Some(marker))
                || matches.len() != 1
                || matches[0].record_type != "A"
                || matches[0].comment.as_deref().is_some_and(|comment| {
                    comment.starts_with("skillet:")
                        || (comment.starts_with("skillet-ddns:") && comment != marker)
                }))
        {
            return Err(DdnsProvisioningError::Invalid("record collision: explicit public A-record takeover required; UI-owned and conflicting records cannot be adopted"));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "ddns_provisioning/tests.rs"]
mod tests;
