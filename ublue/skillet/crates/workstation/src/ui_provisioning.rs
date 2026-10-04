//! Shared host/environment UI hostname, Caddy, and DNS planning.

use crate::{
    cloudflare::{self, CloudflareError, DesiredRecord, IssuedToken, OwnedDns, Zone},
    provisioning_policy::{Environment, ProvisioningPolicy},
    provisioning_state::{self, CloudflareVmOwnership, ProvisioningIdentity},
    vault::VaultError,
};
use skillet_caddy::{CaddyError, CaddySites, UiEnvironment};
use std::{collections::BTreeSet, path::Path, time::Duration};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UiProvisioningError {
    #[error("host profile {0} declares no private UI services")]
    NoUiServices(String),
    #[error(transparent)]
    Caddy(#[from] CaddyError),
    #[error(transparent)]
    Cloudflare(#[from] CloudflareError),
    #[error(transparent)]
    State(#[from] provisioning_state::ProvisioningStateError),
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error("invalid disposable UI provisioning request: {0}")]
    Invalid(String),
}

/// Desired public DNS and Caddy configuration for one declared host profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiProvisioningPlan {
    pub sites: CaddySites,
    pub dns_records: Vec<DesiredRecord>,
}

/// The narrow Cloudflare operations used by disposable UI provisioning.
pub trait UiCloudflareProvider {
    fn zone(&self, token: &str, zone_id: &str) -> Result<Zone, CloudflareError>;
    fn create_zone_token(
        &self,
        creator_token: &str,
        zone_id: &str,
        account_id: &str,
        name: &str,
        lifetime: Option<Duration>,
    ) -> Result<IssuedToken, CloudflareError>;
    fn reconcile_dns(
        &self,
        token: &str,
        zone_id: &str,
        marker: &str,
        ui_domain: &str,
        desired: &[DesiredRecord],
    ) -> Result<OwnedDns, CloudflareError>;
    fn remove_dns_marker(
        &self,
        token: &str,
        zone_id: &str,
        marker: &str,
        ui_domain: &str,
    ) -> Result<(), CloudflareError>;
    fn token_ids_by_name(
        &self,
        creator_token: &str,
        account_id: &str,
        name: &str,
    ) -> Result<Vec<String>, CloudflareError>;
    fn revoke_token(
        &self,
        creator_token: &str,
        account_id: &str,
        token_id: &str,
    ) -> Result<(), CloudflareError>;
}

impl UiCloudflareProvider for cloudflare::Cloudflare {
    fn zone(&self, token: &str, zone_id: &str) -> Result<Zone, CloudflareError> {
        cloudflare::Cloudflare::zone(self, token, zone_id)
    }

    fn create_zone_token(
        &self,
        creator_token: &str,
        zone_id: &str,
        account_id: &str,
        name: &str,
        lifetime: Option<Duration>,
    ) -> Result<IssuedToken, CloudflareError> {
        cloudflare::Cloudflare::create_zone_token(
            self,
            creator_token,
            zone_id,
            account_id,
            name,
            lifetime,
        )
    }

    fn reconcile_dns(
        &self,
        token: &str,
        zone_id: &str,
        marker: &str,
        ui_domain: &str,
        desired: &[DesiredRecord],
    ) -> Result<OwnedDns, CloudflareError> {
        cloudflare::Cloudflare::reconcile_dns(self, token, zone_id, marker, ui_domain, desired)
    }

    fn remove_dns_marker(
        &self,
        token: &str,
        zone_id: &str,
        marker: &str,
        ui_domain: &str,
    ) -> Result<(), CloudflareError> {
        cloudflare::Cloudflare::remove_dns_marker(self, token, zone_id, marker, ui_domain)
    }

    fn token_ids_by_name(
        &self,
        creator_token: &str,
        account_id: &str,
        name: &str,
    ) -> Result<Vec<String>, CloudflareError> {
        cloudflare::Cloudflare::token_ids_by_name(self, creator_token, account_id, name)
    }

    fn revoke_token(
        &self,
        creator_token: &str,
        account_id: &str,
        token_id: &str,
    ) -> Result<(), CloudflareError> {
        cloudflare::Cloudflare::revoke_token(self, creator_token, account_id, token_id)
    }
}

pub struct DisposableUiRequest<'a> {
    pub host: &'a str,
    pub instance: &'a str,
    pub policy: ProvisioningPolicy,
    pub zone_id: &'a str,
    pub relative_ui_domain: Option<&'a str>,
    pub creator_token: &'a str,
    pub run_directory: &'a Path,
    pub addresses: &'a BTreeSet<String>,
}

