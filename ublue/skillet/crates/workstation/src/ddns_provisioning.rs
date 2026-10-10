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
    State(#[from] crate::provisioning_state::ProvisioningStateError),
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
        lifetime: Option<std::time::Duration>,
    ) -> Result<IssuedToken, CloudflareError>;
    fn revoke(&self, creator: &str, account: &str, id: &str) -> Result<(), CloudflareError>;
    fn replace(
        &self,
        creator: &str,
        zone: &str,
        account: &str,
        name: &str,
        lifetime: Option<std::time::Duration>,
    ) -> Result<IssuedToken, CloudflareError>;
    fn owned_test_records(
        &self,
        token: &str,
        zone: &str,
        marker: &str,
        names: &[String],
    ) -> Result<Vec<RecordRef>, CloudflareError>;
    fn remove_test_records(
        &self,
        token: &str,
        zone: &str,
        marker: &str,
        names: &[String],
        ids: &[String],
    ) -> Result<(), CloudflareError>;
    fn token_ids(
        &self,
        creator: &str,
        account: &str,
        name: &str,
    ) -> Result<Vec<String>, CloudflareError>;
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
        lifetime: Option<std::time::Duration>,
    ) -> Result<IssuedToken, CloudflareError> {
        self.create_zone_token(creator, zone, account, name, lifetime)
    }
    fn revoke(&self, creator: &str, account: &str, id: &str) -> Result<(), CloudflareError> {
        self.revoke_token(creator, account, id)
    }
    fn replace(
        &self,
        creator: &str,
        zone: &str,
        account: &str,
        name: &str,
        lifetime: Option<std::time::Duration>,
    ) -> Result<IssuedToken, CloudflareError> {
        self.replace_named_zone_token(creator, zone, account, name, lifetime)
    }
    fn owned_test_records(
        &self,
        token: &str,
        zone: &str,
        marker: &str,
        names: &[String],
    ) -> Result<Vec<RecordRef>, CloudflareError> {
        self.list_owned_ddns_records(token, zone, marker, names)
    }
    fn remove_test_records(
        &self,
        token: &str,
        zone: &str,
        marker: &str,
        names: &[String],
        ids: &[String],
    ) -> Result<(), CloudflareError> {
        self.remove_owned_ddns_records(token, zone, marker, names, ids)
    }
    fn token_ids(
        &self,
        creator: &str,
        account: &str,
        name: &str,
    ) -> Result<Vec<String>, CloudflareError> {
        self.token_ids_by_name(creator, account, name)
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
        || {
            provider.issue(
                request.creator_token,
                request.zone_id,
                account,
                &token_name,
                None,
            )
        },
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

pub struct DisposableDdnsRequest<'a> {
    pub host: &'a str,
    pub instance: &'a str,
    pub policy: ProvisioningPolicy,
    pub zone_id: &'a str,
    pub relative_ui_domain: Option<&'a str>,
    pub config: &'a str,
    pub creator_token: &'a str,
    pub run_directory: &'a std::path::Path,
}

struct DisposableDdnsPlan {
    config: PrivateConfig,
    account: String,
    marker: String,
    token_name: String,
    cleanup_token_name: String,
    record_names: Vec<String>,
}

/// Journal first, mint a bounded test token, then activate the existing guest
/// DDNS writer and wait for its allowlisted marked A records to appear.
pub fn provision_disposable_ddns(
    request: &DisposableDdnsRequest<'_>,
    provider: &impl DdnsProvider,
    guest: &impl GuestTransport,
    timeout: std::time::Duration,
    interval: std::time::Duration,
) -> Result<(), DdnsProvisioningError> {
    if request.policy.environment() != Environment::Test {
        return Err(DdnsProvisioningError::Invalid(
            "disposable DDNS requires test policy",
        ));
    }
    skillet_hosts::profile_for_name(request.host)
        .filter(|profile| profile.supports_service("ddns"))
        .ok_or(DdnsProvisioningError::Invalid("host does not declare DDNS"))?;
    let plan = disposable_ddns_plan(request, provider)?;
    let path = request.run_directory.join("ddns.json");
    let mut ownership = load_or_create_ddns_ownership(request, &plan, &path)?;
    let token = provider.replace(
        request.creator_token,
        request.zone_id,
        &plan.account,
        &plan.token_name,
        request.policy.cloudflare_token_lifetime(),
    )?;
    ownership.token_id = Some(token.id);
    ownership.expires_on = token.expires_on;
    crate::provisioning_state::save_ddns_ownership(&path, &ownership)?;
    activate_disposable_ddns(
        request,
        &plan,
        &token.value,
        &mut ownership,
        &path,
        provider,
        guest,
        timeout,
        interval,
    )
}

