use super::{DeviceClass, Environment, ProvisioningPolicy};
use std::time::Duration;

#[test]
fn production_policy_selects_durable_credentials_and_live_acme() {
    let policy = ProvisioningPolicy::new(Environment::Production);

    assert_eq!(policy.name(), "production");
    assert_eq!(
        policy.ui_domain_entry(),
        "skillet/environments/production/dns/ui-domain"
    );
    assert_eq!(
        policy.cloudflare_zone_entry(),
        "skillet/environments/dns/cloudflare-zone-id"
    );
    assert_eq!(policy.cloudflare_token_lifetime(), None);
    assert!(!policy.acme_staging());
    assert_eq!(
        policy.tailscale_tag(DeviceClass::ProductionHost),
        "tag:skillet-server"
    );
}

#[test]
fn disposable_policy_selects_short_lived_credentials_and_staging_acme() {
    let policy = ProvisioningPolicy::new(Environment::Test);

    assert_eq!(policy.name(), "test");
    assert_eq!(
        policy.ui_domain_entry(),
        "skillet/environments/test/dns/ui-domain"
    );
    assert_eq!(
        policy.cloudflare_token_lifetime(),
        Some(Duration::from_hours(12))
    );
    assert_eq!(policy.cleanup_token_lifetime(), Duration::from_mins(15));
    assert!(policy.acme_staging());
    assert_eq!(
        policy.tailscale_tag(DeviceClass::DisposableVm),
        "tag:skillet-smoke"
    );
}