pub struct DisposableUiCredentials {
    pub sites: CaddySites,
    pub token: IssuedToken,
    pub account_id: String,
    pub token_name: String,
}

pub struct DisposableUiCleanupRequest<'a> {
    pub host: &'a str,
    pub instance: &'a str,
    pub policy: ProvisioningPolicy,
    pub ownership_path: std::path::PathBuf,
    pub configured_zone_id: &'a str,
    pub relative_ui_domain: Option<&'a str>,
    pub creator_token: &'a str,
}

/// Remove only DNS and tokens owned by the typed disposable VM journal. The
/// journal remains until provider cleanup and temporary-token revocation pass.
pub fn cleanup_disposable_ui(
    request: &DisposableUiCleanupRequest<'_>,
    provider: &impl UiCloudflareProvider,
) -> Result<bool, UiProvisioningError> {
    if request.policy.environment() != Environment::Test {
        return Err(UiProvisioningError::Invalid(
            "disposable UI cleanup requires the test environment policy".into(),
        ));
    }
    if !provisioning_state::cloudflare_ownership_exists(&request.ownership_path)? {
        return Ok(false);
    }
    let ownership = provisioning_state::load_cloudflare_ownership(&request.ownership_path)?;
    let expected_identity =
        ProvisioningIdentity::new(request.host, request.policy.name(), request.instance);
    provisioning_state::validate_cloudflare_identity(&ownership, &expected_identity)?;
    let expected_marker = format!(
        "skillet:{}:{}:{}",
        request.policy.name(),
        request.host,
        request.instance
    );
    let expected_token_name = format!(
        "skillet:{}:{}-{}",
        request.policy.name(),
        request.host,
        request.instance
    );
    if ownership.environment != request.policy.name()
        || ownership.marker != expected_marker
        || ownership.token_name != expected_token_name
    {
        return Err(UiProvisioningError::Invalid(
            "Cloudflare metadata does not match this VM identity; refusing cleanup".into(),
        ));
    }
    cloudflare::validate_zone_id(&ownership.zone_id)?;
    if request.configured_zone_id.trim() != ownership.zone_id {
        return Err(UiProvisioningError::Invalid(
            "test Cloudflare zone differs from the recorded VM owner; restore the original vault values before cleanup".into(),
        ));
    }
    let zone = provider.zone(request.creator_token, &ownership.zone_id)?;
    let configured_domain =
        skillet_caddy::resolve_ui_domain(&zone.name, request.relative_ui_domain)?;
    if configured_domain != ownership.ui_domain {
        return Err(UiProvisioningError::Invalid(
            "test UI namespace differs from the recorded VM owner; restore the original relative prefix before cleanup".into(),
        ));
    }
    skillet_caddy::validate_domain_in_zone(&ownership.ui_domain, &zone.name)?;
    let account_id = cloudflare::Cloudflare::account_id(&zone)?.to_string();
    let cleanup_name = format!("{}:cleanup", ownership.token_name);
    let cleanup_token = provider.create_zone_token(
        request.creator_token,
        &ownership.zone_id,
        &account_id,
        &cleanup_name,
        Some(request.policy.cleanup_token_lifetime()),
    )?;
    let cleanup = (|| {
        provider.zone(&cleanup_token.value, &ownership.zone_id)?;
        provider.remove_dns_marker(
            &cleanup_token.value,
            &ownership.zone_id,
            &ownership.marker,
            &ownership.ui_domain,
        )?;
        for id in
            provider.token_ids_by_name(request.creator_token, &account_id, &ownership.token_name)?
        {
            provider.revoke_token(request.creator_token, &account_id, &id)?;
        }
        Ok::<(), CloudflareError>(())
    })();
    let revoke_cleanup = provider
        .token_ids_by_name(request.creator_token, &account_id, &cleanup_name)
        .and_then(|ids| {
            for id in ids {
                provider.revoke_token(request.creator_token, &account_id, &id)?;
            }
            Ok(())
        });
    cleanup?;
    revoke_cleanup?;
    provisioning_state::remove_cloudflare_ownership(&request.ownership_path)?;
    Ok(true)
}

