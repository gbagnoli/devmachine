//! Recoverable Tailscale enrollment for disposable guest machines.

use crate::{
    provisioning_policy::{DeviceClass, Environment, ProvisioningPolicy},
    provisioning_state::{self, ProvisioningIdentity},
    tailscale::{self, AuthKey, DeviceRecord, OAuthCredentials, TailscaleError},
};
use skillet_vm::{credential, transport::GuestTransport};
use std::{collections::BTreeSet, path::Path, time::Duration};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EnrollmentError {
    #[error("invalid disposable Tailscale enrollment: {0}")]
    Invalid(String),
    #[error(transparent)]
    Provider(#[from] TailscaleError),
    #[error(transparent)]
    State(#[from] provisioning_state::ProvisioningStateError),
    #[error(transparent)]
    Guest(#[from] skillet_vm::Error),
}

/// Narrow provider capability needed by the enrollment workflow.
pub trait EnrollmentProvider {
    fn create_auth_key(&self, tag: &str, description: &str) -> tailscale::Result<AuthKey>;
    fn find_device(
        &self,
        expected_hostname: &str,
        expected_tag: &str,
        addresses: &BTreeSet<String>,
    ) -> tailscale::Result<DeviceRecord>;
}

/// Device removal is kept separate from enrollment so callers and tests only
/// provide the provider capability used by a particular lifecycle action.
pub trait DeviceCleanupProvider {
    fn remove_device(
        &self,
        hostname: &str,
        expected_tag: &str,
        expected: Option<&DeviceRecord>,
    ) -> tailscale::Result<Option<DeviceRecord>>;
}

impl EnrollmentProvider for OAuthCredentials {
    fn create_auth_key(&self, tag: &str, description: &str) -> tailscale::Result<AuthKey> {
        tailscale::create_auth_key(self, tag, description)
    }

    fn find_device(
        &self,
        expected_hostname: &str,
        expected_tag: &str,
        addresses: &BTreeSet<String>,
    ) -> tailscale::Result<DeviceRecord> {
        tailscale::find_device(self, expected_hostname, expected_tag, addresses)
    }
}

impl DeviceCleanupProvider for OAuthCredentials {
    fn remove_device(
        &self,
        hostname: &str,
        expected_tag: &str,
        expected: Option<&DeviceRecord>,
    ) -> tailscale::Result<Option<DeviceRecord>> {
        tailscale::remove_device_for_hostname(self, hostname, expected_tag, expected)
    }
}

/// Separate host, environment, instance, runtime hostname and transport inputs
/// so the caller's VM identity is never reconstructed from a composite name.
pub struct DisposableEnrollment<'a> {
    pub host: &'a str,
    pub instance: &'a str,
    pub vm_hostname: &'a str,
    pub run_directory: &'a Path,
    pub policy: ProvisioningPolicy,
}

pub fn enroll_disposable_vm(
    request: &DisposableEnrollment<'_>,
    provider: &impl EnrollmentProvider,
    guest: &impl GuestTransport,
) -> Result<DeviceRecord, EnrollmentError> {
    if request.policy.environment() != Environment::Test {
        return Err(EnrollmentError::Invalid(
            "disposable enrollment requires the test environment policy".into(),
        ));
    }
    let profile = skillet_hosts::profile_for_name(request.host).ok_or_else(|| {
        EnrollmentError::Invalid(format!("unknown host profile {}", request.host))
    })?;
    if !profile.supports_service("tailscale") {
        return Err(EnrollmentError::Invalid(format!(
            "host {} does not declare Tailscale",
            request.host
        )));
    }
    if request.vm_hostname.is_empty() || request.instance.is_empty() {
        return Err(EnrollmentError::Invalid(
            "VM hostname and instance must not be empty".into(),
        ));
    }

    let identity = ProvisioningIdentity::new(request.host, request.policy.name(), request.instance);
    provisioning_state::mark_tailscale_pending(
        request.run_directory,
        &identity,
        request.vm_hostname,
    )?;

    let mut addresses = guest_addresses(guest)?;
    let tag = request.policy.tailscale_tag(DeviceClass::DisposableVm);
    if addresses.is_empty() {
        let auth_key = provider.create_auth_key(
            tag,
            &format!("Skillet disposable VM {}", request.vm_hostname),
        )?;
        credential::install(
            guest,
            request.host,
            "tailscale_auth_key",
            "skillet-full-apply.service",
            credential::ActivationPolicy::StartConsumer,
            auth_key.key.as_bytes(),
        )?;
        addresses = tailscale::wait_for_addresses(
            || guest_addresses(guest),
            Duration::from_mins(2),
            Duration::from_secs(2),
        )?;
    }

    let record = provider.find_device(request.vm_hostname, tag, &addresses)?;
    provisioning_state::save_tailscale_record(request.run_directory, &identity, &record)?;
    provisioning_state::remove_tailscale_pending(request.run_directory)?;
    Ok(record)
}

pub struct DisposableCleanup<'a> {
    pub host: &'a str,
    pub instance: &'a str,
    pub vm_hostname: &'a str,
    pub run_directory: &'a Path,
    pub policy: ProvisioningPolicy,
}

/// Remove the provider device verified by its ownership journal, then remove
/// local recovery records. If local cleanup fails after provider removal, a
/// retry safely observes the absent provider device and finishes journal cleanup.
pub fn cleanup_disposable_vm(
    request: &DisposableCleanup<'_>,
    provider: &impl DeviceCleanupProvider,
) -> Result<bool, EnrollmentError> {
    if request.policy.environment() != Environment::Test {
        return Err(EnrollmentError::Invalid(
            "disposable Tailscale cleanup requires the test environment policy".into(),
        ));
    }
    let profile = skillet_hosts::profile_for_name(request.host).ok_or_else(|| {
        EnrollmentError::Invalid(format!("unknown host profile {}", request.host))
    })?;
    if !profile.supports_service("tailscale") {
        return Err(EnrollmentError::Invalid(format!(
            "host {} does not declare Tailscale",
            request.host
        )));
    }
    let identity = ProvisioningIdentity::new(request.host, request.policy.name(), request.instance);
    let record_exists = provisioning_state::tailscale_record_exists(request.run_directory)?;
    let pending_exists = provisioning_state::validate_tailscale_pending(
        request.run_directory,
        &identity,
        request.vm_hostname,
    )?;
    if !record_exists && !pending_exists {
        return Ok(false);
    }
    let expected = if record_exists {
        Some(provisioning_state::load_tailscale_record(
            &request.run_directory.join("tailscale.json"),
            &identity,
            request.vm_hostname,
        )?)
    } else {
        None
    };
    provider.remove_device(
        request.vm_hostname,
        request.policy.tailscale_tag(DeviceClass::DisposableVm),
        expected.as_ref(),
    )?;
    provisioning_state::remove_tailscale_record(request.run_directory)?;
    provisioning_state::remove_tailscale_pending(request.run_directory)?;
    Ok(true)
}

fn guest_addresses(guest: &impl GuestTransport) -> Result<BTreeSet<String>, EnrollmentError> {
    let output = guest.execute(
        &skillet_vm::transport::GuestCommand {
            program: "/usr/bin/sudo",
            arguments: &[
                "-n",
                "podman",
                "exec",
                "tailscale",
                "tailscale",
                "status",
                "--json",
            ],
        },
        None,
    )?;
    if !output.status.success() {
        return Err(EnrollmentError::Invalid(format!(
            "Tailscale guest status command failed with status {}; refusing to treat probe failure as an unenrolled VM",
            output.status
        )));
    }
    Ok(tailscale::status_addresses(&output.stdout)?)
}

#[cfg(test)]
#[path = "tailscale_enrollment/tests.rs"]
mod tests;