fn disposable_ddns_plan(
    request: &DisposableDdnsRequest<'_>,
    provider: &impl DdnsProvider,
) -> Result<DisposableDdnsPlan, DdnsProvisioningError> {
    let config = PrivateConfig::parse(request.config)?;
    crate::cloudflare::validate_zone_id(request.zone_id)?;
    config.payload(request.zone_id, "validation-placeholder")?;
    let zone = provider.zone(request.creator_token, request.zone_id)?;
    if zone.id != request.zone_id {
        return Err(DdnsProvisioningError::Invalid(
            "provider zone identity mismatch",
        ));
    }
    config.validate_zone(&zone.name, &[])?;
    let ui_domain = skillet_caddy::resolve_ui_domain(&zone.name, request.relative_ui_domain)?;
    if config.records.iter().any(|record| {
        let name = format!("{}.{zone}", record.name, zone = zone.name);
        name == ui_domain || name.ends_with(&format!(".{ui_domain}"))
    }) {
        return Err(ConfigError::Overlap.into());
    }
    let account = Cloudflare::account_id(&zone)?;
    let marker = format!("skillet-ddns:test:{}:{}", request.host, request.instance);
    let token_name = format!("skillet:test:{}-{}:ddns", request.host, request.instance);
    let cleanup_token_name = format!("{token_name}:cleanup");
    let record_names = config
        .records
        .iter()
        .map(|record| format!("{}.{zone}", record.name, zone = zone.name))
        .collect::<Vec<_>>();
    Ok(DisposableDdnsPlan {
        config,
        account: account.to_string(),
        marker,
        token_name,
        cleanup_token_name,
        record_names,
    })
}

fn load_or_create_ddns_ownership(
    request: &DisposableDdnsRequest<'_>,
    plan: &DisposableDdnsPlan,
    path: &std::path::Path,
) -> Result<crate::provisioning_state::DdnsVmOwnership, DdnsProvisioningError> {
    let ownership = if crate::provisioning_state::ddns_ownership_exists(path)? {
        let saved = crate::provisioning_state::load_ddns_ownership(path)?;
        if saved.identity
            != crate::provisioning_state::ProvisioningIdentity::new(
                request.host,
                request.policy.name(),
                request.instance,
            )
            || saved.zone_id != request.zone_id
            || saved.marker != plan.marker
            || saved.token_name != plan.token_name
            || saved.record_names != plan.record_names
            || saved.cleanup_token_name != plan.cleanup_token_name
        {
            return Err(DdnsProvisioningError::Invalid(
                "DDNS journal does not match this VM/configuration",
            ));
        }
        saved
    } else {
        let value = crate::provisioning_state::DdnsVmOwnership {
            identity: crate::provisioning_state::ProvisioningIdentity::new(
                request.host,
                request.policy.name(),
                request.instance,
            ),
            zone_id: request.zone_id.into(),
            marker: plan.marker.clone(),
            token_name: plan.token_name.clone(),
            token_id: None,
            expires_on: None,
            record_names: plan.record_names.clone(),
            record_ids: Vec::new(),
            cleanup_token_name: plan.cleanup_token_name.clone(),
            cleanup_token_id: None,
        };
        crate::provisioning_state::save_ddns_ownership(path, &value)?;
        value
    };
    Ok(ownership)
}

#[allow(clippy::too_many_arguments)]
fn activate_disposable_ddns(
    request: &DisposableDdnsRequest<'_>,
    plan: &DisposableDdnsPlan,
    token: &str,
    ownership: &mut crate::provisioning_state::DdnsVmOwnership,
    path: &std::path::Path,
    provider: &impl DdnsProvider,
    guest: &impl GuestTransport,
    timeout: std::time::Duration,
    interval: std::time::Duration,
) -> Result<(), DdnsProvisioningError> {
    let existing =
        provider.owned_test_records(token, request.zone_id, &plan.marker, &plan.record_names)?;
    let all_records = provider.records(token, request.zone_id)?;
    for name in &plan.record_names {
        if all_records.iter().any(|record| {
            record.name.eq_ignore_ascii_case(name)
                && record.comment.as_deref() != Some(plan.marker.as_str())
        }) {
            return Err(DdnsProvisioningError::Invalid(
                "test DDNS record name is already in use",
            ));
        }
    }
    ownership.record_ids = existing.iter().map(|record| record.id.clone()).collect();
    crate::provisioning_state::save_ddns_ownership(path, ownership)?;
    let payload = plan
        .config
        .payload(request.zone_id, token)?
        .with_record_comment(&plan.marker)?
        .render()?;
    credential::install(
        guest,
        request.host,
        skillet_ddns::CREDENTIAL,
        "skillet-ddns-apply.service",
        credential::ActivationPolicy::StartConsumer,
        payload.as_bytes(),
    )?;
    wait_for_disposable_ddns(
        request, plan, token, ownership, path, provider, timeout, interval,
    )
}

