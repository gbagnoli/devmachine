use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};

#[test]
fn retained_units_reset_obsolete_prerequisites_and_use_current_consumers() {
    for profile in crate::declared_profiles()
        .into_iter()
        .filter(HostProfile::requires_full_apply_credentials)
    {
        let system = MockSystem::new();
        let files = MockFiles::new();
        install(&system, &files, &profile).unwrap();
        let content = render(&profile);
        assert!(content.contains("[Unit]\nConditionPathExists=\n"));
        assert!(content.contains("[Service]\nLoadCredentialEncrypted=\n"));
        assert!(content.contains("LoadCredentialEncrypted=tailscale_auth_key:"));
        assert!(!content.contains("pihole_web_password"));
        install(&system, &files, &profile).unwrap();
    }
}

#[test]
fn interrupted_unit_reload_is_retried_and_native_callers_keep_their_credential() {
    use std::sync::atomic::Ordering;
    let mut profile = crate::profile_for_name("clamps").unwrap();
    for service in &mut profile.services {
        if matches!(service.config, crate::ServiceConfig::Pihole) {
            service.ui = None;
        }
    }
    assert!(render(&profile).contains("LoadCredentialEncrypted=pihole_web_password:"));
    let system = MockSystem::new();
    let files = MockFiles::new();
    system.fail_reload_once.store(true, Ordering::SeqCst);
    assert!(install(&system, &files, &profile).is_err());
    assert!(!files
        .files
        .lock()
        .unwrap()
        .contains_key("/etc/systemd/system/skillet-full-apply.service.d/credentials.applied"));
    install(&system, &files, &profile).unwrap();
    assert!(system
        .services
        .lock()
        .unwrap()
        .contains_key("daemon-reload"));
}

#[test]
fn joplin_apply_unit_loads_only_its_password_and_caddy_site_credentials() {
    let profile = crate::profile_for_name("donbot").expect("Donbot profile");
    let system = MockSystem::new();
    let files = MockFiles::new();
    install(&system, &files, &profile).unwrap();
    let units = files.files.lock().unwrap();
    let content =
        String::from_utf8(units["/etc/systemd/system/skillet-joplin-apply.service"].clone())
            .unwrap();
    assert!(content.contains(
        "LoadCredentialEncrypted=caddy_sites:/etc/credstore.encrypted/skillet/caddy_sites.cred"
    ));
    assert!(content.contains("LoadCredentialEncrypted=joplin_postgres_password:/etc/credstore.encrypted/skillet/joplin_postgres_password.cred"));
    assert!(content.contains("skillet-donbot apply --phase joplin"));
    assert!(!content.contains("--host-file"));
    assert!(units.contains_key("/etc/systemd/system/skillet-joplin-apply.service"));
    assert!(system
        .services
        .lock()
        .unwrap()
        .contains_key("skillet-joplin-apply.service"));
}
