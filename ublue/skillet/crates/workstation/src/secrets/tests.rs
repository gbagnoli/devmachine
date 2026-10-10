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
        &|_, _| Ok(None),
    )
    .unwrap();
    assert_eq!(report.checked, 6);
    assert_eq!(report.missing.len(), 6);
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
        &|_, field| {
            Ok(Some(
                match field {
                    "port" => "587",
                    "tls" => "starttls",
                    "sender" => "server@example.com",
                    _ => "not-displayed",
                }
                .into(),
            ))
        },
    )
    .unwrap();
    assert_eq!(report.checked, 3);
    assert!(report.missing.is_empty());
}

#[test]
fn invalid_and_empty_entries_are_reported_without_values_or_lookup_error_text() {
    let profiles = skillet_hosts::profile::declared_profiles();
    let policy = ProvisioningPolicy::new(Environment::Production);
    for lookup in [false, true] {
        let report = check(&profiles, policy, &|_, _| {
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
        "skillet/datadog/api-key",
        "skillet/environments/test/datadog/api-key",
        "skillet/environments/test/datadog/site",
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
    assert_eq!(unused.len(), 5);
    assert!(unused.contains(&"skillet/environments/test/datadog/api-key".into()));
    assert!(unused.contains(&"skillet/environments/test/datadog/site".into()));
    assert!(unused.contains(&"skillet/environments/dns/cloudflare-zone-id".into()));
    assert!(
        unused.contains(&"skillet/environments/prod/hosts/unknown/cloudflare/acme-token".into())
    );
    assert!(unused
        .contains(&"skillet/environments/test/hosts/beezelbot/cloudflare/ddns-dns-name".into()));
}

#[test]
fn template_reference_discovery_is_vault_free() {
    for environment in ["prod", "test"] {
        let paths =
            configuration_templates::secret_paths("datadog", "clamps", environment).unwrap();
        assert_eq!(paths, ["skillet/datadog/api-key"]);
    }
}

#[test]
fn smtp_fields_are_required_validated_and_never_rendered() {
    let profiles = [skillet_hosts::profile_for_name("clamps").unwrap()];
    for environment in [Environment::Production] {
        for bad_field in ["UserName", "Password", "host", "port", "tls", "sender"] {
            for missing in [false, true] {
                let report = check(
                    &profiles,
                    ProvisioningPolicy::new(environment),
                    &|path, field| {
                        if path != "skillet/smtp" {
                            return Ok(Some("fixture-only".into()));
                        }
                        if field == bad_field {
                            return Ok(if missing {
                                None
                            } else {
                                Some(
                                    if matches!(field, "UserName" | "Password") {
                                        " "
                                    } else {
                                        "PRIVATE-INVALID-VALUE!"
                                    }
                                    .into(),
                                )
                            });
                        }
                        Ok(Some(
                            match field {
                                "host" => "smtp.example.com",
                                "port" => "587",
                                "tls" => "starttls",
                                "sender" => "server@example.com",
                                _ => "PRIVATE-CREDENTIAL",
                            }
                            .into(),
                        ))
                    },
                )
                .unwrap();
                assert_eq!(report.missing.len(), 1);
                let failure = &report.missing[0];
                assert_eq!(failure.path, "skillet/smtp");
                assert!(failure.invalid);
                assert!(failure.guide.contains(bad_field));
                assert!(!failure.guide.contains("PRIVATE-"));
            }
        }
    }
}

#[test]
fn field_rules_reject_unsafe_hosts_ports_and_tls_modes() {
    for value in [
        "",
        " ",
        "smtp://example.com",
        "user@example.com",
        "-bad.example",
        "bad..example",
        "bad\n.example",
    ] {
        assert!(!valid_hostname(value), "{value:?}");
    }
    for value in ["smtp.example.com", "127.0.0.1", "::1"] {
        assert!(valid_hostname(value));
    }
    let port = FieldRequirement {
        name: "port".into(),
        rule: FieldRule::Port,
        values: vec![],
    };
    for value in ["0", "65536", "-1", "587\n", "587:25"] {
        assert!(!port.valid(value));
    }
    assert!(port.valid("587"));
    let tls = FieldRequirement {
        name: "tls".into(),
        rule: FieldRule::Choice,
        values: vec!["starttls".into()],
    };
    for value in ["", "none", "optional", "STARTTLS"] {
        assert!(!tls.valid(value));
    }
    assert!(tls.valid("starttls"));
}

#[test]
fn shared_smtp_is_counted_once_and_is_not_unused() {
    let profiles = skillet_hosts::declared_profiles();
    let report = check(
        &profiles,
        ProvisioningPolicy::new(Environment::Production),
        &|_, _| Ok(None),
    )
    .unwrap();
    assert_eq!(
        report
            .missing
            .iter()
            .filter(|entry| entry.path == "skillet/smtp")
            .count(),
        1
    );
    assert_eq!(
        unused_paths(&profiles, &["skillet/smtp".into()]).unwrap(),
        Vec::<String>::new()
    );
    let agent = skillet_hosts::profile_for_name("agent").unwrap();
    assert_eq!(
        check(
            &[agent],
            ProvisioningPolicy::new(Environment::Test),
            &|_, _| Ok(None)
        )
        .unwrap()
        .checked,
        0
    );
}

#[test]
fn capture_environment_does_not_require_or_read_smtp_provider_fields() {
    let profiles = skillet_hosts::declared_profiles();
    let report = check(
        &profiles,
        ProvisioningPolicy::new(Environment::Test),
        &|path, _| {
            assert_ne!(path, "skillet/smtp");
            Ok(None)
        },
    )
    .unwrap();
    assert!(report
        .missing
        .iter()
        .all(|entry| entry.path != "skillet/smtp"));
}

#[test]
fn recovery_inventory_recognizes_only_declared_hosts_and_valid_instances() {
    let profiles = skillet_hosts::profile::declared_profiles();
    let paths: Vec<String> = [
        "skillet/hosts/clamps/storage/root-recovery-key",
        "skillet/environments/test/hosts/clamps/instances/smoke/storage/root-recovery-key",
        "skillet/environments/test/hosts/unknown/instances/smoke/storage/root-recovery-key",
        "skillet/environments/test/hosts/clamps/instances/a/b/storage/root-recovery-key",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(
        unused_paths(&profiles, &paths).unwrap(),
        vec![paths[3].clone(), paths[2].clone()]
    );
}
