use super::*;
use crate::vault::VaultError;
use std::{
    cell::{Cell, RefCell},
    os::unix::process::ExitStatusExt,
    process::{ExitStatus, Output},
};

#[derive(Default)]
struct Store {
    values: BTreeMap<String, String>,
    saves: usize,
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
    fn save_verified(&mut self, path: &str, value: &str) -> Result<(), VaultError> {
        self.saves += 1;
        self.values.insert(path.into(), value.into());
        Ok(())
    }
}
#[derive(Default)]
struct Guest {
    present: Cell<bool>,
    app_state: bool,
    payloads: RefCell<Vec<Vec<u8>>>,
    sab_payloads: RefCell<Vec<Vec<u8>>>,
}
impl GuestTransport for Guest {
    fn execute(
        &self,
        command: &GuestCommand<'_>,
        input: Option<&[u8]>,
    ) -> skillet_vm::Result<Output> {
        assert!(command
            .arguments
            .iter()
            .all(|arg| !arg.contains("test-provider-token")));
        if let Some(input) = input {
            if command
                .arguments
                .contains(&skillet_media::SABNZBD_CREDENTIAL)
            {
                self.sab_payloads.borrow_mut().push(input.to_vec());
            } else {
                self.payloads.borrow_mut().push(input.to_vec());
            }
        }
        let code = i32::from(command.arguments.contains(&"test") && !self.app_state);
        Ok(Output {
            status: ExitStatus::from_raw(code << 8),
            stdout: if command.arguments.contains(&"state") {
                if self.present.get() {
                    b"present\n".to_vec()
                } else {
                    b"absent\n".to_vec()
                }
            } else {
                Vec::new()
            },
            stderr: Vec::new(),
        })
    }
    fn upload(&self, _: &std::path::Path, _: &str) -> skillet_vm::Result<()> {
        panic!("no file upload for credentials")
    }
}

#[test]
fn test_delivery_never_falls_back_to_production_and_reuses_instance_keys() {
    let mut store = Store::default();
    store.values.insert(
        "skillet/environments/prod/hosts/donbot/media/putio-token".into(),
        "production-do-not-use".into(),
    );
    let guest = Guest::default();
    let policy = ProvisioningPolicy::new(Environment::Test);
    assert!(deliver(
        "donbot",
        Some("arr"),
        policy,
        &mut store,
        &guest,
        ActivationPolicy::DeferConsumer
    )
    .is_err());
    assert_eq!(store.saves, 0);
    assert!(guest.payloads.borrow().is_empty());
    store.values.insert(
        "skillet/environments/test/hosts/donbot/media/putio-token".into(),
        "test-provider-token".into(),
    );
    deliver(
        "donbot",
        Some("arr"),
        policy,
        &mut store,
        &guest,
        ActivationPolicy::DeferConsumer,
    )
    .unwrap();
    assert_eq!(store.saves, 6);
    guest.present.set(true);
    deliver(
        "donbot",
        Some("arr"),
        policy,
        &mut store,
        &guest,
        ActivationPolicy::DeferConsumer,
    )
    .unwrap();
    assert_eq!(store.saves, 6);
    let payloads = guest.payloads.borrow();
    assert_eq!(payloads[0], payloads[1]);
    assert!(!String::from_utf8_lossy(&payloads[0]).contains("production-do-not-use"));
    assert!(store
        .values
        .keys()
        .filter(|path| path.contains("-api-key"))
        .all(|path| path.contains("/test/hosts/donbot/instances/arr/")));
    assert!(store
        .reads
        .borrow()
        .iter()
        .all(|path| !path.contains("/prod/")));
}

#[test]
fn retained_arr_state_refuses_to_invent_lost_api_keys() {
    let mut store = Store::default();
    store.values.insert(
        "skillet/environments/test/hosts/donbot/media/putio-token".into(),
        "test-provider-token".into(),
    );
    let guest = Guest {
        app_state: true,
        ..Guest::default()
    };
    assert!(deliver(
        "donbot",
        Some("arr"),
        ProvisioningPolicy::new(Environment::Test),
        &mut store,
        &guest,
        ActivationPolicy::DeferConsumer
    )
    .is_err());
    assert_eq!(store.saves, 0);
    assert!(guest.payloads.borrow().is_empty());
}

#[test]
fn sabnzbd_only_never_reads_putio_and_persists_its_key_before_installation() {
    let mut store = Store::default();
    let guest = Guest::default();
    let mut config = skillet_hosts::profile_for_name("donbot")
        .unwrap()
        .media_config()
        .unwrap();
    config.download_backends = &[skillet_media::DownloadBackend::Sabnzbd];
    deliver_selected(
        "donbot",
        Some("usenet"),
        ProvisioningPolicy::new(Environment::Test),
        &mut store,
        &guest,
        ActivationPolicy::DeferConsumer,
        config,
    )
    .unwrap();
    assert!(!store
        .reads
        .borrow()
        .iter()
        .any(|path| path.contains("putio")));
    assert_eq!(store.saves, 5);
    assert_eq!(guest.sab_payloads.borrow().len(), 1);
    let payload = skillet_media::Credentials::parse(
        &String::from_utf8(guest.payloads.borrow()[0].clone()).unwrap(),
    )
    .unwrap();
    assert_eq!(payload.putio_token, "");
    assert!(payload.sabnzbd_api_key.is_some());
    assert!(store.values.contains_key(
        "skillet/environments/test/hosts/donbot/instances/usenet/media/sabnzbd-api-key"
    ));
}

#[test]
fn new_backend_uses_its_own_credential_guard_beside_retained_arr_state() {
    struct LegacyGuest(Guest);
    impl GuestTransport for LegacyGuest {
        fn execute(
            &self,
            command: &GuestCommand<'_>,
            input: Option<&[u8]>,
        ) -> skillet_vm::Result<Output> {
            if command.arguments.contains(&"state")
                && command
                    .arguments
                    .contains(&skillet_media::SABNZBD_CREDENTIAL)
            {
                return Ok(Output {
                    status: ExitStatus::from_raw(0),
                    stdout: b"absent\n".to_vec(),
                    stderr: Vec::new(),
                });
            }
            self.0.execute(command, input)
        }
        fn upload(&self, _: &std::path::Path, _: &str) -> skillet_vm::Result<()> {
            panic!("no upload")
        }
    }
    let mut store = Store::default();
    let guest = Guest::default();
    let policy = ProvisioningPolicy::new(Environment::Test);
    store.values.insert(
        "skillet/environments/test/hosts/donbot/media/putio-token".into(),
        "test-provider-token".into(),
    );
    deliver(
        "donbot",
        Some("upgrade"),
        policy,
        &mut store,
        &guest,
        ActivationPolicy::DeferConsumer,
    )
    .unwrap();
    store
        .values
        .remove("skillet/environments/test/hosts/donbot/instances/upgrade/media/sabnzbd-api-key");
    store.saves = 0;
    guest.present.set(true);
    assert!(deliver(
        "donbot",
        Some("upgrade"),
        policy,
        &mut store,
        &guest,
        ActivationPolicy::DeferConsumer
    )
    .is_err());
    assert_eq!(store.saves, 0);
    let guest = LegacyGuest(guest);
    deliver(
        "donbot",
        Some("upgrade"),
        policy,
        &mut store,
        &guest,
        ActivationPolicy::DeferConsumer,
    )
    .unwrap();
    assert_eq!(store.saves, 1);
}
