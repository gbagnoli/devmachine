//! CLI phase parsing and compatibility forwarding to canonical host profiles.

use skillet_core::credentials::CredentialInputs;
use skillet_core::{files::FileResource, system::SystemResource};
pub use skillet_hosts::{
    boot_policy_for_host, declared_profiles, profile_for_host, profile_for_name,
    ui_config_for_host, ApplyError, CredentialConsumer, HostBootPolicy, HostId, HostProfile,
    HostService, HostUiConfig, ServiceConfig, UiServiceDeclaration, CADDY_SITES_CREDENTIAL,
    CLOUDFLARE_ACME_TOKEN_CREDENTIAL, PIHOLE_WEB_PASSWORD_CREDENTIAL,
    TAILSCALE_AUTH_KEY_CREDENTIAL,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum ApplyPhase {
    Base,
    Full,
    Caddy,
    Ddns,
    Datadog,
}

pub fn apply_host_phase(
    hostname: &str,
    phase: ApplyPhase,
    system: &dyn SystemResource,
    files: &dyn FileResource,
    credentials: &CredentialInputs,
) -> Result<(), ApplyError> {
    let phase = match phase {
        ApplyPhase::Base => skillet_hosts::HostApplyPhase::Base,
        ApplyPhase::Full => skillet_hosts::HostApplyPhase::Full,
        ApplyPhase::Caddy => skillet_hosts::HostApplyPhase::Caddy,
        ApplyPhase::Ddns => skillet_hosts::HostApplyPhase::Ddns,
        ApplyPhase::Datadog => skillet_hosts::HostApplyPhase::Datadog,
    };
    skillet_hosts::apply_host_phase(hostname, phase, system, files, credentials)
}
