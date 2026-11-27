use super::*;

#[test]
fn symlink_errors_identify_link_target_and_requested_path() {
    let temporary = tempfile::tempdir().unwrap();
    let target = temporary.path().join("real");
    fs::create_dir(&target).unwrap();
    let link = temporary.path().join("linked");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    // Output directories may not exist yet: identify a linked ancestor too.
    for requested in [link.clone(), link.join("new-bundle")] {
        let error = reject_symlinks(&requested).unwrap_err().to_string();
        assert!(error.contains(&format!(
            "symlink {} -> {}",
            link.display(),
            target.display()
        )));
        assert!(error.contains(&format!(
            "checking installation path {}",
            requested.display()
        )));
    }
    reject_symlinks(&target.join("new-bundle")).unwrap();
}

#[test]
fn output_allows_canonical_parent_aliases_but_never_reuses_an_existing_leaf() {
    let temporary = tempfile::tempdir().unwrap();
    let actual = temporary.path().join("actual");
    fs::create_dir(&actual).unwrap();
    let alias = temporary.path().join("alias");
    std::os::unix::fs::symlink(&actual, &alias).unwrap();
    assert_eq!(
        resolve_output(&alias.join("new-parent/bundle")).unwrap(),
        actual.join("new-parent/bundle")
    );
    assert!(resolve_output(&alias).is_err());
    let dangling = actual.join("dangling");
    std::os::unix::fs::symlink(actual.join("missing"), &dangling).unwrap();
    assert!(resolve_output(&dangling).is_err());
    assert!(resolve_output(&dangling.join("bundle")).is_err());
    assert!(resolve_output(&alias.join("../bundle")).is_err());
}

fn host() -> Value {
    yaml_serde::from_str(include_str!("../../../../../butane/clamps.bu")).unwrap()
}
fn passwd() -> Value {
    yaml_serde::from_str(include_str!("../../../../../butane/includes/passwd.bu")).unwrap()
}
#[test]
fn physical_profiles_preserve_vm_source() {
    let original = host();
    for profile in [RootProfile::Unencrypted, RootProfile::Tpm] {
        let mut config = original.clone();
        specialize(&mut config, "/dev/disk/by-id/ata-example", profile).unwrap();
        assert_eq!(
            config["storage"]["disks"][0]["device"].as_str(),
            Some("/dev/disk/by-id/ata-example")
        );
        assert_eq!(
            config["storage"]["filesystems"][0]["device"].as_str(),
            Some(if profile == RootProfile::Tpm {
                "/dev/mapper/root"
            } else {
                "/dev/disk/by-partlabel/root"
            })
        );
        assert_eq!(
            config["storage"]["disks"][0]["partitions"][0]["resize"].as_bool(),
            Some(true)
        );
    }
    assert_eq!(host(), original);
    assert_eq!(
        original["storage"]["disks"][0]["device"].as_str(),
        Some("/dev/vda")
    );
}
#[test]
fn disk_selection_rejects_partitions_ambient_devices_and_conflicts() {
    for disk in [
        "/dev/sda",
        "/dev/vda",
        "/dev/disk/by-id/x-part1",
        "/dev/disk/by-id/../x",
        "/dev/disk/by-id/",
        "/dev/disk/by-id/x\n",
    ] {
        assert!(specialize(&mut host(), disk, RootProfile::Unencrypted).is_err());
    }
    let mut config = host();
    config["storage"]["disks"][0]["device"] = Value::String("/dev/sda".into());
    assert!(specialize(&mut config, "/dev/disk/by-id/ata-example", RootProfile::Tpm).is_err());
}
#[test]
fn production_access_replaces_inherited_keys_and_hash() {
    let key =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIG5BCzEZkae5DssrD+FFi0a4YGYT55b57LbXW8SveytN operator";
    let mut value = passwd();
    let hash = format!("$6$testSalt${}", "A".repeat(86));
    configure_access(&mut value, Some(key), &hash).unwrap();
    for user in value["passwd"]["users"].as_sequence().unwrap() {
        assert_eq!(user["ssh_authorized_keys"].as_sequence().unwrap().len(), 1);
        assert_eq!(user["ssh_authorized_keys"][0].as_str(), Some(key));
    }
    assert!(value["passwd"]["users"][1]["password_hash"]
        .as_str()
        .is_some_and(|value| value == hash));
    assert!(configure_access(&mut passwd(), Some(key), "$6$malformed").is_err());
    assert!(value.get("storage").is_none());
    assert!(configure_access(&mut passwd(), Some("not-a-public-key"), "password").is_err());
}
#[test]
fn numeric_modes_and_live_media_require_manual_installation() {
    let mut value = host();
    skillet_vm::install::normalize_butane_modes(&mut value).unwrap();
    assert_eq!(value["storage"]["files"][0]["mode"].as_u64(), Some(420));
    let live = live_ignition(
        "/dev/disk/by-id/ata-example",
        RootProfile::Tpm,
        b"private ignition",
        b"helper",
        &["ssh-ed25519 test-public-key".into()],
        Some(b"certificate"),
    );
    assert!(live.get("systemd").is_none());
    assert_eq!(live["passwd"]["users"][0]["name"], "core");
    assert_eq!(
        live["passwd"]["users"][0]["sshAuthorizedKeys"][0],
        "ssh-ed25519 test-public-key"
    );
    assert!(serde_json::to_vec(&live).unwrap().len() < 262_144);
    assert_eq!(live["storage"]["files"][0]["mode"], 384);
    assert_eq!(live["storage"]["files"][3]["mode"], 448);
    assert_eq!(live["storage"]["files"].as_array().unwrap().len(), 5);
    assert_eq!(
        live["storage"]["files"][0]["contents"]["source"],
        format!(
            "data:;base64,{}",
            STANDARD.encode(hex::encode(Sha256::digest(b"private ignition")))
        )
    );
}

