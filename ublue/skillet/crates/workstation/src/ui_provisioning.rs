//! Shared host/environment UI hostname, Caddy, and DNS planning.

use crate::{
    cloudflare::{self, CloudflareError, DesiredRecord, IssuedToken, OwnedDns, Zone},
    provisioning_policy::{Environment, ProvisioningPolicy},
    provisioning_state::{self, CloudflareVmOwnership, ProvisioningIdentity},
    tailscale::{self, OAuthCredentials, TailscaleError},
    vault::{SecretStore, VaultError},
};
use skillet_caddy::{CaddyError, CaddySites, UiEnvironment};
use skillet_vm::transport::GuestTransport;
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
    #[error(transparent)]
    Tailscale(#[from] TailscaleError),
    #[error(transparent)]
    Guest(#[from] skillet_vm::Error),
    #[error("invalid UI provisioning request: {0}")]
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
    fn replace_named_zone_token(
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

    fn replace_named_zone_token(
        &self,
        creator_token: &str,
        zone_id: &str,
        account_id: &str,
        name: &str,
        lifetime: Option<Duration>,
    ) -> Result<IssuedToken, CloudflareError> {
        cloudflare::Cloudflare::replace_named_zone_token(
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

pub trait UiTailscaleProvider {
    fn find_device_by_hostname(
        &self,
        hostname: &str,
        tag: &str,
    ) -> Result<tailscale::DeviceRecord, TailscaleError>;
}

impl UiTailscaleProvider for OAuthCredentials {
    fn find_device_by_hostname(
        &self,
        hostname: &str,
        tag: &str,
    ) -> Result<tailscale::DeviceRecord, TailscaleError> {
        tailscale::find_device_by_hostname(self, hostname, tag)
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

/// Install the complete disposable Caddy input, activate it, verify the
/// non-tailnet rejection response, and then remove superseded named tokens.
pub fn deliver_disposable_ui(
    host: &str,
    credentials: &DisposableUiCredentials,
    creator_token: &str,
    provider: &impl UiCloudflareProvider,
    guest: &impl GuestTransport,
    acceptance_timeout: Duration,
    acceptance_interval: Duration,
) -> Result<(), UiProvisioningError> {
    if credentials.sites.host != host {
        return Err(UiProvisioningError::Invalid(
            "Caddy payload host does not match the requested guest".into(),
        ));
    }
    let sites_payload = serde_json::to_vec(&credentials.sites)
        .map_err(|error| UiProvisioningError::Invalid(format!("encoding Caddy sites: {error}")))?;
    skillet_vm::credential::install_set(
        guest,
        host,
        "skillet-caddy-apply.service",
        skillet_vm::credential::ActivationPolicy::DeferConsumer,
        &[
            ("caddy_sites", &sites_payload),
            ("cloudflare_acme_token", credentials.token.value.as_bytes()),
        ],
    )?;
    run_guest(
        guest,
        host,
        &["-n", "systemctl", "start", "skillet-caddy-apply.service"],
        "activating Caddy after installing its credentials",
    )?;
    verify_non_tailnet_denial(
        guest,
        &credentials.sites,
        acceptance_timeout,
        acceptance_interval,
    )?;
    for old_id in provider.token_ids_by_name(
        creator_token,
        &credentials.account_id,
        &credentials.token_name,
    )? {
        if old_id != credentials.token.id {
            provider.revoke_token(creator_token, &credentials.account_id, &old_id)?;
        }
    }
    Ok(())
}

fn run_guest(
    guest: &impl GuestTransport,
    host: &str,
    arguments: &[&str],
    description: &str,
) -> Result<(), UiProvisioningError> {
    let output = guest.execute(
        &skillet_vm::transport::GuestCommand {
            program: "/usr/bin/sudo",
            arguments,
        },
        None,
    )?;
    if !output.status.success() {
        return Err(UiProvisioningError::Invalid(format!(
            "{description} failed for {host} with status {}",
            output.status
        )));
    }
    Ok(())
}

fn verify_non_tailnet_denial(
    guest: &impl GuestTransport,
    sites: &CaddySites,
    timeout: Duration,
    interval: Duration,
) -> Result<(), UiProvisioningError> {
    let mut verified_names = 0;
    for site in &sites.services {
        for hostname in std::iter::once(&site.hostname).chain(&site.aliases) {
            let started = std::time::Instant::now();
            let resolve = format!("{hostname}:443:127.0.0.1");
            let url = format!("https://{hostname}/");
            loop {
                let output = guest.execute(
                    &skillet_vm::transport::GuestCommand {
                        program: "/usr/bin/curl",
                        arguments: &[
                            "--insecure",
                            "--silent",
                            "--show-error",
                            "--max-time",
                            "8",
                            "--resolve",
                            &resolve,
                            "--write-out",
                            "\\n%{http_code}",
                            &url,
                        ],
                    },
                    None,
                )?;
                if output.status.success()
                    && output.stdout == b"Access denied by Skillet tailnet policy\n403"
                {
                    verified_names += 1;
                    break;
                }
                if started.elapsed() >= timeout {
                    return Err(UiProvisioningError::Invalid(format!(
                        "Caddy did not return its explicit tailnet-denial response for UI name {hostname}"
                    )));
                }
                std::thread::sleep(interval);
            }
        }
    }
    if verified_names == 0 {
        return Err(UiProvisioningError::Invalid(
            "Caddy payload declares no hostnames to verify".into(),
        ));
    }
    Ok(())
}

pub struct PersistentUiDelivery<'a> {
    pub host: &'a str,
    pub policy: ProvisioningPolicy,
    pub zone_id: &'a str,
    pub relative_ui_domain: Option<&'a str>,
    pub creator_token: &'a str,
}

/// Resolve production or named-host UI configuration, reconcile its DNS, and
/// deliver both Caddy credentials before activating the consumer.
pub fn deliver_persistent_ui(
    request: &PersistentUiDelivery<'_>,
    token_store: &mut impl SecretStore,
    cloudflare: &impl UiCloudflareProvider,
    tailnet: &impl UiTailscaleProvider,
    guest: &impl GuestTransport,
) -> Result<CaddySites, UiProvisioningError> {
    let profile = skillet_hosts::profile_for_name(request.host)
        .ok_or_else(|| UiProvisioningError::NoUiServices(request.host.to_string()))?;
    if profile.ui_services().is_empty() {
        return Err(UiProvisioningError::NoUiServices(request.host.to_string()));
    }
    cloudflare::validate_zone_id(request.zone_id)?;
    let zone = cloudflare.zone(request.creator_token, request.zone_id)?;
    let tag = request
        .policy
        .tailscale_tag(crate::provisioning_policy::DeviceClass::ProductionHost);
    let device = tailnet.find_device_by_hostname(request.host, tag)?;
    let plan = build_ui_provisioning_plan(
        request.host,
        request.policy,
        &zone.name,
        request.relative_ui_domain,
        &device.addresses,
    )?;
    let account_id = crate::cloudflare::Cloudflare::account_id(&zone)?.to_string();
    let token_path = format!(
        "skillet/environments/{}/hosts/{}/cloudflare/acme-token",
        request.policy.name(),
        request.host
    );
    let token_name = format!("skillet:{}:{}", request.policy.name(), request.host);
    let token = if let Some(token) = token_store.get(&token_path)? {
        token
    } else {
        let legacy_path = format!("skillet/hosts/{}/cloudflare/acme-token", request.host);
        let legacy = if request.policy.environment() == Environment::Production {
            token_store.get(&legacy_path)?
        } else {
            None
        };
        if let Some(token) = legacy {
            token_store.ensure_unchanged()?;
            token_store.save_verified(&token_path, &token)?;
            token
        } else {
            token_store.ensure_unchanged()?;
            let issued = cloudflare.replace_named_zone_token(
                request.creator_token,
                request.zone_id,
                &account_id,
                &token_name,
                request.policy.cloudflare_token_lifetime(),
            )?;
            if let Err(save_error) = token_store.save_verified(&token_path, &issued.value) {
                return match cloudflare.revoke_token(
                    request.creator_token,
                    &account_id,
                    &issued.id,
                ) {
                    Ok(()) => Err(UiProvisioningError::Vault(save_error)),
                    Err(revoke_error) => Err(UiProvisioningError::Invalid(format!(
                        "saving issued token failed ({save_error}); revoking token {} also failed ({revoke_error})",
                        issued.id
                    ))),
                };
            }
            issued.value
        }
    };
    cloudflare.zone(&token, request.zone_id)?;
    token_store.ensure_unchanged()?;
    let marker = format!("skillet:{}:{}", request.policy.name(), request.host);
    cloudflare.reconcile_dns(
        &token,
        request.zone_id,
        &marker,
        &plan.sites.ui_domain,
        &plan.dns_records,
    )?;
    token_store.ensure_unchanged()?;
    let sites_payload = serde_json::to_vec(&plan.sites)
        .map_err(|error| UiProvisioningError::Invalid(format!("encoding Caddy sites: {error}")))?;
    skillet_vm::credential::install_set(
        guest,
        request.host,
        "skillet-caddy-apply.service",
        skillet_vm::credential::ActivationPolicy::DeferConsumer,
        &[
            ("caddy_sites", &sites_payload),
            ("cloudflare_acme_token", token.as_bytes()),
        ],
    )?;
    let output = guest.execute(
        &skillet_vm::transport::GuestCommand {
            program: "/usr/bin/sudo",
            arguments: &["-n", "systemctl", "start", "skillet-caddy-apply.service"],
        },
        None,
    )?;
    if !output.status.success() {
        return Err(UiProvisioningError::Invalid(format!(
            "Caddy apply failed with status {}",
            output.status
        )));
    }
    Ok(plan.sites)
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
