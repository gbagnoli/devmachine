use super::*;
use std::os::unix::fs::symlink;

pub(crate) fn legacy_run() -> (tempfile::TempDir, ManifestStore, RunIdentity) {
    let tmp = tempfile::tempdir().unwrap();
    let identity = RunIdentity::new("fixture", "retained-2").unwrap();
    let store = ManifestStore::new(tmp.path(), users::get_current_uid()).unwrap();
    let dir = store.run_dir(&identity);
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("run.conf"), format!(
        "vm={}\nhost=fixture\nuri=qemu:///session\nssh_port=2207\nimage=/original/image\nartifact=/original/binary\nruntime_dir=/run/user/{}\nbackend=native\nsource_commit=original-revision\n",
        identity.domain_name(), users::get_current_uid()
    )).unwrap();
    fs::write(
        dir.join("domain.uuid"),
        "188ecb9b-3606-4f0f-9868-abcf366c824b\n",
    )
    .unwrap();
    fs::write(
        dir.join("skillet.sha256"),
        format!("{}  /original/host\n", "a".repeat(64)),
    )
    .unwrap();
    fs::write(
        dir.join("skillet-generic.sha256"),
        format!("{}  /original/generic\n", "b".repeat(64)),
    )
    .unwrap();
    (tmp, store, identity)
}

#[test]
fn identity_separates_profile_and_instance_and_rejects_path_input() {
    let identity = RunIdentity::new("fixture", "2-smoke").unwrap();
    assert_eq!(identity.host(), "fixture");
    assert_eq!(identity.instance(), "2-smoke");
    assert_eq!(identity.domain_name(), "fixture-test-2-smoke");
    for invalid in ["", "../escape", "two words", "UPPER", "-bad", "a\n"] {
        assert!(RunIdentity::new(invalid, "smoke").is_err());
        assert!(RunIdentity::new("fixture", invalid).is_err());
    }
}

#[test]
fn legacy_read_is_non_mutating_and_preserves_original_artifacts() {
    let (_tmp, store, identity) = legacy_run();
    let dir = store.run_dir(&identity);
    let before = fs::read(dir.join("run.conf")).unwrap();
    let run = store.load(&identity).unwrap();
    assert_eq!(run.ssh.port, 2207);
    assert_eq!(run.source_commit, "original-revision");
    assert_eq!(run.captured.unwrap().host, "a".repeat(64));
    assert!(!dir.join("vm.json").exists());
    assert_eq!(fs::read(dir.join("run.conf")).unwrap(), before);
}

#[test]
fn explicit_import_is_private_repeatable_and_preserves_external_journals() {
    let (_tmp, store, identity) = legacy_run();
    let dir = store.run_dir(&identity);
    fs::write(dir.join("cloudflare.json"), "opaque external ownership").unwrap();
    fs::write(dir.join("tailscale-pending"), "pending").unwrap();
    let first = store.import(&identity).unwrap();
    assert_eq!(store.import(&identity).unwrap(), first);
    assert_eq!(store.load(&identity).unwrap(), first);
    assert_eq!(
        fs::metadata(dir.join("vm.json"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::read_to_string(dir.join("cloudflare.json")).unwrap(),
        "opaque external ownership"
    );
    assert!(dir.join("tailscale-pending").exists());
    assert!(dir.join("run.conf").exists());
}

#[test]
fn legacy_import_rejects_ambiguous_or_partial_ownership_without_writing() {
    for corrupt in [
        "duplicate",
        "uuid",
        "identity",
        "port",
        "hash",
        "backend",
        "deployed",
    ] {
        let (_tmp, store, identity) = legacy_run();
        let dir = store.run_dir(&identity);
        let config_path = dir.join("run.conf");
        let config = fs::read_to_string(&config_path).unwrap();
        match corrupt {
            "duplicate" => fs::write(&config_path, format!("{config}ssh_port=2201\n")).unwrap(),
            "uuid" => fs::remove_file(dir.join("domain.uuid")).unwrap(),
            "identity" => {
                fs::write(&config_path, config.replace("host=fixture", "host=other")).unwrap();
            }
            "port" => {
                fs::write(&config_path, config.replace("ssh_port=2207", "ssh_port=22")).unwrap();
            }
            "hash" => fs::write(dir.join("skillet.sha256"), "bad hash").unwrap(),
            "backend" => fs::write(
                &config_path,
                config.replace("backend=native", "backend=unknown"),
            )
            .unwrap(),
            "deployed" => fs::write(dir.join("deployed-skillet.sha256"), "c".repeat(64)).unwrap(),
            _ => unreachable!(),
        }
        assert!(store.import(&identity).is_err(), "accepted {corrupt}");
        assert!(!dir.join("vm.json").exists());
    }
}

#[test]
fn json_manifest_rejects_owner_identity_version_and_paths() {
    let (_tmp, store, identity) = legacy_run();
    let original = store.load(&identity).unwrap();
    for field in [
        "version", "owner", "identity", "disk", "ssh", "runtime", "nil", "hash",
    ] {
        let mut run = original.clone();
        match field {
            "version" => run.version = 99,
            "owner" => run.owner_uid = run.owner_uid.wrapping_add(1),
            "identity" => run.identity = RunIdentity::new("other", "retained-2").unwrap(),
            "disk" => run.disk = "/outside/data.qcow2".into(),
            "ssh" => run.ssh.known_hosts = "/outside/known_hosts".into(),
            "runtime" => run.connection.runtime_dir = "/run/user/999999/../other".into(),
            "nil" => run.uuid = Uuid::nil(),
            "hash" => run.captured.as_mut().unwrap().host = "bad".into(),
            _ => unreachable!(),
        }
        assert!(store.validate(&run, &identity).is_err(), "accepted {field}");
    }
}

#[test]
fn symlinks_in_manifest_and_artifacts_are_refused() {
    for component in ["run.conf", "domain.uuid", "vm.json", "disk", "ancestor"] {
        let (tmp, store, identity) = legacy_run();
        let dir = store.run_dir(&identity);
        let target = tmp.path().join("unrelated");
        fs::write(&target, "not owned").unwrap();
        let path = match component {
            "disk" => dir.join(format!("{}.qcow2", identity.domain_name())),
            "ancestor" => dir.join("ssh"),
            _ => dir.join(component),
        };
        if path.exists() {
            fs::remove_file(&path).unwrap();
        }
        symlink(&target, &path).unwrap();
        assert!(store.load(&identity).is_err(), "accepted link {component}");
        assert_eq!(fs::read_to_string(target).unwrap(), "not owned");
    }
}

#[test]
fn partial_cleanup_can_load_when_owned_artifacts_are_already_absent() {
    let (_tmp, store, identity) = legacy_run();
    let mut run = store.import(&identity).unwrap();
    run.phase = Phase::DomainRemoved;
    store.save(&run).unwrap();
    assert_eq!(store.load(&identity).unwrap().phase, Phase::DomainRemoved);
}

#[test]
fn flatpak_and_native_connections_have_distinct_validated_runtime_paths() {
    let uid = users::get_current_uid();
    let mut connection = Connection {
        backend: Backend::Flatpak,
        uri: "qemu:///session".into(),
        runtime_dir: format!("/run/user/{uid}/skvm").into(),
    };
    assert!(connection.validate(uid).is_ok());
    connection.runtime_dir = format!("/run/user/{uid}/other").into();
    assert!(connection.validate(uid).is_err());
    connection.backend = Backend::Native;
    assert!(connection.validate(uid).is_ok());
    connection.uri = "qemu:///system".into();
    assert!(connection.validate(uid).is_err());
}
