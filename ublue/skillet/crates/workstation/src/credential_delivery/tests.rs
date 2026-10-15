use super::*;
use crate::{
    provisioning_policy::Environment,
    vault::{SecretStore, VaultError},
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    os::unix::process::ExitStatusExt,
    process::{ExitStatus, Output},
    sync::Mutex,
};

#[derive(Default)]
struct FakeStore {
    entries: RefCell<BTreeMap<String, String>>,
    saves: Cell<usize>,
    changed: Cell<bool>,
}

impl SecretStore for FakeStore {
    fn get(&self, path: &str) -> Result<Option<String>, VaultError> {
        Ok(self.entries.borrow().get(path).cloned())
    }

    fn ensure_unchanged(&self) -> Result<(), VaultError> {
        if self.changed.get() {
            Err(VaultError::Invalid("fixture vault changed".into()))
        } else {
            Ok(())
        }
    }

    fn save_verified(&mut self, path: &str, secret: &str) -> Result<(), VaultError> {
        self.saves.set(self.saves.get() + 1);
        self.entries
            .borrow_mut()
            .insert(path.to_string(), secret.to_string());
        Ok(())
    }
}

#[derive(Default)]
struct FakeGuest {
    state: Vec<u8>,
    state_exit_code: i32,
    calls: Mutex<Vec<CapturedGuestCall>>,
}

type CapturedGuestCall = (Vec<String>, Option<Vec<u8>>);

#[test]
fn datadog_delivery_validates_capability_and_sends_only_stdin() {
    let guest = FakeGuest::default();
    let key = "0123456789abcdef0123456789abcdef";
    let payload = serde_json::json!({"version":1,"api_key":key,"site":"datadoghq.eu"}).to_string();
    let policy = ProvisioningPolicy::new(Environment::Test);
    assert!(deliver_datadog_credential(
        "agent",
        policy,
        &payload,
        &guest,
        skillet_vm::credential::ActivationPolicy::StartConsumer
    )
    .is_err());
    assert!(deliver_datadog_credential(
        "clamps",
        policy,
        "invalid",
        &guest,
        skillet_vm::credential::ActivationPolicy::StartConsumer
    )
    .is_err());
    assert!(guest.calls.lock().unwrap().is_empty());
    deliver_datadog_credential(
        "clamps",
        policy,
        &payload,
        &guest,
        skillet_vm::credential::ActivationPolicy::StartConsumer,
    )
    .unwrap();
    let calls = guest.calls.lock().unwrap();
    assert!(calls
        .iter()
        .all(|(args, _)| args.iter().all(|arg| !arg.contains(key))));
    assert!(calls
        .iter()
        .any(|(_, input)| input.as_ref().is_some_and(|bytes| {
            let value = String::from_utf8_lossy(bytes);
            value.contains(key) && value.contains("env:test") && value.contains("region:lab")
        })));
}

impl GuestTransport for FakeGuest {
    fn execute(
        &self,
        command: &skillet_vm::transport::GuestCommand<'_>,
        input: Option<&[u8]>,
    ) -> skillet_vm::Result<Output> {
        self.calls.lock().unwrap().push((
            std::iter::once(command.program.to_string())
                .chain(
                    command
                        .arguments
                        .iter()
                        .map(std::string::ToString::to_string),
                )
                .collect(),
            input.map(<[u8]>::to_vec),
        ));
        let status = if command.arguments.contains(&"test") || command.arguments.contains(&"state")
        {
            ExitStatus::from_raw(self.state_exit_code << 8)
        } else {
            ExitStatus::from_raw(0)
        };
        Ok(Output {
            status,
            stdout: if command.arguments.contains(&"state") {
                self.state.clone()
            } else {
                Vec::new()
            },
            stderr: Vec::new(),
        })
    }

    fn upload(&self, _source: &std::path::Path, _destination: &str) -> skillet_vm::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct FakeTailscale {
    calls: Cell<usize>,
    tag: RefCell<String>,
    description: RefCell<String>,
}

impl ProductionAuthKeyProvider for FakeTailscale {
    fn create_auth_key(&self, tag: &str, description: &str) -> tailscale::Result<AuthKey> {
        self.calls.set(self.calls.get() + 1);
        *self.tag.borrow_mut() = tag.to_string();
        *self.description.borrow_mut() = description.to_string();
        Ok(AuthKey {
            key: "dummy auth key".into(),
        })
    }
}

#[test]
fn missing_pihole_password_checks_remote_state_then_persists_and_installs() {
    let mut store = FakeStore::default();
    let guest = FakeGuest {
        state: b"absent\n".to_vec(),
        ..FakeGuest::default()
    };

    deliver_pihole_credential(
        "clamps",
        &mut store,
        &guest,
        skillet_vm::credential::ActivationPolicy::StartConsumer,
    )
    .unwrap();

    assert_eq!(store.saves.get(), 1);
    let password = store
        .entries
        .borrow()
        .get("skillet/hosts/clamps/pihole/web-password")
        .cloned()
        .unwrap();
    assert_eq!(password.len(), 64);
    assert!(password.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let calls = guest.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].0.iter().any(|argument| argument == "state"));
    assert_eq!(calls[1].1.as_deref(), Some(password.as_bytes()));
}