#[test]
fn requests_reject_missing_tpm_certificate_unknown_hosts_and_public_outputs() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let butane = workspace.join("../butane");
    let temporary = tempfile::tempdir().unwrap();
    let output = temporary.path().join("bundle");
    let mut request = Request {
        host: "clamps",
        disk: "/dev/disk/by-id/ata-example",
        mirror_disk: None,
        root_profile: RootProfile::Unencrypted,
        iso: Path::new("missing.iso"),
        output: &output,
        public_key: Some(Path::new("missing.pub")),
        password_hash: Path::new("missing.hash"),
        mok_certificate: None,
        butane: &butane,
        workspace: &workspace,
    };
    validate_request(&request).unwrap();
    request.mirror_disk = Some("/dev/disk/by-id/ata-example-two");
    assert!(validate_request(&request).is_err());
    request.host = "donbot";
    validate_request(&request).unwrap();
    request.mirror_disk = Some(request.disk);
    assert!(validate_request(&request).is_err());
    request.mirror_disk = None;
    request.host = "clamps";
    request.root_profile = RootProfile::Tpm;
    assert!(validate_request(&request).is_err());
    request.root_profile = RootProfile::Unencrypted;
    request.host = "unknown-profile";
    assert!(validate_request(&request).is_err());
    request.host = "clamps";
    let public_output = fs::canonicalize(&workspace).unwrap().join("install-bundle");
    request.output = &public_output;
    assert!(validate_request(&request).is_err());
    let alias = temporary.path().join("repository-alias");
    std::os::unix::fs::symlink(fs::canonicalize(&workspace).unwrap(), &alias).unwrap();
    let redirected = resolve_output(&alias.join("private-bundle")).unwrap();
    request.output = &redirected;
    assert!(validate_request(&request).is_err());
    request.output = &output;
    fs::create_dir(&output).unwrap();
    assert!(validate_request(&request).is_err());
}

#[test]
fn mirror_replaces_vm_disk_and_uses_native_btrfs_raid1_for_data() {
    let mut config: Value =
        yaml_serde::from_str(include_str!("../../../../../butane/donbot.bu")).unwrap();
    let primary = "/dev/disk/by-id/virtio-fixture-a";
    let secondary = "/dev/disk/by-id/virtio-fixture-b";
    mirror::configure(&mut config, primary, secondary).unwrap();
    assert_eq!(
        config["boot_device"]["mirror"]["devices"][0].as_str(),
        Some(primary)
    );
    assert_eq!(
        config["boot_device"]["mirror"]["devices"][1].as_str(),
        Some(secondary)
    );
    assert_eq!(
        config["storage"]["disks"][0]["partitions"][0]["label"].as_str(),
        Some("root-1")
    );
    assert_eq!(
        config["storage"]["disks"][1]["partitions"][0]["label"].as_str(),
        Some("root-2")
    );
    let options = config["storage"]["filesystems"][0]["options"]
        .as_sequence()
        .unwrap();
    assert_eq!(
        options
            .iter()
            .map(|option| option.as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "-f",
            "-d",
            "raid1",
            "-m",
            "raid1",
            "/dev/disk/by-partlabel/skillet-data-2"
        ]
    );
    let rendered = yaml_serde::to_string(&config).unwrap();
    assert!(rendered.contains("includes/mirrored-data.bu"));
    assert!(!rendered.contains("/dev/vda"));
}

#[test]
fn fleet_access_keeps_the_shared_lists_and_uses_core_keys_for_live_media() {
    let mut config: Value =
        yaml_serde::from_str(include_str!("../../../../../butane/includes/passwd.bu")).unwrap();
    let before = config["passwd"]["users"].clone();
    let hash = format!("$6$testSalt${}", "A".repeat(86));
    let live_keys = configure_access(&mut config, None, &hash).unwrap();
    let users = &config["passwd"]["users"];
    for index in 0..2 {
        assert_eq!(
            users[index]["ssh_authorized_keys"],
            before[index]["ssh_authorized_keys"]
        );
    }
    assert_eq!(
        yaml_serde::to_value(live_keys).unwrap(),
        before[0]["ssh_authorized_keys"]
    );
    assert_eq!(users[1]["password_hash"].as_str(), Some(hash.as_str()));
}
