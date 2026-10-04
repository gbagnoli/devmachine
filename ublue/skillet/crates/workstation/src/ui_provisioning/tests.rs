use super::*;
use crate::provisioning_policy::{Environment, ProvisioningPolicy};
use crate::{
    cloudflare::{
        CloudflareError, DesiredRecord, IssuedToken, OwnedDns, RecordRef, Zone, ZoneAccount,
    },
    provisioning_state::CloudflareVmOwnership,
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

fn addresses() -> BTreeSet<String> {
    BTreeSet::from(["100.64.0.10".to_string(), "fd7a:115c:a1e0::10".to_string()])
}

struct FakeCloudflare {
    run_directory: PathBuf,
    fail_dns: bool,
    issued: std::cell::Cell<usize>,
    removed: std::cell::Cell<usize>,
    revoked: std::cell::RefCell<Vec<String>>,
}

impl UiCloudflareProvider for FakeCloudflare {
    fn zone(&self, _token: &str, zone_id: &str) -> Result<Zone, CloudflareError> {
        Ok(Zone {
            id: zone_id.to_string(),
            name: "example.invalid".to_string(),
            account: Some(ZoneAccount {
                id: "11111111111111111111111111111111".to_string(),
            }),
        })
    }

    fn create_zone_token(
        &self,
        _creator_token: &str,
        _zone_id: &str,
        _account_id: &str,
        name: &str,
        lifetime: Option<std::time::Duration>,
    ) -> Result<IssuedToken, CloudflareError> {
        let ownership = read_ownership(&self.run_directory);
        assert!(
            name == ownership.token_name || name == format!("{}:cleanup", ownership.token_name)
        );
        if name == ownership.token_name {
            assert!(ownership.token_id.is_none());
            assert_eq!(lifetime, Some(std::time::Duration::from_hours(12)));
        } else {
            assert_eq!(lifetime, Some(std::time::Duration::from_mins(15)));
        }
        self.issued.set(self.issued.get() + 1);
        Ok(IssuedToken {
            id: "22222222222222222222222222222222".to_string(),
            value: "dummy token value".to_string(),
            expires_on: Some("2030-01-01T00:00:00Z".to_string()),
        })
    }

    fn reconcile_dns(
        &self,
        _token: &str,
        _zone_id: &str,
        marker: &str,
        _ui_domain: &str,
        desired: &[DesiredRecord],
    ) -> Result<OwnedDns, CloudflareError> {
        let ownership = read_ownership(&self.run_directory);
        assert!(ownership.token_id.is_some());
        assert_eq!(ownership.marker, marker);
        if self.fail_dns {
            return Err(CloudflareError::Invalid("fixture DNS failure".into()));
        }
        Ok(OwnedDns {
            marker: marker.to_string(),
            records: desired
                .iter()
                .enumerate()
                .map(|(index, desired)| RecordRef {
                    id: format!("{index:032x}"),
                    name: desired.name.clone(),
                    record_type: desired.record_type.clone(),
                    content: desired.content.clone(),
                    comment: Some(marker.to_string()),
                })
                .collect(),
        })
    }

    fn remove_dns_marker(
        &self,
        _token: &str,
        _zone_id: &str,
        _marker: &str,
        _ui_domain: &str,
    ) -> Result<(), CloudflareError> {
        if self.fail_dns {
            return Err(CloudflareError::Invalid("fixture DNS failure".into()));
        }
        self.removed.set(self.removed.get() + 1);
        Ok(())
    }

    fn token_ids_by_name(
        &self,
        _creator_token: &str,
        _account_id: &str,
        name: &str,
    ) -> Result<Vec<String>, CloudflareError> {
        Ok(vec![format!("id-for:{name}")])
    }

    fn revoke_token(
        &self,
        _creator_token: &str,
        _account_id: &str,
        token_id: &str,
    ) -> Result<(), CloudflareError> {
        self.revoked.borrow_mut().push(token_id.to_string());
        Ok(())
    }
}

fn fake_cloudflare(run_directory: &Path, fail_dns: bool) -> FakeCloudflare {
    FakeCloudflare {
        run_directory: run_directory.to_path_buf(),
        fail_dns,
        issued: std::cell::Cell::new(0),
        removed: std::cell::Cell::new(0),
        revoked: std::cell::RefCell::new(Vec::new()),
    }
}

fn read_ownership(run_directory: &Path) -> CloudflareVmOwnership {
    serde_json::from_slice(&std::fs::read(run_directory.join("cloudflare.json")).unwrap()).unwrap()
}

fn disposable_request<'a>(
    run_directory: &'a Path,
    addresses: &'a BTreeSet<String>,
) -> DisposableUiRequest<'a> {
    DisposableUiRequest {
        host: "clamps",
        instance: "smoke",
        policy: ProvisioningPolicy::new(Environment::Test),
        zone_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        relative_ui_domain: Some("smoke.ui"),
        creator_token: "dummy creator token",
        run_directory,
        addresses,
    }
}

