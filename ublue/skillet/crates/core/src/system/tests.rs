#[cfg(feature = "test-utils")]
use super::{AccountLookupResource, AccountResource, ServiceResource};
#[cfg(feature = "test-utils")]
use crate::test_utils::MockSystem;

#[test]
#[cfg(feature = "test-utils")]
fn test_mock_system_resource() {
    let system = MockSystem::new();
    let changed = system.ensure_group("syslog", None).unwrap();
    assert!(changed);
    assert!(system
        .groups
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains("syslog"));

    let changed_again = system.ensure_group("syslog", None).unwrap();
    assert!(!changed_again);
}

#[test]
#[cfg(feature = "test-utils")]
fn test_mock_system_services() {
    let system = MockSystem::new();
    system.service_start("test-service").unwrap();
    assert_eq!(
        system
            .services
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get("test-service")
            .unwrap(),
        "started"
    );

    system.service_restart("test-service").unwrap();
    assert_eq!(
        system
            .services
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get("test-service")
            .unwrap(),
        "restarted"
    );
}

#[test]
#[cfg(feature = "test-utils")]
fn mock_account_boundary_preserves_and_checks_numeric_identity() {
    use super::AccountResource;

    let system = MockSystem::new();
    assert!(system
        .ensure_user("service", Some(1042), Some(2042))
        .unwrap());
    assert_eq!(
        system.user_by_name("service").unwrap(),
        Some(super::UserIdentity {
            name: "service".to_string(),
            uid: 1042,
            primary_gid: 2042,
        })
    );
    assert_eq!(system.user_by_uid(1042).unwrap().unwrap().name, "service");
    assert!(!system
        .ensure_user("service", Some(1042), Some(2042))
        .unwrap());
    assert!(system
        .ensure_user("service", Some(1043), Some(2042))
        .is_err());
    assert!(system.group_by_name("service").unwrap().is_some());
}
