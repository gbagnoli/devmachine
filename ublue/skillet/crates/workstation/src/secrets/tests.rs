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
    assert_eq!(report.checked, 7);
    assert_eq!(report.missing.len(), 7);
    assert!(report
        .missing
        .iter()
        .any(|entry| entry.path == "skillet/environments/test/dns/local-prefix"));
    assert!(report
        .missing
        .iter()
        .all(|entry| !entry.path.contains("/prod/")
            && !entry.path.ends_with("web-password")
            && !entry.path.ends_with("acme-token")));
    let profile = skillet_hosts::profile_for_name("joey").unwrap();
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
    assert_eq!(report.checked, 8);
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
        "skillet/environments/test/dns/local-prefix",
        "skillet/environments/prod/hosts/joey/cloudflare/acme-token",
        "skillet/environments/test/hosts/joey/cloudflare/acme-token",
        "skillet/hosts/clamps/cloudflare/acme-token",
        "skillet/hosts/clamps/pihole/web-password",
        "personal/not-for-skillet",
        "skillet/environments/dns/cloudflare-zone-id",
        "skillet/environments/prod/hosts/unknown/cloudflare/acme-token",
        "skillet/environments/test/hosts/joey/cloudflare/ddns-dns-name",
    ]
    .map(str::to_string);
    let unused = unused_paths(&skillet_hosts::profile::declared_profiles(), &stored).unwrap();
    assert_eq!(unused.len(), 6);
    assert!(unused.contains(&"skillet/hosts/clamps/pihole/web-password".into()));
    assert!(unused.contains(&"skillet/environments/test/datadog/api-key".into()));
    assert!(unused.contains(&"skillet/environments/test/datadog/site".into()));
    assert!(unused.contains(&"skillet/environments/dns/cloudflare-zone-id".into()));
    assert!(
        unused.contains(&"skillet/environments/prod/hosts/unknown/cloudflare/acme-token".into())
    );
    assert!(!unused.contains(&"skillet/environments/test/dns/local-prefix".into()));
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
                        if path.contains("/network/") {
                            return Ok(Some(
                                match field {
                                    "interface" => "eth0",
                                    "address" => "192.0.2.10",
                                    "gateway" | "dns1" => "192.0.2.1",
                                    "dns2" => "127.0.0.1",
                                    "mask" => "24",
                                    "UserName" => "fixture",
                                    _ => "fixture-key",
                                }
                                .into(),
                            ));
                        }
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

#[test]
fn network_requirements_follow_selected_interfaces_and_keep_test_dhcp_private() {
    let mut profile = skillet_hosts::profile_for_name("clamps").unwrap();
    let stored = [
        "skillet/hosts/clamps/network/wired".into(),
        "skillet/hosts/clamps/network/wifi".into(),
    ];
    assert_eq!(
        unused_paths(&[profile.clone()], &stored).unwrap(),
        Vec::<String>::new()
    );
    let test = check(
        &[profile.clone()],
        ProvisioningPolicy::new(Environment::Test),
        &|path, _| {
            assert!(!path.contains("/network/"));
            Ok(None)
        },
    )
    .unwrap();
    assert!(test.missing.iter().all(|entry| entry.module != "Network"));
    for service in &mut profile.services {
        if let skillet_hosts::ServiceConfig::Network(selection) = &mut service.config {
            selection.wifi = false;
        }
    }
    let prod = check(
        &[profile.clone()],
        ProvisioningPolicy::new(Environment::Production),
        &|_, _| Ok(None),
    )
    .unwrap();
    let network = prod
        .missing
        .iter()
        .filter(|entry| entry.module == "Network")
        .collect::<Vec<_>>();
    assert_eq!(network.len(), 1);
    assert_eq!(network[0].path, stored[0]);
    assert_eq!(
        unused_paths(&[profile], &stored).unwrap(),
        [stored[1].clone()]
    );
}

#[test]
fn private_dns_collection_audit_follows_environment_and_validates_custom_fields() {
    let profiles = [skillet_hosts::profile_for_name("clamps").unwrap()];
    let policy = ProvisioningPolicy::new(Environment::Test);
    let path = "skillet/environments/test/dns/entries/printer".to_string();
    let stored = vec![
        path.clone(),
        "skillet/environments/prod/dns/entries/private".into(),
    ];
    let lookup = |p: &str, field: &str| -> Result<Option<String>, VaultError> {
        Ok(if p.ends_with("local-prefix") {
            Some("home".into())
        } else if p == path && field == "ipv4" {
            Some("192.0.2.20".into())
        } else if p == path {
            None
        } else {
            Some(
                match field {
                    "port" => "587",
                    "tls" => "starttls",
                    "sender" => "server@example.invalid",
                    _ => "fixture",
                }
                .into(),
            )
        })
    };
    let report = check_with_entries(&profiles, policy, &stored, &lookup).unwrap();
    assert!(report.missing.is_empty());
    assert!(!unused_paths(&profiles, &stored).unwrap().contains(&path));
    let report = check_with_entries(&profiles, policy, &stored, &|p, f| {
        if p == path && f == "ipv4" {
            Ok(Some("not-an-ip-private-value".into()))
        } else {
            lookup(p, f)
        }
    })
    .unwrap();
    assert_eq!(report.missing.len(), 1);
    assert!(!report.missing[0].guide.contains("not-an-ip-private-value"));
}

#[test]
fn homeassistant_keys_are_optional_but_present_values_are_validated() {
    let profile = skillet_hosts::profile_for_name("clamps").unwrap();
    let policy = ProvisioningPolicy::new(Environment::Production);
    let absent = check(std::slice::from_ref(&profile), policy, &|_, _| Ok(None)).unwrap();
    assert!(absent
        .missing
        .iter()
        .all(|entry| entry.module != "Home Assistant"));
    let invalid = check_with_entries(
        &[profile],
        policy,
        &["skillet/environments/prod/hosts/clamps/homeassistant/api-token".into()],
        &|path, _| {
            Ok(path
                .ends_with("homeassistant/api-token")
                .then(|| "short".into()))
        },
    )
    .unwrap();
    let entries: Vec<_> = invalid
        .missing
        .iter()
        .filter(|entry| entry.module == "Home Assistant")
        .collect();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].invalid);
}

#[test]
fn optional_airtrail_keys_are_skipped_when_absent_and_validated_when_present() {
    let profile = skillet_hosts::profile_for_name("donbot").unwrap();
    let policy = ProvisioningPolicy::new(Environment::Production);
    let path = "skillet/environments/prod/hosts/donbot/airtrail/aerodatabox-api-key";

    let absent = check_with_entries(std::slice::from_ref(&profile), policy, &[], &|_, _| {
        Ok(None)
    })
    .unwrap();
    assert!(!absent.present.iter().any(|entry| entry == path));
    assert!(!absent.missing.iter().any(|entry| entry.path == path));

    let present = check_with_entries(
        std::slice::from_ref(&profile),
        policy,
        &[path.to_string()],
        &|entry, field| {
            Ok((entry == path && field == "Password").then(|| "provider-key-value".into()))
        },
    )
    .unwrap();
    assert!(present.present.iter().any(|entry| entry == path));
    assert!(!present.missing.iter().any(|entry| entry.path == path));

    let invalid = check_with_entries(
        std::slice::from_ref(&profile),
        policy,
        &[path.to_string()],
        &|entry, field| Ok((entry == path && field == "Password").then(String::new)),
    )
    .unwrap();
    assert!(invalid
        .missing
        .iter()
        .any(|entry| entry.path == path && entry.invalid));
}