#[test]
fn existing_pihole_password_is_reused_without_remote_state_probe() {
    let mut store = FakeStore::default();
    store.entries.borrow_mut().insert(
        "skillet/hosts/clamps/pihole/web-password".into(),
        "existing password".into(),
    );
    let guest = FakeGuest::default();

    deliver_pihole_credential(
        "clamps",
        &mut store,
        &guest,
        skillet_vm::credential::ActivationPolicy::StartConsumer,
    )
    .unwrap();

    assert_eq!(store.saves.get(), 0);
    let calls = guest.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1.as_deref(), Some(b"existing password".as_slice()));
}

#[test]
fn existing_remote_pihole_secret_blocks_replacement_and_store_mutation() {
    let mut store = FakeStore::default();
    let guest = FakeGuest {
        state: b"present\n".to_vec(),
        ..FakeGuest::default()
    };

    assert!(deliver_pihole_credential(
        "clamps",
        &mut store,
        &guest,
        skillet_vm::credential::ActivationPolicy::StartConsumer
    )
    .is_err());
    assert_eq!(store.saves.get(), 0);
    assert_eq!(guest.calls.lock().unwrap().len(), 1);
}

#[test]
fn tailscale_production_key_uses_profile_tag_and_shared_credential_installer() {
    let provider = FakeTailscale::default();
    let guest = FakeGuest::default();

    deliver_tailscale_credential(
        "clamps",
        ProvisioningPolicy::new(Environment::Production),
        &provider,
        &guest,
        skillet_vm::credential::ActivationPolicy::StartConsumer,
    )
    .unwrap();

    assert_eq!(provider.calls.get(), 1);
    assert_eq!(&*provider.tag.borrow(), tailscale::SERVER_TAG);
    assert_eq!(
        &*provider.description.borrow(),
        "Skillet clamps production host"
    );
    let calls = guest.calls.lock().unwrap();
    let payload = std::str::from_utf8(calls[0].1.as_deref().unwrap()).unwrap();
    let input = skillet_hosts::tailscale::EnrollmentInput::parse(payload).unwrap();
    assert_eq!(input.auth_key, "dummy auth key");
    assert!(input.advertise_exit_node);
}

#[test]
fn invalid_host_or_nonproduction_tailscale_policy_fails_before_effects() {
    let provider = FakeTailscale::default();
    let guest = FakeGuest::default();
    assert!(deliver_tailscale_credential(
        "clamps",
        ProvisioningPolicy::new(Environment::Test),
        &provider,
        &guest,
        skillet_vm::credential::ActivationPolicy::StartConsumer,
    )
    .is_err());
    assert!(deliver_pihole_credential(
        "agent",
        &mut FakeStore::default(),
        &guest,
        skillet_vm::credential::ActivationPolicy::StartConsumer
    )
    .is_err());
    assert_eq!(provider.calls.get(), 0);
    assert!(guest.calls.lock().unwrap().is_empty());
}

#[test]
fn disposable_pihole_credential_is_created_when_missing_and_rotation_is_requested() {
    let guest = FakeGuest {
        state_exit_code: 1,
        ..FakeGuest::default()
    };

    ensure_disposable_pihole_credential("clamps", false, &guest).unwrap();

    let calls = guest.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].0.iter().any(|argument| argument == "test"));
    assert_eq!(calls[1].1.as_ref().map(Vec::len), Some(64));
}

#[test]
fn disposable_pihole_reapply_reuses_secret_and_rotation_replaces_it() {
    let guest = FakeGuest::default();

    ensure_disposable_pihole_credential("clamps", false, &guest).unwrap();

    let calls = guest.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(calls[1]
        .0
        .windows(3)
        .any(|arguments| arguments == ["systemctl", "start", "skillet-full-apply.service"]));
}

#[test]
fn disposable_pihole_inspection_errors_do_not_generate_a_replacement() {
    let guest = FakeGuest {
        state_exit_code: 2,
        ..FakeGuest::default()
    };

    assert!(ensure_disposable_pihole_credential("clamps", false, &guest).is_err());
    assert_eq!(guest.calls.lock().unwrap().len(), 1);
}

#[test]
fn datadog_production_delivery_uses_caller_region() {
    let guest = FakeGuest::default();
    let payload = serde_json::json!({"version":1,"api_key":"0123456789abcdef0123456789abcdef",
        "site":"datadoghq.com","tags":["region:obsolete"]})
    .to_string();
    deliver_datadog_credential(
        "clamps",
        ProvisioningPolicy::new(Environment::Production),
        &payload,
        &guest,
        skillet_vm::credential::ActivationPolicy::StartConsumer,
    )
    .unwrap();
    let calls = guest.calls.lock().unwrap();
    let input = calls.iter().find_map(|(_, input)| input.as_ref()).unwrap();
    let value: serde_json::Value = serde_json::from_slice(input).unwrap();
    assert_eq!(
        value["tags"],
        serde_json::json!(["env:prod", "region:ftwo"])
    );
}

