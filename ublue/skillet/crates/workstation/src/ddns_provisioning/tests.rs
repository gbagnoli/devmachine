use super::*;
use crate::cloudflare::ZoneAccount;
use skillet_vm::transport::GuestCommand;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    os::unix::process::ExitStatusExt,
    path::Path,
    process::{ExitStatus, Output},
};

const ZONE: &str = "0123456789abcdef0123456789abcdef";
const CONFIG: &str = r#"{"version":1,"records":[{"name":"edge","proxied":false}]}"#;
const MARKER: &str = "skillet-ddns:production:clamps";

#[derive(Default)]
struct Store {
    entries: BTreeMap<String, String>,
    fail_save: bool,
    changed: bool,
}
impl SecretStore for Store {
    fn get(&self, path: &str) -> Result<Option<String>, VaultError> {
        Ok(self.entries.get(path).cloned())
    }
    fn ensure_unchanged(&self) -> Result<(), VaultError> {
        if self.changed {
            Err(VaultError::Invalid("vault changed".into()))
        } else {
            Ok(())
        }
    }
    fn save_verified(&mut self, path: &str, value: &str) -> Result<(), VaultError> {
        if self.fail_save {
            return Err(VaultError::Invalid("save failed".into()));
        }
        self.entries.insert(path.into(), value.into());
        Ok(())
    }
}

#[derive(Default)]
struct Provider {
    issued: Cell<usize>,
    revoked: Cell<usize>,
    records: Vec<RecordRef>,
    fail_revoke: bool,
    owned_calls: Cell<usize>,
    removed_records: Cell<usize>,
}
impl DdnsProvider for Provider {
    fn zone(&self, _token: &str, zone: &str) -> Result<Zone, CloudflareError> {
        Ok(Zone {
            id: zone.into(),
            name: "example.com".into(),
            account: Some(ZoneAccount { id: ZONE.into() }),
        })
    }
    fn records(&self, token: &str, zone: &str) -> Result<Vec<RecordRef>, CloudflareError> {
        assert_eq!(zone, ZONE);
        assert!(matches!(token, "dummy-child-token" | "dummy-rotated-token"));
        Ok(self.records.clone())
    }
    fn issue(
        &self,
        creator: &str,
        zone: &str,
        account: &str,
        name: &str,
        lifetime: Option<std::time::Duration>,
    ) -> Result<IssuedToken, CloudflareError> {
        assert_eq!(creator, "dummy-master-token");
        assert_eq!(zone, ZONE);
        assert_eq!(account, ZONE);
        assert_eq!(name, "skillet:production:clamps:ddns");
        assert_eq!(lifetime, None);
        self.issued.set(self.issued.get() + 1);
        Ok(IssuedToken {
            id: "dummy-id".into(),
            value: "dummy-child-token".into(),
            expires_on: None,
        })
    }
    fn revoke(&self, _creator: &str, _account: &str, id: &str) -> Result<(), CloudflareError> {
        assert!(id == "dummy-id" || id == "abcdefabcdefabcdefabcdefabcdefab");
        self.revoked.set(self.revoked.get() + 1);
        if self.fail_revoke {
            Err(CloudflareError::Invalid("revoke failed".into()))
        } else {
            Ok(())
        }
    }

    fn replace(
        &self,
        _creator: &str,
        _zone: &str,
        _account: &str,
        name: &str,
        lifetime: Option<std::time::Duration>,
    ) -> Result<IssuedToken, CloudflareError> {
        assert!(name.starts_with("skillet:test:clamps-") && name.contains(":ddns"));
        assert!(lifetime.is_some());
        self.issued.set(self.issued.get() + 1);
        Ok(IssuedToken {
            id: "abcdefabcdefabcdefabcdefabcdefab".into(),
            value: "dummy-child-token".into(),
            expires_on: Some("2030-01-01T00:00:00Z".into()),
        })
    }
    fn owned_test_records(
        &self,
        _token: &str,
        _zone: &str,
        marker: &str,
        names: &[String],
    ) -> Result<Vec<RecordRef>, CloudflareError> {
        let call = self.owned_calls.get();
        self.owned_calls.set(call + 1);
        if call == 0 {
            return Ok(Vec::new());
        }
        Ok(names
            .iter()
            .map(|name| RecordRef {
                id: "1234567890abcdef1234567890abcdef".into(),
                name: name.clone(),
                record_type: "A".into(),
                content: "8.8.8.8".into(),
                comment: Some(marker.into()),
            })
            .collect())
    }
    fn remove_test_records(
        &self,
        _token: &str,
        _zone: &str,
        marker: &str,
        names: &[String],
        ids: &[String],
    ) -> Result<(), CloudflareError> {
        assert!(marker.starts_with("skillet-ddns:test:"));
        assert_ne!(ids.len(), 0);
        assert_eq!(names.len(), ids.len());
        self.removed_records.set(ids.len());
        Ok(())
    }
    fn token_ids(
        &self,
        _creator: &str,
        _account: &str,
        name: &str,
    ) -> Result<Vec<String>, CloudflareError> {
        assert!(name.contains(":ddns"));
        Ok(vec!["abcdefabcdefabcdefabcdefabcdefab".into()])
    }
}

