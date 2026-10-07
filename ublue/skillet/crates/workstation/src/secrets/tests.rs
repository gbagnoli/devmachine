use super::*;
use crate::provisioning_policy::Environment;

#[test]
fn documentation_matches_the_shared_requirement_metadata() {
    assert_eq!(
        documentation().unwrap(),
        include_str!("../../../../../../SECRETS.md")
    );
    let modules = catalog().unwrap();
    for profile in skillet_hosts::profile::declared_profiles() {
        for service in profile.services {
            assert!(
                modules
                    .iter()
                    .any(|module| module.required_by.iter().any(|name| name == service.name())),
                "undocumented service {}",
                service.name()
            );
        }
    }
}

#[test]
fn manual_requirements_follow_profiles_environment_and_template_references() {
    let profile = skillet_hosts::profile_for_name("clamps").unwrap();
    let report = check(
        &[profile],
        ProvisioningPolicy::new(Environment::Test),
        &|_| Ok(None),
    )
    .unwrap();
    assert_eq!(report.checked, 7);
    assert_eq!(report.missing.len(), 7);
    assert!(report.missing.iter().any(
        |entry| entry.path == "skillet/environments/test/hosts/clamps/cloudflare/ddns-dns-name"
    ));
    assert!(report
        .missing
        .iter()
        .all(|entry| !entry.path.contains("/prod/")
            && !entry.path.ends_with("web-password")
            && !entry.path.ends_with("acme-token")));
    let profile = skillet_hosts::profile_for_name("beezelbot").unwrap();
    let report = check(
        &[profile],
        ProvisioningPolicy::new(Environment::Production),
        &|_| Ok(Some("not-displayed".into())),
    )
    .unwrap();
    assert_eq!(report.checked, 2);
    assert!(report.missing.is_empty());
}

#[test]
fn invalid_and_empty_entries_are_reported_without_values_or_lookup_error_text() {
    let profiles = skillet_hosts::profile::declared_profiles();
    let policy = ProvisioningPolicy::new(Environment::Production);
    for lookup in [false, true] {
        let report = check(&profiles, policy, &|_| {
            if lookup {
                Err(VaultError::Invalid("private-value-do-not-print".into()))
            } else {
                Ok(Some(String::new()))
            }
        })
        .unwrap();
        assert!(report
            .missing
            .iter()
            .all(|entry| entry.invalid && !entry.guide.contains("private-value-do-not-print")));
        assert_eq!(report.checked, report.missing.len());
    }
}

#[test]
fn unused_audit_keeps_both_environments_hosts_generated_and_legacy_paths() {
    let stored = [
        "skillet/environments/prod/datadog/api-key",
        "skillet/environments/test/datadog/api-key",
        "skillet/environments/test/hosts/clamps/cloudflare/ddns-dns-name",
        "skillet/environments/prod/hosts/beezelbot/cloudflare/acme-token",
        "skillet/environments/test/hosts/beezelbot/cloudflare/acme-token",
        "skillet/hosts/clamps/cloudflare/acme-token",
        "skillet/hosts/clamps/pihole/web-password",
        "personal/not-for-skillet",
        "skillet/environments/dns/cloudflare-zone-id",
        "skillet/environments/prod/hosts/unknown/cloudflare/acme-token",
        "skillet/environments/test/hosts/beezelbot/cloudflare/ddns-dns-name",
    ]
    .map(str::to_string);
    let unused = unused_paths(&skillet_hosts::profile::declared_profiles(), &stored).unwrap();
    assert_eq!(unused.len(), 3);
    assert!(unused.contains(&"skillet/environments/dns/cloudflare-zone-id".into()));
    assert!(
        unused.contains(&"skillet/environments/prod/hosts/unknown/cloudflare/acme-token".into())
    );
    assert!(unused
        .contains(&"skillet/environments/test/hosts/beezelbot/cloudflare/ddns-dns-name".into()));
}

#[test]
fn template_reference_discovery_is_vault_free() {
    let paths = configuration_templates::secret_paths("datadog", "clamps", "test").unwrap();
    assert_eq!(
        paths,
        [
            "skillet/environments/test/datadog/api-key",
            "skillet/environments/test/datadog/site"
        ]
    );
}
