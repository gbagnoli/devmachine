use super::*;
use std::collections::BTreeMap;

fn fields() -> BTreeMap<(String, String), String> {
    BTreeMap::from([
        (
            (
                "skillet/hosts/clamps/network/switches".into(),
                "switches".into(),
            ),
            "office".into(),
        ),
        (
            ("skillet/network/switches/office".into(), "make".into()),
            "netgear".into(),
        ),
        (
            ("skillet/network/switches/office".into(), "URL".into()),
            "http://192.168.50.10".into(),
        ),
        (
            ("skillet/network/switches/office".into(), "Password".into()),
            "dummy-switch-password".into(),
        ),
    ])
}

#[test]
fn parses_unique_stable_names_and_empty_inventory() {
    assert_eq!(
        parse_names("office, router-uplink\nlab_switch_1").unwrap(),
        vec!["office", "router-uplink", "lab_switch_1"]
    );
    assert_eq!(parse_names("  \n").unwrap(), Vec::<String>::new());
    for invalid in [
        "office,,lab",
        "office,office",
        "../office",
        "Office",
        "-office",
    ] {
        assert!(matches!(
            parse_names(invalid),
            Err(SwitchInventoryError::InvalidSwitchList(_))
        ));
    }
}

#[test]
fn resolves_device_fields_and_ignores_test_scope() {
    let fields = fields();
    let result = resolve("clamps", "prod", |path, field| {
        Ok(fields.get(&(path.into(), field.into())).cloned())
    })
    .unwrap();
    let result = result.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].name, "office");
    assert_eq!(result[0].endpoint, "http://192.168.50.10");

    let test = resolve("clamps", "test", |_, _| {
        panic!("test must not read production vault entries")
    })
    .unwrap();
    assert!(test.is_none());
}

#[test]
fn missing_inventory_disables_but_incomplete_referenced_entry_fails() {
    assert!(resolve("clamps", "prod", |_, _| Ok(None))
        .unwrap()
        .is_none());
    let fields = fields();
    assert!(matches!(
        resolve("clamps", "prod", |path, field| {
            if field == "Password" {
                Ok(None)
            } else {
                Ok(fields.get(&(path.into(), field.into())).cloned())
            }
        }),
        Err(SwitchInventoryError::Invalid)
    ));
}

#[test]
fn explicit_empty_inventory_is_distinct_from_an_absent_field() {
    let selected = resolve("clamps", "prod", |path, field| {
        Ok(
            (path.ends_with("hosts/clamps/network/switches") && field == "switches")
                .then(String::new),
        )
    })
    .unwrap();
    assert!(selected.is_some_and(|switches| switches.is_empty()));
}

#[test]
fn production_payload_contains_configured_secret_but_test_render_removes_it() {
    let fields = fields();
    let selected = resolve("clamps", "prod", |path, field| {
        Ok(fields.get(&(path.into(), field.into())).cloned())
    })
    .unwrap();
    let base = serde_json::json!({
        "version": 1,
        "api_key": "0123456789abcdef0123456789abcdef",
        "site": "datadoghq.com",
        "tags": []
    })
    .to_string();
    let production = add_to_datadog_payload(&base, selected, false).unwrap();
    assert!(production.contains("dummy-switch-password"));
    let test = skillet_datadog::Input::parse(&production)
        .unwrap()
        .render_for_environment("test", "lab")
        .unwrap();
    assert!(!test.contains("dummy-switch-password"));
    assert!(!test.contains("192.168.50.10"));
}

#[test]
fn validates_make_endpoint_and_password_without_echoing_values() {
    for (make, endpoint, password) in [
        ("other", "http://192.168.50.10", "secret-value"),
        ("netgear", "http://user:pass@example.com", "secret-value"),
        ("netgear", "http://8.8.8.8", "secret-value"),
        ("netgear", "http://example.com/admin", "secret-value"),
        ("netgear", "https://example.com", "secret-value"),
        ("netgear", "http://example.com", ""),
    ] {
        let value = SwitchCredential {
            name: "office".into(),
            endpoint: endpoint.into(),
            make: make.into(),
            password: password.into(),
            metrics_port: 9500,
        };
        let error = value.validate().unwrap_err();
        assert!(!error.to_string().contains("secret-value"));
    }
}

#[test]
fn rejects_duplicate_owners() {
    let first = vec![SwitchCredential {
        name: "office".into(),
        endpoint: "http://192.168.50.10".into(),
        make: "netgear".into(),
        password: "first-secret".into(),
        metrics_port: 9500,
    }];
    let second = vec![SwitchCredential {
        name: "office".into(),
        endpoint: "http://192.168.50.11".into(),
        make: "netgear".into(),
        password: "second-secret".into(),
        metrics_port: 9501,
    }];
    assert!(matches!(
        validate_single_owner([("clamps", first.as_slice()), ("joey", second.as_slice())]),
        Err(SwitchInventoryError::DuplicateOwner)
    ));
}

#[test]
fn inventory_ownership_is_checked_without_device_credentials() {
    let inventories = vec![
        ("clamps".into(), vec!["office".into()]),
        ("joey".into(), vec!["office".into()]),
    ];
    assert!(matches!(
        validate_inventory_owners(&inventories),
        Err(SwitchInventoryError::DuplicateOwner)
    ));
}

