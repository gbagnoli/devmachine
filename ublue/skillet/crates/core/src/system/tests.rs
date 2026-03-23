use super::*;
#[cfg(feature = "test-utils")]
use crate::test_utils::MockSystem;

#[test]
#[cfg(feature = "test-utils")]
fn test_mock_system_resource() {
    let system = MockSystem::new();
    let changed = system.ensure_group("syslog").unwrap();
    assert!(changed);
    assert!(system
        .groups
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains("syslog"));

    let changed_again = system.ensure_group("syslog").unwrap();
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
            .unwrap_or_else(|e| e.into_inner())
            .get("test-service")
            .unwrap(),
        "started"
    );

    system.service_restart("test-service").unwrap();
    assert_eq!(
        system
            .services
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get("test-service")
            .unwrap(),
        "restarted"
    );
}
