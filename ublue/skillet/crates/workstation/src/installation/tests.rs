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

fn host() -> Value {
    serde_yml::from_str(include_str!("../../../../../butane/clamps.bu")).unwrap()
}
fn passwd() -> Value {
    serde_yml::from_str(include_str!("../../../../../butane/includes/passwd.bu")).unwrap()
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
    configure_access(&mut value, key, &hash).unwrap();
    for user in value["passwd"]["users"].as_sequence().unwrap() {
        assert_eq!(user["ssh_authorized_keys"].as_sequence().unwrap().len(), 1);
        assert_eq!(user["ssh_authorized_keys"][0].as_str(), Some(key));
    }
    assert!(value["passwd"]["users"][1]["password_hash"]
        .as_str()
        .is_some_and(|value| value == hash));
    assert!(configure_access(&mut passwd(), key, "$6$malformed").is_err());
    assert!(value.get("storage").is_none());
    assert!(configure_access(&mut passwd(), "not-a-public-key", "password").is_err());
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
        "ssh-ed25519 test-public-key",
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
        root_profile: RootProfile::Unencrypted,
        iso: Path::new("missing.iso"),
        output: &output,
        public_key: Path::new("missing.pub"),
        password_hash: Path::new("missing.hash"),
        mok_certificate: None,
        butane: &butane,
        workspace: &workspace,
    };
    validate_request(&request).unwrap();
    request.root_profile = RootProfile::Tpm;
    assert!(validate_request(&request).is_err());
    request.root_profile = RootProfile::Unencrypted;
    request.host = "unknown-profile";
    assert!(validate_request(&request).is_err());
    request.host = "clamps";
    let public_output = fs::canonicalize(&workspace).unwrap().join("install-bundle");
    request.output = &public_output;
    assert!(validate_request(&request).is_err());
    request.output = &output;
    fs::create_dir(&output).unwrap();
    assert!(validate_request(&request).is_err());
}
