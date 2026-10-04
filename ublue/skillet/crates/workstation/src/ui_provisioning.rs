//! Shared host/environment UI hostname, Caddy, and DNS planning.

use crate::{
    cloudflare::{self, CloudflareError, DesiredRecord},
    provisioning_policy::ProvisioningPolicy,
};
use skillet_caddy::{CaddyError, CaddySites, UiEnvironment};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UiProvisioningError {
    #[error("host profile {0} declares no private UI services")]
    NoUiServices(String),
    #[error(transparent)]
    Caddy(#[from] CaddyError),
    #[error(transparent)]
    Cloudflare(#[from] CloudflareError),
}

/// Desired public DNS and Caddy configuration for one declared host profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiProvisioningPlan {
    pub sites: CaddySites,
    pub dns_records: Vec<DesiredRecord>,
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