#[test]
fn smtp_delivery_is_stdin_only_and_capture_cannot_contain_provider_fields() {
    let guest = FakeGuest::default();
    let input = skillet_smtp::Input::Production {
        host: "smtp.example.com".into(),
        port: 587,
        tls: "starttls".into(),
        username: "fixture-key".into(),
        password: "fixture-password".into(),
        sender: "server@example.com".into(),
    };
    assert!(crate::smtp_provisioning::deliver(
        "agent",
        &input,
        &guest,
        skillet_vm::credential::ActivationPolicy::StartConsumer
    )
    .is_err());
    assert_eq!(guest.calls.lock().unwrap().len(), 0);
    crate::smtp_provisioning::deliver(
        "joey",
        &input,
        &guest,
        skillet_vm::credential::ActivationPolicy::StartConsumer,
    )
    .unwrap();
    let calls = guest.calls.lock().unwrap();
    assert!(calls.iter().all(|(args, _)| args
        .iter()
        .all(|arg| !arg.contains("fixture-password") && !arg.contains("fixture-key"))));
    assert!(calls.iter().any(|(_, bytes)| bytes
        .as_ref()
        .is_some_and(|value| String::from_utf8_lossy(value).contains("fixture-password"))));
}

#[test]
fn deferred_delivery_never_starts_related_or_independent_consumers() {
    let guest = FakeGuest::default();
    let mut store = FakeStore::default();
    store.entries.borrow_mut().insert(
        "skillet/hosts/clamps/pihole/web-password".into(),
        "fixture-password".into(),
    );
    let deferred = credential::ActivationPolicy::DeferConsumer;
    let policy = ProvisioningPolicy::new(Environment::Production);
    deliver_pihole_credential("clamps", &mut store, &guest, deferred).unwrap();
    deliver_tailscale_credential(
        "clamps",
        policy,
        &FakeTailscale::default(),
        &guest,
        deferred,
    )
    .unwrap();
    let payload = serde_json::json!({"version":1,"api_key":"0123456789abcdef0123456789abcdef","site":"datadoghq.com"}).to_string();
    deliver_datadog_credential("clamps", policy, &payload, &guest, deferred).unwrap();
    crate::smtp_provisioning::deliver("clamps", &skillet_smtp::Input::Capture {}, &guest, deferred)
        .unwrap();
    let calls = guest.calls.lock().unwrap();
    assert_eq!(calls.len(), 4);
    for (args, input) in calls.iter() {
        assert!(args.iter().any(|arg| arg == "--no-start"));
        assert!(!args.iter().any(|arg| arg == "systemctl"));
        assert!(input.is_some());
    }
}

#[test]
fn enrollment_presence_does_not_confuse_failed_inspection_with_absence() {
    for (code, state, expected) in [
        (0, "present\n", Some(true)),
        (0, "absent\n", Some(false)),
        (1, "", None),
        (2, "", None),
        (0, "unknown\n", None),
    ] {
        let guest = FakeGuest {
            state_exit_code: code,
            state: state.as_bytes().to_vec(),
            ..FakeGuest::default()
        };
        assert_eq!(
            tailscale_credential_present("clamps", &guest).ok(),
            expected
        );
        let calls = guest.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].0.iter().any(|arg| arg == "state"));
        assert!(calls[0].0.iter().any(|arg| arg == "tailscale_auth_key"));
        assert!(calls[0].1.is_none());
    }
}

// Exercise retained native-password delivery through an explicit fixture profile.
fn native_profile(host: &str) -> skillet_hosts::HostProfile {
    let mut profile = skillet_hosts::profile_for_name(host).unwrap();
    for service in &mut profile.services {
        if matches!(service.config, skillet_hosts::ServiceConfig::Pihole) {
            service.ui = None;
        }
    }
    profile
}

fn deliver_pihole_credential(
    host: &str,
    store: &mut impl SecretStore,
    guest: &impl GuestTransport,
    activation: credential::ActivationPolicy,
) -> Result<(), CredentialDeliveryError> {
    if !native_profile(host).supports_service("pihole") {
        return super::deliver_pihole_credential(host, store, guest, activation);
    }
    super::deliver_pihole_for_profile(&native_profile(host), store, guest, activation)
}

fn ensure_disposable_pihole_credential(
    host: &str,
    rotate: bool,
    guest: &impl GuestTransport,
) -> Result<(), CredentialDeliveryError> {
    super::ensure_disposable_pihole_for_profile(&native_profile(host), rotate, guest)
}

#[test]
fn protected_pihole_never_reads_or_replaces_native_credentials() {
    let guest = FakeGuest::default();
    let mut store = FakeStore {
        changed: Cell::new(true),
        ..FakeStore::default()
    };
    for host in ["clamps", "joey"] {
        super::deliver_pihole_credential(
            host,
            &mut store,
            &guest,
            credential::ActivationPolicy::StartConsumer,
        )
        .unwrap();
        super::ensure_disposable_pihole_credential(host, true, &guest).unwrap();
    }
    assert_eq!(store.saves.get(), 0);
    assert!(guest.calls.lock().unwrap().is_empty());
}
