use super::*;

fn fixture_field(field: &str) -> String {
    match field {
        "interface" => "eth0",
        "address" => "192.0.2.10",
        "mask" => "255.255.255.0",
        "gateway" => "192.0.2.1",
        "dns1" => "192.0.2.53",
        "dns2" => "127.0.0.1",
        "UserName" => "test",
        _ => "test-password",
    }
    .into()
}
#[test]
fn policy_and_caller_selection_are_checked_before_vault_access() {
    for (host, policy) in [
        ("clamps", Environment::Test),
        ("joey", Environment::Production),
        ("unknown", Environment::Production),
    ] {
        assert!(input(host, ProvisioningPolicy::new(policy), &|_, _| panic!(
            "unexpected vault access"
        ))
        .is_err());
    }
}
#[test]
fn both_interface_entries_are_typed_validated_and_private() {
    let lookup = |path: &str, field: &str| {
        if path.ends_with("/wifi") {
            match field {
                "address" => return Ok(Some("192.0.2.11".into())),
                "interface" => return Ok(Some("wlan0".into())),
                _ => {}
            }
        }
        Ok(Some(fixture_field(field)))
    };
    let policy = ProvisioningPolicy::new(Environment::Production);
    let value = input("clamps", policy, &lookup).unwrap();
    let selection = skillet_hosts::profile_for_name("clamps")
        .unwrap()
        .host_network_selection()
        .unwrap();
    let payload = value.payload("clamps", selection).unwrap();
    let parsed = Input::parse(&payload, "clamps", selection).unwrap();
    assert_eq!(
        parsed.wired.as_ref().unwrap().dns,
        ["192.0.2.53", "127.0.0.1"]
    );
    for missing in [
        "interface",
        "address",
        "mask",
        "gateway",
        "dns1",
        "dns2",
        "UserName",
        "Password",
    ] {
        let error = input("clamps", policy, &|path, field| {
            if field == missing {
                Ok(None)
            } else {
                lookup(path, field)
            }
        })
        .err()
        .unwrap();
        assert!(!error.to_string().contains("test-password"));
    }
}
