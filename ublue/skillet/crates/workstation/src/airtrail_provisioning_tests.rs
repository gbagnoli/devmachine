use super::*;

#[test]
fn disposable_identity_uses_instance_scoped_vault_path() {
    assert!(valid_component("joplin-airtrail"));
    assert!(!valid_component("Donbot"));
    assert!(!valid_component("../other"));
}

#[test]
fn generated_database_password_policy_matches_database_url_format() {
    assert!(valid_password(&"A".repeat(32)));
    assert!(!valid_password(&"!".repeat(32)));
    assert!(!valid_password("short"));
}
