use super::*;
use crate::provisioning_policy::{Environment, ProvisioningPolicy};
use std::collections::BTreeSet;

fn addresses() -> BTreeSet<String> {
    BTreeSet::from(["100.64.0.10".to_string(), "fd7a:115c:a1e0::10".to_string()])
}

#[test]
fn production_and_test_share_host_dns_plan_but_select_different_acme_policy() {
    let production = build_ui_provisioning_plan(
        "beezelbot",
        ProvisioningPolicy::new(Environment::Production),
        "example.invalid",
        Some("ui"),
        &addresses(),
    )
    .unwrap();
    let test = build_ui_provisioning_plan(
        "beezelbot",
        ProvisioningPolicy::new(Environment::Test),
        "example.invalid",
        Some("smoke.ui"),
        &addresses(),
    )
    .unwrap();

    assert_eq!(
        production.sites.machine_hostname,
        "beezelbot.ui.example.invalid"
    );
    assert_eq!(
        test.sites.machine_hostname,
        "beezelbot.smoke.ui.example.invalid"
    );
    assert!(!production.sites.acme_staging);
    assert!(test.sites.acme_staging);
    assert_eq!(production.sites.services.len(), test.sites.services.len());
    assert!(production.dns_records.iter().any(|record| record.name
        == production.sites.machine_hostname
        && record.record_type == "A"));
    assert!(production.dns_records.iter().any(|record| {
        record.name == production.sites.machine_hostname && record.record_type == "AAAA"
    }));
}

#[test]
fn each_plan_uses_only_the_calling_profile_services_and_aliases() {
    let plan = build_ui_provisioning_plan(
        "clamps",
        ProvisioningPolicy::new(Environment::Test),
        "example.invalid",
        None,
        &addresses(),
    )
    .unwrap();

    let declared = skillet_hosts::profile_for_name("clamps")
        .unwrap()
        .ui_services();
    let declared_names = declared
        .iter()
        .map(|service| service.name.as_str())
        .collect::<Vec<_>>();
    let planned_names = plan
        .sites
        .services
        .iter()
        .map(|service| service.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(planned_names, declared_names);
    for alias in plan
        .sites
        .services
        .iter()
        .flat_map(|service| &service.aliases)
    {
        assert!(plan.dns_records.iter().any(|record| record.name == *alias));
    }
}

#[test]
fn profiles_without_ui_and_incomplete_address_families_fail_closed() {
    assert!(matches!(
        build_ui_provisioning_plan(
            "agent",
            ProvisioningPolicy::new(Environment::Test),
            "example.invalid",
            None,
            &addresses()
        ),
        Err(UiProvisioningError::NoUiServices(_))
    ));
    let only_v4 = BTreeSet::from(["100.64.0.10".to_string()]);
    assert!(matches!(
        build_ui_provisioning_plan(
            "beezelbot",
            ProvisioningPolicy::new(Environment::Test),
            "example.invalid",
            None,
            &only_v4
        ),
        Err(UiProvisioningError::Cloudflare(_))
    ));
}