fn cleanup_request(run_directory: &Path) -> DisposableUiCleanupRequest<'_> {
    DisposableUiCleanupRequest {
        host: "clamps",
        instance: "smoke",
        policy: ProvisioningPolicy::new(Environment::Test),
        ownership_path: run_directory.join("cloudflare.json"),
        configured_zone_id: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        relative_ui_domain: Some("smoke.ui"),
        creator_token: "dummy creator token",
    }
}

fn create_cleanup_fixture(run_directory: &Path, provider: &FakeCloudflare) {
    let addresses = addresses();
    provision_disposable_ui(
        &disposable_request(run_directory, &addresses),
        provider,
        || Ok(()),
    )
    .unwrap();
}

#[test]
fn production_and_test_share_host_dns_plan_but_select_different_acme_policy() {
    let production = build_ui_provisioning_plan(
        "beezelbot",
        ProvisioningPolicy::new(Environment::Production),
        "example.invalid",
        Some("ui"),
        &addresses(),
    )
    .unwrap();
    let test = build_ui_provisioning_plan(
        "beezelbot",
        ProvisioningPolicy::new(Environment::Test),
        "example.invalid",
        Some("smoke.ui"),
        &addresses(),
    )
    .unwrap();

    assert_eq!(
        production.sites.machine_hostname,
        "beezelbot.ui.example.invalid"
    );
    assert_eq!(
        test.sites.machine_hostname,
        "beezelbot.smoke.ui.example.invalid"
    );
    assert!(!production.sites.acme_staging);
    assert!(test.sites.acme_staging);
    assert_eq!(production.sites.services.len(), test.sites.services.len());
    assert!(production.dns_records.iter().any(|record| record.name
        == production.sites.machine_hostname
        && record.record_type == "A"));
    assert!(production.dns_records.iter().any(|record| {
        record.name == production.sites.machine_hostname && record.record_type == "AAAA"
    }));
}

