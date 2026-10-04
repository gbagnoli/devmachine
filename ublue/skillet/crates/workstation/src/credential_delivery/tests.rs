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
        let status = if command.arguments.contains(&"test") {
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

    deliver_pihole_credential("clamps", &mut store, &guest).unwrap();

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

    deliver_pihole_credential("clamps", &mut store, &guest).unwrap();

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

    assert!(deliver_pihole_credential("clamps", &mut store, &guest).is_err());
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
    )
    .unwrap();

    assert_eq!(provider.calls.get(), 1);
    assert_eq!(&*provider.tag.borrow(), tailscale::SERVER_TAG);
    assert_eq!(
        &*provider.description.borrow(),
        "Skillet clamps production host"
    );
    assert_eq!(
        guest.calls.lock().unwrap()[0].1.as_deref(),
        Some(b"dummy auth key".as_slice())
    );
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
    )
    .is_err());
    assert!(deliver_pihole_credential("agent", &mut FakeStore::default(), &guest).is_err());
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