#[derive(Default)]
struct Guest {
    payloads: RefCell<Vec<Vec<u8>>>,
    fail: Cell<bool>,
}
impl GuestTransport for Guest {
    fn execute(
        &self,
        command: &GuestCommand<'_>,
        payload: Option<&[u8]>,
    ) -> skillet_vm::Result<Output> {
        assert_eq!(command.program, "/usr/bin/sudo");
        assert!(command.arguments.contains(&"cloudflare_ddns_config"));
        assert!(command.arguments.contains(&"skillet-ddns-apply.service"));
        assert!(command
            .arguments
            .iter()
            .all(|arg| !arg.contains("dummy-child-token") && !arg.contains("dummy-master-token")));
        let payload = payload.unwrap();
        assert!(!String::from_utf8_lossy(payload).contains("dummy-master-token"));
        skillet_ddns::config::Payload::parse(std::str::from_utf8(payload).unwrap()).unwrap();
        self.payloads.borrow_mut().push(payload.to_vec());
        Ok(Output {
            status: ExitStatus::from_raw(if self.fail.get() { 256 } else { 0 }),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
    fn upload(&self, _source: &Path, _destination: &str) -> skillet_vm::Result<()> {
        panic!("delivery must use stdin")
    }
}
fn request(config: &str) -> DdnsDelivery<'_> {
    DdnsDelivery {
        host: "clamps",
        policy: ProvisioningPolicy::new(Environment::Production),
        zone_id: ZONE,
        relative_ui_domain: None,
        config,
        creator_token: "dummy-master-token",
    }
}

#[test]
fn creates_once_and_reuses_saved_token_after_delivery_failure() {
    let mut store = Store::default();
    let provider = Provider::default();
    let guest = Guest::default();
    guest.fail.set(true);
    assert!(deliver_persistent_ddns(&request(CONFIG), &mut store, &provider, &guest).is_err());
    guest.fail.set(false);
    deliver_persistent_ddns(&request(CONFIG), &mut store, &provider, &guest).unwrap();
    assert_eq!(provider.issued.get(), 1);
    assert_eq!(
        store.entries["skillet/environments/prod/hosts/clamps/cloudflare/ddns-token"],
        "dummy-child-token"
    );
    assert_eq!(guest.payloads.borrow()[0], guest.payloads.borrow()[1]);
    store.entries.insert(
        "skillet/environments/prod/hosts/clamps/cloudflare/ddns-token".into(),
        "dummy-rotated-token".into(),
    );
    deliver_persistent_ddns(&request(CONFIG), &mut store, &provider, &guest).unwrap();
    assert_eq!(provider.issued.get(), 1);
    assert!(String::from_utf8_lossy(&guest.payloads.borrow()[2]).contains("dummy-rotated-token"));
}

#[test]
fn invalid_config_ui_namespace_and_test_policy_never_issue_or_deliver() {
    let mut store = Store::default();
    let provider = Provider::default();
    let guest = Guest::default();
    for config in [
        "invalid",
        r#"{"version":1,"records":[{"name":"edge.ui","proxied":false}]}"#,
    ] {
        assert!(deliver_persistent_ddns(&request(config), &mut store, &provider, &guest).is_err());
    }
    let mut request = request(CONFIG);
    request.policy = ProvisioningPolicy::new(Environment::Test);
    assert!(deliver_persistent_ddns(&request, &mut store, &provider, &guest).is_err());
    request.policy = ProvisioningPolicy::new(Environment::Production);
    request.host = "beezelbot";
    assert!(deliver_persistent_ddns(&request, &mut store, &provider, &guest).is_err());
    assert_eq!(provider.issued.get(), 0);
    assert!(guest.payloads.borrow().is_empty());
}

#[test]
fn save_failure_revokes_and_vault_conflict_prevents_issuance() {
    let provider = Provider::default();
    let guest = Guest::default();
    let mut store = Store {
        fail_save: true,
        ..Store::default()
    };
    assert!(deliver_persistent_ddns(&request(CONFIG), &mut store, &provider, &guest).is_err());
    assert_eq!(provider.revoked.get(), 1);
    assert!(guest.payloads.borrow().is_empty());
    store.changed = true;
    assert!(deliver_persistent_ddns(&request(CONFIG), &mut store, &provider, &guest).is_err());
    assert_eq!(provider.issued.get(), 1);
}

#[test]
fn revocation_failure_returns_safe_recovery_error() {
    let provider = Provider {
        fail_revoke: true,
        ..Provider::default()
    };
    let mut store = Store {
        fail_save: true,
        ..Store::default()
    };
    let error = deliver_persistent_ddns(&request(CONFIG), &mut store, &provider, &Guest::default())
        .unwrap_err();
    assert!(error.to_string().contains("named-token recovery"));
    assert!(!format!("{error:?} {error}").contains("dummy-child-token"));
}

fn existing(kind: &str, comment: Option<&str>) -> RecordRef {
    RecordRef {
        id: "record-id".into(),
        name: "edge.example.com".into(),
        record_type: kind.into(),
        content: "192.0.2.1".into(),
        comment: comment.map(str::to_string),
    }
}
#[test]
fn only_explicit_single_public_a_record_takeover_is_allowed() {
    let mut config = PrivateConfig::parse(CONFIG).unwrap();
    assert!(validate_takeover(&config, "example.com", MARKER, &[existing("A", None)]).is_err());
    assert!(validate_takeover(
        &config,
        "example.com",
        MARKER,
        &[existing("A", Some(MARKER))]
    )
    .is_ok());
    config.takeover_existing = true;
    assert!(validate_takeover(&config, "example.com", MARKER, &[existing("A", None)]).is_ok());
    for records in [
        vec![existing("CNAME", None)],
        vec![existing("A", Some("skillet:production:clamps"))],
        vec![existing("A", Some("skillet-ddns:production:other"))],
        vec![existing("A", None), existing("A", None)],
    ] {
        assert!(validate_takeover(&config, "example.com", MARKER, &records).is_err());
    }
}

#[test]
fn disposable_provision_and_cleanup_are_journaled_and_identity_bound() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Provider::default();
    let guest = Guest::default();
    let policy = ProvisioningPolicy::new(Environment::Test);
    let request = DisposableDdnsRequest {
        host: "clamps",
        instance: "ddns-smoke",
        policy,
        zone_id: ZONE,
        relative_ui_domain: Some("ui"),
        config: CONFIG,
        creator_token: "dummy-master-token",
        run_directory: directory.path(),
    };
    provision_disposable_ddns(
        &request,
        &provider,
        &guest,
        std::time::Duration::ZERO,
        std::time::Duration::ZERO,
    )
    .unwrap();
    let path = directory.path().join("ddns.json");
    let owner = crate::provisioning_state::load_ddns_ownership(&path).unwrap();
    assert_eq!(owner.identity.host, "clamps");
    assert_eq!(owner.identity.environment, "test");
    assert_eq!(owner.record_names, ["edge.example.com"]);
    assert_eq!(owner.record_ids, ["1234567890abcdef1234567890abcdef"]);
    let cleanup = DisposableDdnsCleanup {
        host: "clamps",
        instance: "ddns-smoke",
        policy,
        ownership_path: &path,
        configured_zone_id: ZONE,
        creator_token: "dummy-master-token",
    };
    assert!(cleanup_disposable_ddns(&cleanup, &provider).unwrap());
    assert_eq!(provider.removed_records.get(), 1);
    assert!(!crate::provisioning_state::ddns_ownership_exists(&path).unwrap());
    assert!(!cleanup_disposable_ddns(&cleanup, &provider).unwrap());
}

#[test]
fn disposable_cleanup_refuses_identity_and_zone_mismatches_before_api_changes() {
    let directory = tempfile::tempdir().unwrap();
    let provider = Provider::default();
    let policy = ProvisioningPolicy::new(Environment::Test);
    let request = DisposableDdnsRequest {
        host: "clamps",
        instance: "ddns-smoke",
        policy,
        zone_id: ZONE,
        relative_ui_domain: None,
        config: CONFIG,
        creator_token: "dummy-master-token",
        run_directory: directory.path(),
    };
    provision_disposable_ddns(
        &request,
        &provider,
        &Guest::default(),
        std::time::Duration::ZERO,
        std::time::Duration::ZERO,
    )
    .unwrap();
    let path = directory.path().join("ddns.json");
    let wrong = DisposableDdnsCleanup {
        host: "clamps",
        instance: "another",
        policy,
        ownership_path: &path,
        configured_zone_id: ZONE,
        creator_token: "dummy-master-token",
    };
    assert!(cleanup_disposable_ddns(&wrong, &provider).is_err());
    assert_eq!(provider.removed_records.get(), 0);
    assert!(!public_ipv4("100.64.0.2"));
    assert!(!public_ipv4("192.0.2.1"));
    assert!(public_ipv4("8.8.8.8"));
}

#[test]
fn rejects_zones_that_exceed_the_updaters_record_page() {
    let config = PrivateConfig::parse(CONFIG).unwrap();
    let records = (0..100)
        .map(|index| RecordRef {
            name: format!("other-{index}.example.com"),
            ..existing("A", None)
        })
        .collect::<Vec<_>>();
    assert!(validate_takeover(&config, "example.com", MARKER, &records).is_err());
    assert!(validate_takeover(&config, "example.com", MARKER, &records[..99]).is_ok());
}