#[test]
fn each_plan_uses_only_the_calling_profile_services_and_aliases() {
    let plan = build_ui_provisioning_plan(
        "clamps",
        ProvisioningPolicy::new(Environment::Test),
        "example.invalid",
        None,
        &addresses(),
    )
    .unwrap();

    let declared = skillet_hosts::profile_for_name("clamps")
        .unwrap()
        .ui_services();
    let declared_names = declared
        .iter()
        .map(|service| service.name.as_str())
        .collect::<Vec<_>>();
    let planned_names = plan
        .sites
        .services
        .iter()
        .map(|service| service.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(planned_names, declared_names);
    for alias in plan
        .sites
        .services
        .iter()
        .flat_map(|service| &service.aliases)
    {
        assert!(plan.dns_records.iter().any(|record| record.name == *alias));
    }
}

#[test]
fn profiles_without_ui_and_incomplete_address_families_fail_closed() {
    assert!(matches!(
        build_ui_provisioning_plan(
            "agent",
            ProvisioningPolicy::new(Environment::Test),
            "example.invalid",
            None,
            &addresses()
        ),
        Err(UiProvisioningError::NoUiServices(_))
    ));
    let only_v4 = BTreeSet::from(["100.64.0.10".to_string()]);
    assert!(matches!(
        build_ui_provisioning_plan(
            "beezelbot",
            ProvisioningPolicy::new(Environment::Test),
            "example.invalid",
            None,
            &only_v4
        ),
        Err(UiProvisioningError::Cloudflare(_))
    ));
}

#[test]
fn disposable_cloudflare_ownership_precedes_token_and_dns_mutations() {
    let run = tempfile::tempdir().unwrap();
    let addresses = addresses();
    let expected_record_count = build_ui_provisioning_plan(
        "clamps",
        ProvisioningPolicy::new(Environment::Test),
        "example.invalid",
        Some("smoke.ui"),
        &addresses,
    )
    .unwrap()
    .dns_records
    .len();
    let provider = fake_cloudflare(run.path(), false);
    let guard_calls = std::cell::Cell::new(0);

    let provisioned = provision_disposable_ui(
        &disposable_request(run.path(), &addresses),
        &provider,
        || {
            guard_calls.set(guard_calls.get() + 1);
            Ok(())
        },
    )
    .unwrap();

    assert_eq!(guard_calls.get(), 1);
    assert_eq!(provider.issued.get(), 1);
    assert_eq!(provisioned.sites.host, "clamps");
    assert_eq!(provisioned.token.value, "dummy token value");
    assert_eq!(provisioned.token_name, "skillet:test:clamps-smoke");
    let ownership = read_ownership(run.path());
    assert_eq!(
        ownership.identity,
        Some(ProvisioningIdentity::new("clamps", "test", "smoke"))
    );
    assert_eq!(
        ownership.token_id.as_deref(),
        Some("22222222222222222222222222222222")
    );
    assert_eq!(ownership.record_ids.len(), expected_record_count);
}

#[test]
fn disposable_cloudflare_journal_keeps_issued_token_after_dns_failure() {
    let run = tempfile::tempdir().unwrap();
    let addresses = addresses();
    let provider = fake_cloudflare(run.path(), true);

    let result = provision_disposable_ui(
        &disposable_request(run.path(), &addresses),
        &provider,
        || Ok(()),
    );
    let Err(error) = result else {
        panic!("fixture DNS failure unexpectedly succeeded")
    };

    assert!(error.to_string().contains("fixture DNS failure"));
    assert_eq!(provider.issued.get(), 1);
    let ownership = read_ownership(run.path());
    assert!(ownership.token_id.is_some());
    assert!(ownership.record_ids.is_empty());
}

#[test]
fn disposable_cloudflare_checks_vault_before_issuing_token() {
    let run = tempfile::tempdir().unwrap();
    let addresses = addresses();
    let provider = fake_cloudflare(run.path(), false);

    let result = provision_disposable_ui(
        &disposable_request(run.path(), &addresses),
        &provider,
        || Err(VaultError::Invalid("vault changed".into())),
    );
    let Err(error) = result else {
        panic!("changed vault unexpectedly issued a token")
    };

    assert!(error.to_string().contains("vault changed"));
    assert_eq!(provider.issued.get(), 0);
    assert!(read_ownership(run.path()).token_id.is_none());
}

#[test]
fn disposable_cloudflare_cleanup_removes_only_journaled_owner_after_revocations() {
    let run = tempfile::tempdir().unwrap();
    let provider = fake_cloudflare(run.path(), false);
    create_cleanup_fixture(run.path(), &provider);

    assert!(cleanup_disposable_ui(&cleanup_request(run.path()), &provider).unwrap());

    assert_eq!(provider.removed.get(), 1);
    assert_eq!(provider.issued.get(), 2);
    assert_eq!(
        provider.revoked.borrow().as_slice(),
        [
            "id-for:skillet:test:clamps-smoke",
            "id-for:skillet:test:clamps-smoke:cleanup"
        ]
    );
    assert!(!run.path().join("cloudflare.json").exists());
    assert!(!cleanup_disposable_ui(&cleanup_request(run.path()), &provider).unwrap());
}

#[test]
fn disposable_cloudflare_cleanup_mismatch_has_no_provider_mutations() {
    let run = tempfile::tempdir().unwrap();
    let provider = fake_cloudflare(run.path(), false);
    create_cleanup_fixture(run.path(), &provider);
    let mut request = cleanup_request(run.path());
    request.instance = "other";

    assert!(cleanup_disposable_ui(&request, &provider).is_err());
    assert_eq!(provider.issued.get(), 1);
    assert_eq!(provider.removed.get(), 0);
    assert!(provider.revoked.borrow().is_empty());
    assert!(run.path().join("cloudflare.json").exists());
}

#[test]
fn disposable_cloudflare_cleanup_failure_retains_journal_for_retry() {
    let run = tempfile::tempdir().unwrap();
    let mut provider = fake_cloudflare(run.path(), false);
    create_cleanup_fixture(run.path(), &provider);
    provider.fail_dns = true;

    assert!(cleanup_disposable_ui(&cleanup_request(run.path()), &provider).is_err());
    assert_eq!(provider.issued.get(), 2);
    assert_eq!(provider.removed.get(), 0);
    assert_eq!(
        provider.revoked.borrow().as_slice(),
        ["id-for:skillet:test:clamps-smoke:cleanup"]
    );
    assert!(run.path().join("cloudflare.json").exists());
}
