use super::*;
use std::{
    cell::Cell,
    collections::VecDeque,
    process::{Command, Output},
    sync::Mutex,
};

#[derive(Default)]
struct FakeProvider {
    keys_created: Cell<usize>,
    devices_found: Cell<usize>,
}

#[derive(Default)]
struct FakeCleanupProvider {
    removed: Cell<usize>,
    fail: Cell<bool>,
    expected_id: Mutex<Option<String>>,
}

impl DeviceCleanupProvider for FakeCleanupProvider {
    fn remove_device(
        &self,
        hostname: &str,
        expected_tag: &str,
        expected: Option<&DeviceRecord>,
    ) -> tailscale::Result<Option<DeviceRecord>> {
        self.removed.set(self.removed.get() + 1);
        assert_eq!(expected_tag, tailscale::SMOKE_TAG);
        *self.expected_id.lock().unwrap() = expected.map(|device| device.id.clone());
        if self.fail.get() {
            return Err(tailscale::TailscaleError::Invalid(
                "fixture removal failure".into(),
            ));
        }
        Ok(expected.cloned().map(|mut device| {
            device.hostname = hostname.to_string();
            device
        }))
    }
}

impl EnrollmentProvider for FakeProvider {
    fn create_auth_key(&self, _tag: &str, _description: &str) -> tailscale::Result<AuthKey> {
        self.keys_created.set(self.keys_created.get() + 1);
        Ok(AuthKey {
            key: "dummy-auth-key".to_string(),
        })
    }

    fn find_device(
        &self,
        expected_hostname: &str,
        _expected_tag: &str,
        _addresses: &BTreeSet<String>,
    ) -> tailscale::Result<DeviceRecord> {
        self.devices_found.set(self.devices_found.get() + 1);
        Ok(DeviceRecord {
            id: "device-id".to_string(),
            hostname: expected_hostname.to_string(),
            addresses: BTreeSet::from(["100.64.0.10".to_string()]),
        })
    }
}

struct FakeGuest {
    statuses: Mutex<VecDeque<Output>>,
    installs: Mutex<Vec<CapturedInstall>>,
    container_exists: bool,
}

type CapturedInstall = (Vec<String>, Option<Vec<u8>>);

impl FakeGuest {
    fn new(statuses: impl IntoIterator<Item = Output>) -> Self {
        Self {
            statuses: Mutex::new(statuses.into_iter().collect()),
            installs: Mutex::new(Vec::new()),
            container_exists: true,
        }
    }

    fn without_tailscale_container(statuses: impl IntoIterator<Item = Output>) -> Self {
        Self {
            statuses: Mutex::new(statuses.into_iter().collect()),
            installs: Mutex::new(Vec::new()),
            container_exists: false,
        }
    }
}

impl GuestTransport for FakeGuest {
    fn execute(
        &self,
        command: &skillet_vm::transport::GuestCommand<'_>,
        input: Option<&[u8]>,
    ) -> skillet_vm::Result<Output> {
        if command
            .arguments
            .ends_with(&["container", "exists", "tailscale"])
        {
            let created_after_credential_delivery = !self.installs.lock().unwrap().is_empty();
            return Ok(output(
                self.container_exists || created_after_credential_delivery,
                Vec::new(),
            ));
        }
        if command.arguments.contains(&"credential") {
            self.installs.lock().unwrap().push((
                command
                    .arguments
                    .iter()
                    .map(|argument| (*argument).to_string())
                    .collect(),
                input.map(<[u8]>::to_vec),
            ));
            return Ok(output(true, Vec::new()));
        }
        Ok(self
            .statuses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| status("Running", &["100.64.0.10"])))
    }

    fn upload(&self, _source: &std::path::Path, _destination: &str) -> skillet_vm::Result<()> {
        Ok(())
    }
}

fn output(success: bool, stdout: Vec<u8>) -> Output {
    Output {
        status: Command::new(if success { "true" } else { "false" })
            .status()
            .expect("create local exit status"),
        stdout,
        stderr: Vec::new(),
    }
}

fn status(state: &str, addresses: &[&str]) -> Output {
    output(
        true,
        serde_json::json!({
            "BackendState": state,
            "Self": {"TailscaleIPs": addresses}
        })
        .to_string()
        .into_bytes(),
    )
}

fn request(run_directory: &std::path::Path) -> DisposableEnrollment<'_> {
    DisposableEnrollment {
        host: "clamps",
        instance: "smoke",
        vm_hostname: "clamps-test-smoke",
        run_directory,
        policy: ProvisioningPolicy::new(Environment::Test),
    }
}

fn cleanup_request(run_directory: &std::path::Path) -> DisposableCleanup<'_> {
    DisposableCleanup {
        host: "clamps",
        instance: "smoke",
        vm_hostname: "clamps-test-smoke",
        run_directory,
        policy: ProvisioningPolicy::new(Environment::Test),
    }
}

fn recorded_device() -> DeviceRecord {
    DeviceRecord {
        id: "device-id".to_string(),
        hostname: "clamps-test-smoke".to_string(),
        addresses: BTreeSet::from(["100.64.0.10".to_string()]),
    }
}

