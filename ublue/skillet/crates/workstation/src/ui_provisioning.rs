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