#[allow(clippy::too_many_arguments)]
fn wait_for_disposable_ddns(
    request: &DisposableDdnsRequest<'_>,
    plan: &DisposableDdnsPlan,
    token: &str,
    ownership: &mut crate::provisioning_state::DdnsVmOwnership,
    path: &std::path::Path,
    provider: &impl DdnsProvider,
    timeout: std::time::Duration,
    interval: std::time::Duration,
) -> Result<(), DdnsProvisioningError> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let records = provider.owned_test_records(
            token,
            request.zone_id,
            &plan.marker,
            &plan.record_names,
        )?;
        let ready = plan.record_names.iter().all(|name| {
            records.iter().any(|record| {
                record.name.eq_ignore_ascii_case(name) && public_ipv4(&record.content)
            })
        });
        if ready {
            ownership.record_ids = records.iter().map(|record| record.id.clone()).collect();
            crate::provisioning_state::save_ddns_ownership(path, ownership)?;
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(DdnsProvisioningError::Invalid(
                "timed out waiting for marked public DDNS records; ownership journal retained",
            ));
        }
        std::thread::sleep(interval);
    }
}

fn public_ipv4(value: &str) -> bool {
    let Ok(ip) = value.parse::<std::net::Ipv4Addr>() else {
        return false;
    };
    let octets = ip.octets();
    let documentation = (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
        || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
        || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113);
    let special = octets[0] == 0
        || (octets[0] == 192 && octets[1] == 0 && octets[2] == 0)
        || (octets[0] == 192 && octets[1] == 88 && octets[2] == 99)
        || (octets[0] == 198 && (18..=19).contains(&octets[1]))
        || octets[0] >= 240;
    !documentation
        && !special
        && !ip.is_private()
        && !ip.is_loopback()
        && !ip.is_link_local()
        && !ip.is_multicast()
        && !ip.is_broadcast()
        && !ip.is_unspecified()
        && !(octets[0] == 100 && (64..=127).contains(&octets[1]))
}

pub struct DisposableDdnsCleanup<'a> {
    pub host: &'a str,
    pub instance: &'a str,
    pub policy: ProvisioningPolicy,
    pub ownership_path: &'a std::path::Path,
    pub configured_zone_id: &'a str,
    pub creator_token: &'a str,
}

pub fn cleanup_disposable_ddns(
    request: &DisposableDdnsCleanup<'_>,
    provider: &impl DdnsProvider,
) -> Result<bool, DdnsProvisioningError> {
    if request.policy.environment() != Environment::Test {
        return Err(DdnsProvisioningError::Invalid(
            "DDNS cleanup requires test policy",
        ));
    }
    if !crate::provisioning_state::ddns_ownership_exists(request.ownership_path)? {
        return Ok(false);
    }
    let mut owner = crate::provisioning_state::load_ddns_ownership(request.ownership_path)?;
    let expected = crate::provisioning_state::ProvisioningIdentity::new(
        request.host,
        request.policy.name(),
        request.instance,
    );
    let token_name = format!("skillet:test:{}-{}:ddns", request.host, request.instance);
    if owner.identity != expected
        || owner.zone_id != request.configured_zone_id
        || owner.marker != format!("skillet-ddns:test:{}:{}", request.host, request.instance)
        || owner.token_name != token_name
        || owner.cleanup_token_name != format!("{token_name}:cleanup")
        || owner.record_names.is_empty()
    {
        return Err(DdnsProvisioningError::Invalid(
            "DDNS ownership journal does not match this VM and environment",
        ));
    }
    let zone = provider.zone(request.creator_token, &owner.zone_id)?;
    if zone.id != owner.zone_id {
        return Err(DdnsProvisioningError::Invalid(
            "provider zone identity mismatch",
        ));
    }
    let account = Cloudflare::account_id(&zone)?;
    let cleanup = provider.replace(
        request.creator_token,
        &owner.zone_id,
        account,
        &owner.cleanup_token_name,
        Some(request.policy.cleanup_token_lifetime()),
    )?;
    owner.cleanup_token_id = Some(cleanup.id);
    crate::provisioning_state::save_ddns_ownership(request.ownership_path, &owner)?;
    provider.zone(&cleanup.value, &owner.zone_id)?;
    provider.remove_test_records(
        &cleanup.value,
        &owner.zone_id,
        &owner.marker,
        &owner.record_names,
        &owner.record_ids,
    )?;
    for id in provider.token_ids(request.creator_token, account, &owner.token_name)? {
        provider.revoke(request.creator_token, account, &id)?;
    }
    for id in provider.token_ids(request.creator_token, account, &owner.cleanup_token_name)? {
        provider.revoke(request.creator_token, account, &id)?;
    }
    crate::provisioning_state::remove_ddns_ownership(request.ownership_path)?;
    Ok(true)
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