#[test]
fn already_enrolled_vm_is_recorded_without_creating_an_auth_key() {
    let run = tempfile::tempdir().unwrap();
    let guest = FakeGuest::new([status("Running", &["100.64.0.10"])]);
    let provider = FakeProvider::default();
    let identity = ProvisioningIdentity::new("clamps", "test", "smoke");

    let device = enroll_disposable_vm(&request(run.path()), &provider, &guest).unwrap();

    assert_eq!(device.hostname, "clamps-test-smoke");
    assert_eq!(provider.keys_created.get(), 0);
    assert_eq!(provider.devices_found.get(), 1);
    assert!(guest.installs.lock().unwrap().is_empty());
    assert!(!run.path().join("tailscale-pending").exists());
    assert_eq!(
        provisioning_state::load_tailscale_record(
            &run.path().join("tailscale.json"),
            &identity,
            "clamps-test-smoke"
        )
        .unwrap(),
        device
    );
}

#[test]
fn unenrolled_vm_receives_auth_key_before_device_lookup_and_state_is_saved() {
    let run = tempfile::tempdir().unwrap();
    let guest = FakeGuest::new([
        status("NeedsLogin", &[]),
        status("Running", &["100.64.0.10"]),
    ]);
    let provider = FakeProvider::default();

    enroll_disposable_vm(&request(run.path()), &provider, &guest).unwrap();

    assert_eq!(provider.keys_created.get(), 1);
    assert_eq!(provider.devices_found.get(), 1);
    let installs = guest.installs.lock().unwrap();
    assert_eq!(installs.len(), 1);
    assert_eq!(installs[0].1.as_deref(), Some(b"dummy-auth-key".as_slice()));
    assert!(installs[0]
        .0
        .iter()
        .any(|argument| argument == "skillet-full-apply.service"));
    assert!(!run.path().join("tailscale-pending").exists());
    assert!(run.path().join("tailscale.json").is_file());
}

#[test]
fn fresh_vm_without_tailscale_container_is_enrolled_after_credential_delivery() {
    let run = tempfile::tempdir().unwrap();
    let guest = FakeGuest::without_tailscale_container([status("Running", &["100.64.0.10"])]);
    let provider = FakeProvider::default();

    enroll_disposable_vm(&request(run.path()), &provider, &guest).unwrap();

    assert_eq!(provider.keys_created.get(), 1);
    assert_eq!(provider.devices_found.get(), 1);
    assert_eq!(guest.installs.lock().unwrap().len(), 1);
}

#[test]
fn failed_guest_status_keeps_recovery_marker_and_does_not_issue_key() {
    let run = tempfile::tempdir().unwrap();
    let guest = FakeGuest::new([output(false, Vec::new())]);
    let provider = FakeProvider::default();

    let error = enroll_disposable_vm(&request(run.path()), &provider, &guest).unwrap_err();

    assert!(error
        .to_string()
        .contains("refusing to treat probe failure"));
    assert_eq!(provider.keys_created.get(), 0);
    assert!(run.path().join("tailscale-pending").is_file());
    assert!(!run.path().join("tailscale.json").exists());
}

#[test]
fn production_policy_is_rejected_before_guest_or_provider_effects() {
    let run = tempfile::tempdir().unwrap();
    let mut request = request(run.path());
    request.policy = ProvisioningPolicy::new(Environment::Production);
    let guest = FakeGuest::new([]);
    let provider = FakeProvider::default();

    assert!(enroll_disposable_vm(&request, &provider, &guest).is_err());
    assert_eq!(provider.keys_created.get(), 0);
    assert!(guest.installs.lock().unwrap().is_empty());
    assert!(!run.path().join("tailscale-pending").exists());
}

#[test]
fn disposable_cleanup_removes_verified_device_then_local_recovery_state() {
    let run = tempfile::tempdir().unwrap();
    let identity = ProvisioningIdentity::new("clamps", "test", "smoke");
    provisioning_state::save_tailscale_record(run.path(), &identity, &recorded_device()).unwrap();
    provisioning_state::mark_tailscale_pending(run.path(), &identity, "clamps-test-smoke").unwrap();
    let provider = FakeCleanupProvider::default();

    assert!(cleanup_disposable_vm(&cleanup_request(run.path()), &provider).unwrap());

    assert_eq!(provider.removed.get(), 1);
    assert_eq!(
        provider.expected_id.lock().unwrap().as_deref(),
        Some("device-id")
    );
    assert!(!run.path().join("tailscale.json").exists());
    assert!(!run.path().join("tailscale-pending").exists());
    assert!(!cleanup_disposable_vm(&cleanup_request(run.path()), &provider).unwrap());
}

#[test]
fn provider_failure_preserves_tailscale_cleanup_journal_for_retry() {
    let run = tempfile::tempdir().unwrap();
    let identity = ProvisioningIdentity::new("clamps", "test", "smoke");
    provisioning_state::save_tailscale_record(run.path(), &identity, &recorded_device()).unwrap();
    let provider = FakeCleanupProvider::default();
    provider.fail.set(true);

    assert!(cleanup_disposable_vm(&cleanup_request(run.path()), &provider).is_err());
    assert!(run.path().join("tailscale.json").is_file());

    provider.fail.set(false);
    assert!(cleanup_disposable_vm(&cleanup_request(run.path()), &provider).unwrap());
    assert!(!run.path().join("tailscale.json").exists());
}

#[test]
fn tailscale_cleanup_identity_mismatch_stops_before_provider_mutation() {
    let run = tempfile::tempdir().unwrap();
    let wrong = ProvisioningIdentity::new("other-host", "test", "smoke");
    provisioning_state::save_tailscale_record(run.path(), &wrong, &recorded_device()).unwrap();
    let provider = FakeCleanupProvider::default();

    assert!(cleanup_disposable_vm(&cleanup_request(run.path()), &provider).is_err());
    assert_eq!(provider.removed.get(), 0);
    assert!(run.path().join("tailscale.json").is_file());
}
