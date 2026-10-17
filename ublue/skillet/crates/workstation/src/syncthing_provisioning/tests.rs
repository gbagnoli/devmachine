use super::*;
use crate::provisioning_policy::Environment;
#[test]
fn authoritative_zone_and_runtime_scope_are_explicit() {
    let prod = ProvisioningPolicy::new(Environment::Production);
    let test = ProvisioningPolicy::new(Environment::Test);
    let input = settings("clamps", "clamps", prod, "example.invalid").unwrap();
    assert_eq!(input.device_name, "clamps.example.invalid");
    let input = settings("joey", "joey-test-seed", test, "example.invalid").unwrap();
    assert_eq!(input.device_name, "joey-test-seed.example.invalid");
    assert!(settings("joey", "joey-test-seed", prod, "example.invalid").is_err());
    assert!(settings("unknown", "unknown", test, "example.invalid").is_err());
    assert!(settings("clamps", "clamps", prod, "bad/zone").is_err());
}
