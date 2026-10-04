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

fn account_lookup_contract(
    resource: &impl super::AccountLookupResource,
    expected_user: &super::UserIdentity,
    expected_group: &super::GroupIdentity,
) {
    assert_eq!(
        resource.user_by_name(&expected_user.name).unwrap(),
        Some(expected_user.clone())
    );
    assert_eq!(
        resource.user_by_uid(expected_user.uid).unwrap(),
        Some(expected_user.clone())
    );
    assert_eq!(
        resource.group_by_name(&expected_group.name).unwrap(),
        Some(expected_group.clone())
    );
}

fn service_resource_contract(resource: &impl super::ServiceResource) {
    assert!(!resource.service_is_active("contract.service").unwrap());
    resource.service_start("contract.service").unwrap();
    assert!(resource.service_is_active("contract.service").unwrap());
    resource.service_restart("contract.service").unwrap();
    assert!(resource.service_is_active("contract.service").unwrap());
    resource.service_stop("contract.service").unwrap();
    assert!(!resource.service_is_active("contract.service").unwrap());
}

fn podman_secret_resource_contract(resource: &impl super::PodmanSecretResource) {
    assert!(resource.ensure_podman_secret("contract", "first").unwrap());
    let first = resource.podman_secret_id("contract").unwrap();
    assert!(!resource.ensure_podman_secret("contract", "first").unwrap());
    assert_eq!(resource.podman_secret_id("contract").unwrap(), first);
    assert!(resource
        .ensure_podman_secret("contract", "rotated")
        .unwrap());
    assert_ne!(resource.podman_secret_id("contract").unwrap(), first);
}

#[test]
fn mock_and_recorded_system_effects_follow_shared_contracts() {
    use crate::{recorder::Recorder, test_utils::MockSystem};

    let mock = MockSystem::new();
    service_resource_contract(&mock);
    podman_secret_resource_contract(&mock);

    let recorder = Recorder::new(MockSystem::new());
    service_resource_contract(&recorder);
    podman_secret_resource_contract(&recorder);
}

#[test]
fn linux_and_mock_account_lookup_adapters_follow_shared_contract() {
    use super::{GroupIdentity, LinuxSystemResource, UserIdentity};
    use crate::{system::AccountResource, test_utils::MockSystem};

    let host_user = users::get_user_by_uid(users::get_current_uid())
        .expect("the test process must map to an account");
    let host_group = users::get_group_by_gid(host_user.primary_group_id())
        .expect("the test process primary group must exist");
    let user = UserIdentity {
        name: host_user.name().to_string_lossy().into_owned(),
        uid: host_user.uid(),
        primary_gid: host_user.primary_group_id(),
    };
    let group = GroupIdentity {
        name: host_group.name().to_string_lossy().into_owned(),
        gid: host_group.gid(),
    };

    let linux = LinuxSystemResource::new();
    account_lookup_contract(&linux, &user, &group);

    let mock = MockSystem::new();
    mock.ensure_group(&group.name, Some(group.gid)).unwrap();
    mock.ensure_user(&user.name, Some(user.uid), Some(user.primary_gid))
        .unwrap();
    account_lookup_contract(&mock, &user, &group);
}