/// Persist each ownership transition around disposable token and DNS
/// mutations. The vault guard runs immediately before token issuance.
pub fn provision_disposable_ui(
    request: &DisposableUiRequest<'_>,
    provider: &impl UiCloudflareProvider,
    ensure_vault_unchanged: impl FnOnce() -> Result<(), VaultError>,
) -> Result<DisposableUiCredentials, UiProvisioningError> {
    if request.policy.environment() != Environment::Test {
        return Err(UiProvisioningError::Invalid(
            "disposable UI provisioning requires the test environment policy".into(),
        ));
    }
    cloudflare::validate_zone_id(request.zone_id)?;
    let zone = provider.zone(request.creator_token, request.zone_id)?;
    let plan = build_ui_provisioning_plan(
        request.host,
        request.policy,
        &zone.name,
        request.relative_ui_domain,
        request.addresses,
    )?;
    let account_id = cloudflare::Cloudflare::account_id(&zone)?.to_string();
    let marker = format!(
        "skillet:{}:{}:{}",
        request.policy.name(),
        request.host,
        request.instance
    );
    let token_name = format!(
        "skillet:{}:{}-{}",
        request.policy.name(),
        request.host,
        request.instance
    );
    let metadata_path = request.run_directory.join("cloudflare.json");
    let mut ownership = CloudflareVmOwnership {
        identity: Some(ProvisioningIdentity::new(
            request.host,
            request.policy.name(),
            request.instance,
        )),
        environment: request.policy.name().to_string(),
        marker,
        zone_id: request.zone_id.to_string(),
        ui_domain: plan.sites.ui_domain.clone(),
        token_name: token_name.clone(),
        token_id: None,
        expires_on: None,
        record_ids: Vec::new(),
    };
    provisioning_state::save_cloudflare_ownership(&metadata_path, &ownership)?;
    ensure_vault_unchanged()?;
    let token = provider.create_zone_token(
        request.creator_token,
        request.zone_id,
        &account_id,
        &token_name,
        request.policy.cloudflare_token_lifetime(),
    )?;
    ownership.token_id = Some(token.id.clone());
    ownership.expires_on.clone_from(&token.expires_on);
    provisioning_state::save_cloudflare_ownership(&metadata_path, &ownership)?;
    let owned = provider.reconcile_dns(
        &token.value,
        request.zone_id,
        &ownership.marker,
        &ownership.ui_domain,
        &plan.dns_records,
    )?;
    ownership.record_ids = owned.records.into_iter().map(|record| record.id).collect();
    provisioning_state::save_cloudflare_ownership(&metadata_path, &ownership)?;
    Ok(DisposableUiCredentials {
        sites: plan.sites,
        token,
        account_id,
        token_name,
    })
}

pub fn build_ui_provisioning_plan(
    host: &str,
    policy: ProvisioningPolicy,
    zone_domain: &str,
    relative_ui_domain: Option<&str>,
    addresses: &BTreeSet<String>,
) -> Result<UiProvisioningPlan, UiProvisioningError> {
    let profile = skillet_hosts::profile_for_name(host)
        .ok_or_else(|| UiProvisioningError::NoUiServices(host.to_string()))?;
    let services = profile.ui_services();
    if services.is_empty() {
        return Err(UiProvisioningError::NoUiServices(host.to_string()));
    }
    let ui_domain = skillet_caddy::resolve_ui_domain(zone_domain, relative_ui_domain)?;
    let sites = CaddySites::from_host(
        host,
        &UiEnvironment {
            ui_domain,
            acme_staging: policy.acme_staging(),
        },
        &services,
    )?;
    let dns_records = cloudflare::desired_records(&sites.machine_hostname, addresses, &sites)?;
    Ok(UiProvisioningPlan { sites, dns_records })
}

#[cfg(test)]
#[path = "ui_provisioning/tests.rs"]
mod tests;
