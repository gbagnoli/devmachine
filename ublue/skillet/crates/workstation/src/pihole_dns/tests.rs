use super::*;
use std::collections::BTreeMap;
fn test_policy() -> ProvisioningPolicy {
    ProvisioningPolicy::new(Environment::Test)
}
#[test]
fn shared_inventory_expands_titles_aliases_and_both_address_families() {
    let root = prefix(test_policy());
    let entry = format!("{root}/entries/printer");
    let fields: BTreeMap<(String, String), String> = BTreeMap::from([
        (
            (format!("{root}/local-prefix"), "Password".into()),
            "home".into(),
        ),
        (
            (entry.clone(), "names".into()),
            "printer, laser\nprint".into(),
        ),
        ((entry.clone(), "ipv4".into()), "192.0.2.20".into()),
        ((entry.clone(), "ipv6".into()), "fd12:3456:789a::20".into()),
    ]);
    let read = |p: &str, f: &str| Ok(fields.get(&(p.into(), f.into())).cloned());
    let input = input(
        test_policy(),
        "example.invalid",
        std::slice::from_ref(&entry),
        &read,
    )
    .unwrap();
    assert_eq!(input.hosts().unwrap(), ["192.0.2.20 printer.home.example.invalid laser.home.example.invalid print.home.example.invalid", "fd12:3456:789a::20 printer.home.example.invalid laser.home.example.invalid print.home.example.invalid"]);
    let other = vec!["skillet/environments/prod/dns/entries/secret".into()];
    assert!(
        super::input(test_policy(), "example.invalid", &other, &read)
            .unwrap()
            .records
            .is_empty()
    );
}
#[test]
fn generated_hosts_use_existing_addresses_and_reject_manual_conflicts() {
    let policy = ProvisioningPolicy::new(Environment::Production);
    let entry = format!("{}/entries/clamps", prefix(policy));
    let read = |p: &str, f: &str| -> Result<Option<String>, VaultError> {
        Ok(if p.ends_with("local-prefix") {
            Some("home".into())
        } else if f == "address" || f == "ipv4" {
            Some("192.0.2.40".into())
        } else {
            None
        })
    };
    let records = input(policy, "example.invalid", &[], &read).unwrap();
    assert!(records
        .hosts()
        .unwrap()
        .iter()
        .any(|s| s.ends_with(" clamps.home.example.invalid")));
    assert!(records
        .hosts()
        .unwrap()
        .iter()
        .any(|s| s.ends_with(" clamps-wifi.home.example.invalid")));
    assert!(input(policy, "example.invalid", &[entry], &read).is_err());
}
#[test]
fn invalid_addresses_and_relative_names_fail_without_values_in_errors() {
    let policy = test_policy();
    let root = prefix(policy);
    let entry = format!("{root}/entries/printer");
    for (field, value) in [
        ("ipv4", "fd12::20"),
        ("ipv6", "192.0.2.20"),
        ("names", "bad name"),
    ] {
        let read = |p: &str, f: &str| -> Result<Option<String>, VaultError> {
            Ok(if p.ends_with("local-prefix") {
                Some("home".into())
            } else if f == field {
                Some(value.into())
            } else {
                None
            })
        };
        assert!(input(
            policy,
            "example.invalid",
            std::slice::from_ref(&entry),
            &read
        )
        .is_err());
    }
}

#[test]
fn private_records_cannot_shadow_the_tailnet_ui_namespace() {
    let policy = test_policy();
    let entry = format!("{}/entries/printer", prefix(policy));
    let read = |p: &str, f: &str| -> Result<Option<String>, VaultError> {
        Ok(if p.ends_with("local-prefix") || p.ends_with("ui-domain") {
            Some("home".into())
        } else if f == "ipv4" {
            Some("192.0.2.20".into())
        } else {
            None
        })
    };
    assert!(input(policy, "example.invalid", &[entry], &read).is_err());
}

#[test]
fn misspelled_collection_group_is_an_error_instead_of_silent_empty_inventory() {
    let policy = test_policy();
    let path = format!("{}/entries /printer", prefix(policy));
    let read = |p: &str, _: &str| -> Result<Option<String>, VaultError> {
        Ok(if p.ends_with("local-prefix") {
            Some("home".into())
        } else {
            None
        })
    };
    assert!(matches!(
        input(policy, "example.invalid", &[path], &read),
        Err(Error::Entry(_))
    ));
}
