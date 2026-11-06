use super::{credential_path, valid_unit, CredentialInstallError};
use std::path::Path;

#[test]
fn credential_names_are_single_literal_components() {
    assert_eq!(
        credential_path("pihole_web_password").unwrap(),
        Path::new("/etc/credstore.encrypted/skillet/pihole_web_password.cred")
    );
    for name in ["", "../secret", "name.service", "name-with-dash", "a/b"] {
        assert!(matches!(
            credential_path(name),
            Err(CredentialInstallError::InvalidName)
        ));
    }
}

#[test]
fn consumer_units_are_service_unit_names() {
    for unit in ["skillet-full-apply.service", "skillet-caddy-apply.service"] {
        assert!(valid_unit(unit));
    }
    for unit in [
        "",
        "service",
        "-service.service",
        "../service.service",
        "service.timer",
    ] {
        assert!(!valid_unit(unit));
    }
}