#[test]
fn audits_only_present_inventory_and_validates_referenced_device_fields() {
    let all = skillet_hosts::declared_profiles();
    let clamps = vec![skillet_hosts::profile_for_name("clamps").unwrap()];
    let stored = vec!["skillet/hosts/clamps/network/switches".to_string()];
    let fields = fields();
    let report = audit(
        &clamps,
        &all,
        ProvisioningPolicy::new(crate::provisioning_policy::Environment::Production),
        &stored,
        &|path, field| Ok(fields.get(&(path.into(), field.into())).cloned()),
    )
    .unwrap();
    assert_eq!(report.checked, 2);
    assert_eq!(report.missing.len(), 0);
    assert!(report.present.contains(&stored[0]));
    assert!(report
        .present
        .contains(&"skillet/network/switches/office".to_string()));

    let missing = audit(
        &clamps,
        &all,
        ProvisioningPolicy::new(crate::provisioning_policy::Environment::Production),
        &stored,
        &|path, field| {
            if path.ends_with("network/switches") && field == "switches" {
                Ok(Some("office".into()))
            } else {
                Ok(None)
            }
        },
    )
    .unwrap();
    assert_eq!(missing.checked, 2);
    assert_eq!(missing.missing[0].path, "skillet/network/switches/office");
    assert_eq!(missing.missing[0].fields, ["make", "URL", "Password"]);
    assert!(!missing.missing[0].invalid);
}

#[test]
fn explicit_empty_inventory_is_valid_and_duplicate_owner_is_reported() {
    let all = skillet_hosts::declared_profiles();
    let clamps = vec![skillet_hosts::profile_for_name("clamps").unwrap()];
    let stored = vec![
        "skillet/hosts/clamps/network/switches".to_string(),
        "skillet/hosts/joey/network/switches".to_string(),
    ];
    let empty = audit(
        &clamps,
        &all,
        ProvisioningPolicy::new(crate::provisioning_policy::Environment::Production),
        &stored[..1],
        &|_, _| Ok(Some(String::new())),
    )
    .unwrap();
    assert_eq!(empty.checked, 1);
    assert!(empty.missing.is_empty());

    let duplicate = audit(
        &clamps,
        &all,
        ProvisioningPolicy::new(crate::provisioning_policy::Environment::Production),
        &stored,
        &|path, field| {
            if path.ends_with("network/switches") && field == "switches" {
                Ok(Some("office".into()))
            } else {
                Ok(None)
            }
        },
    )
    .unwrap();
    assert!(duplicate
        .missing
        .iter()
        .any(|problem| problem.path == stored[0] && problem.invalid));
}

#[test]
fn test_inventory_audit_never_reads_production_paths() {
    let all = skillet_hosts::declared_profiles();
    let clamps = vec![skillet_hosts::profile_for_name("clamps").unwrap()];
    let report = audit(
        &clamps,
        &all,
        ProvisioningPolicy::new(crate::provisioning_policy::Environment::Test),
        &["skillet/hosts/clamps/network/switches".into()],
        &|_, _| panic!("test policy must not query production switch entries"),
    )
    .unwrap();
    assert_eq!(report.checked, 0);
    assert!(report.missing.is_empty());
}

#[test]
fn requirements_list_configured_switch_fields_and_leave_absent_inventory_optional() {
    let all = skillet_hosts::declared_profiles();
    let clamps = vec![skillet_hosts::profile_for_name("clamps").unwrap()];
    let policy = ProvisioningPolicy::new(crate::provisioning_policy::Environment::Production);
    let empty = requirement_problems(&clamps, &all, policy, &[], &|_, _| {
        panic!("absent inventory must not trigger a vault lookup")
    })
    .unwrap();
    assert!(empty.is_empty());

    let stored = vec!["skillet/hosts/clamps/network/switches".into()];
    let configured = requirement_problems(&clamps, &all, policy, &stored, &|path, field| {
        if path == stored[0] && field == "switches" {
            Ok(Some("office".into()))
        } else {
            Ok(None)
        }
    })
    .unwrap();
    assert_eq!(configured.len(), 1);
    assert_eq!(configured[0].path, "skillet/network/switches/office");
    assert_eq!(configured[0].fields, ["make", "URL", "Password"]);

    let empty_list = requirement_problems(&clamps, &all, policy, &stored, &|_, _| {
        Ok(Some(String::new()))
    })
    .unwrap();
    assert!(empty_list.is_empty());
}

#[test]
fn invalid_inventory_audit_explains_name_format_without_echoing_values() {
    let all = skillet_hosts::declared_profiles();
    let clamps = vec![skillet_hosts::profile_for_name("clamps").unwrap()];
    let stored = vec!["skillet/hosts/clamps/network/switches".to_string()];
    let report = requirement_problems(
        &clamps,
        &all,
        ProvisioningPolicy::new(crate::provisioning_policy::Environment::Production),
        &stored,
        &|_, field| Ok((field == "switches").then(|| "GS308EP-private-name".into())),
    )
    .unwrap();
    assert_eq!(report.len(), 1);
    assert!(report[0]
        .detail
        .is_some_and(|detail| detail.contains("lowercase ASCII")));
    assert!(!report[0].detail.unwrap().contains("GS308EP-private-name"));
}
