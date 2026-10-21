use super::*;
#[test]
fn only_completed_encrypted_local_backups_are_selected() {
    let value = json!({"state":"idle","agent_errors":{},"backups":[{"backup_id":"abc","agents":{"backup.local":{"protected":true,"size":1024}},"homeassistant_included":true,"database_included":true,"failed_addons":[],"failed_agent_ids":[],"failed_folders":[]}]});
    assert_eq!(completed(&value).unwrap().len(), 1);
    for path in ["state", "agent_errors"] {
        let mut bad = value.clone();
        bad[path] = Value::Null;
        assert!(completed(&bad).is_err());
    }
    let mut bad = value;
    bad["backups"][0]["agents"]["backup.local"]["protected"] = json!(false);
    assert!(completed(&bad).is_err());
}
fn archive(path: &Path, id: &str, protected: bool) {
    let mut tar = tar::Builder::new(File::create(path).unwrap());
    for (name, bytes) in [
        (
            "backup.json",
            serde_json::to_vec(
                &json!({"slug":id,"protected":protected,"homeassistant":{"version":"2026.10.1"}}),
            )
            .unwrap(),
        ),
        ("homeassistant.tar.gz", vec![1, 2, 3, 4]),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o600);
        header.set_size(bytes.len() as u64);
        header.set_cksum();
        tar.append_data(&mut header, name, &bytes[..]).unwrap();
    }
    tar.finish().unwrap();
}
#[test]
fn atomic_export_is_idempotent_and_refuses_corruption_or_plaintext() {
    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    let path = source.path().join("native.tar");
    archive(&path, "abc", true);
    let store = LocalArchives {
        source: source.path().into(),
        destination: destination.path().into(),
        uid: nix::unistd::getuid().as_raw(),
        gid: nix::unistd::getgid().as_raw(),
    };
    let item = Completed {
        backup_id: "abc".into(),
        size: fs::metadata(&path).unwrap().len(),
    };
    assert!(store.publish(&item).unwrap());
    assert!(!store.publish(&item).unwrap());
    fs::write(destination.path().join("ha-abc.tar"), b"bad").unwrap();
    assert!(store.publish(&item).is_err());
    archive(&path, "abc", false);
    assert!(store.publish(&item).is_err());
}

struct FakeApi {
    requests: Vec<Value>,
    configured: Value,
}
impl Api for FakeApi {
    fn request(&mut self, command: Value) -> Result<Value, Error> {
        self.requests.push(command.clone());
        if command["type"] == "backup/config/info" {
            Ok(self.configured.clone())
        } else {
            Ok(Value::Null)
        }
    }
}
#[test]
fn schedule_initialization_is_encrypted_and_retained_settings_are_not_overwritten() {
    let input = Settings {
        version: 1,
        host: "fixture".into(),
        runtime_hostname: "fixture-test".into(),
        api_token: "t".repeat(64),
        backup_password: "k".repeat(64),
    };
    let mut api = FakeApi {
        requests: vec![],
        configured: json!({"config":{"automatic_backups_configured":false}}),
    };
    configure(&mut api, &input).unwrap();
    let update = &api.requests[1];
    assert_eq!(update["agents"]["backup.local"]["protected"], true);
    assert_eq!(update["retention"]["copies"], 7);
    assert_eq!(update["schedule"]["recurrence"], "daily");
    api.requests.clear();
    api.configured = json!({"config":{"automatic_backups_configured":true,"agents":{"backup.local":{"protected":true}},"create_backup":{"agent_ids":["backup.local"],"password":input.backup_password}}});
    configure(&mut api, &input).unwrap();
    assert_eq!(api.requests.len(), 1);
    api.configured["config"]["create_backup"]["password"] = json!("changed outside vault");
    assert!(configure(&mut api, &input).is_err());
}
#[test]
fn export_rejects_symlinks_and_leaves_no_partial_destination() {
    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    let other = tempfile::NamedTempFile::new().unwrap();
    std::os::unix::fs::symlink(other.path(), source.path().join("unsafe.tar")).unwrap();
    let store = LocalArchives {
        source: source.path().into(),
        destination: destination.path().into(),
        uid: nix::unistd::getuid().as_raw(),
        gid: nix::unistd::getgid().as_raw(),
    };
    assert!(store
        .publish(&Completed {
            backup_id: "abc".into(),
            size: 512
        })
        .is_err());
    assert_eq!(fs::read_dir(destination.path()).unwrap().count(), 0);
}

#[test]
fn completed_inventory_rejects_failed_partial_and_unprotected_backups() {
    let valid = json!({"state":"idle","agent_errors":{},"backups":[{"backup_id":"fixture","agents":{"backup.local":{"protected":true,"size":512}},"homeassistant_included":true,"database_included":true,"failed_addons":[],"failed_agent_ids":[],"failed_folders":[]}]});
    for field in ["failed_addons", "failed_agent_ids", "failed_folders"] {
        let mut value = valid.clone();
        value["backups"][0][field] = json!(["failed"]);
        assert!(completed(&value).is_err());
    }
    for id in ["../outside", "", "nested/name"] {
        let mut value = valid.clone();
        value["backups"][0]["backup_id"] = json!(id);
        assert!(completed(&value).is_err());
    }
    let mut value = valid;
    value["backups"][0]["database_included"] = json!(false);
    assert!(completed(&value).is_err());
}

#[test]
fn interrupted_temporary_export_is_not_a_published_archive_and_retry_is_private() {
    use std::os::unix::fs::MetadataExt as _;
    let source = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    fs::write(
        destination
            .path()
            .join(".syncthing.homeassistant-interrupted.tmp"),
        b"incomplete",
    )
    .unwrap();
    let path = source.path().join("native.tar");
    archive(&path, "fixture", true);
    let store = LocalArchives {
        source: source.path().into(),
        destination: destination.path().into(),
        uid: nix::unistd::getuid().as_raw(),
        gid: nix::unistd::getgid().as_raw(),
    };
    let item = Completed {
        backup_id: "fixture".into(),
        size: fs::metadata(path).unwrap().len(),
    };
    assert!(store.publish(&item).unwrap());
    assert!(!store.publish(&item).unwrap());
    let metadata = fs::metadata(destination.path().join("ha-fixture.tar")).unwrap();
    assert_eq!(metadata.mode() & 0o777, 0o600);
    assert_eq!(metadata.uid(), store.uid);
    assert_eq!(metadata.gid(), store.gid);
    assert_eq!(
        fs::read_dir(destination.path())
            .unwrap()
            .filter(|entry| entry
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|ext| ext == "tar"))
            .count(),
        1
    );
}
