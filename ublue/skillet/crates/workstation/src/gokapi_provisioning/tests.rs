use super::*;
use crate::{provisioning_policy::Environment, vault::VaultError};
use skillet_vm::transport::{GuestCommand, GuestTransport};
use std::{cell::RefCell, collections::BTreeMap, path::Path, process::Output};
struct Store {
    values: BTreeMap<String, String>,
    reads: RefCell<Vec<String>>,
}
impl SecretStore for Store {
    fn get(&self, path: &str) -> Result<Option<String>, VaultError> {
        self.reads.borrow_mut().push(path.into());
        Ok(self.values.get(path).cloned())
    }
    fn ensure_unchanged(&self) -> Result<(), VaultError> {
        Ok(())
    }
    fn save_verified(&mut self, _: &str, _: &str) -> Result<(), VaultError> {
        panic!("admin identity must never be generated")
    }
}
struct NoMutation;
impl GuestTransport for NoMutation {
    fn execute(&self, _: &GuestCommand<'_>, _: Option<&[u8]>) -> skillet_vm::Result<Output> {
        panic!("invalid credentials must fail before guest contact")
    }
    fn upload(&self, _: &Path, _: &str) -> skillet_vm::Result<()> {
        panic!("no artifact delivery")
    }
}
#[test]
fn admin_selection_is_scoped_and_must_be_an_allowed_complete_identity() {
    let path = "skillet/environments/test/hosts/donbot/gokapi/admin-user";
    let mut store = Store {
        values: BTreeMap::from([
            (
                "skillet/environments/prod/hosts/donbot/gokapi/admin-user".into(),
                "prod-owner@example.invalid".into(),
            ),
            (
                skillet_caddy::auth::USERS_ENTRY.into(),
                "owner@example.invalid".into(),
            ),
        ]),
        reads: RefCell::new(vec![]),
    };
    let policy = ProvisioningPolicy::new(Environment::Test);
    let error = deliver(
        "donbot",
        policy,
        &store,
        &NoMutation,
        ActivationPolicy::DeferConsumer,
    )
    .unwrap_err();
    assert!(error.to_string().contains(path));
    assert_eq!(*store.reads.borrow(), vec![path]);
    store
        .values
        .insert(path.into(), "unlisted@example.invalid".into());
    assert!(deliver(
        "donbot",
        policy,
        &store,
        &NoMutation,
        ActivationPolicy::DeferConsumer
    )
    .is_err());
    assert!(deliver(
        "clamps",
        policy,
        &store,
        &NoMutation,
        ActivationPolicy::DeferConsumer
    )
    .is_err());
}
