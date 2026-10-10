use super::*;
use crate::{Backend, Connection, RunIdentity};
use std::{fs, os::unix::fs::symlink, path::PathBuf, process::Command};

fn fixture() -> (
    tempfile::TempDir,
    ManifestStore,
    VmRun,
    PathBuf,
    PathBuf,
    PathBuf,
    PathBuf,
    PathBuf,
) {
    let temp = tempfile::tempdir().unwrap();
    let store = ManifestStore::new(temp.path(), crate::current_uid()).unwrap();
    let identity = RunIdentity::new("clamps", "stage-test").unwrap();
    let run = store
        .prepare_intent(
            &identity,
            Connection {
                backend: Backend::Native,
                uri: "qemu:///session".into(),
                runtime_dir: PathBuf::from(format!("/run/user/{}", crate::current_uid())),
            },
            2204,
            "source-revision",
        )
        .unwrap();
    let config = temp.path().join("clamps.bu");
    let includes = temp.path().join("includes");
    fs::create_dir(&includes).unwrap();
    fs::write(
        &config,
        "variant: fcos\nversion: 1.6.0\nstorage:\n  files:\n    - path: /etc/hostname\n      mode: 420\n      contents:\n        inline: clamps\n",
    )
    .unwrap();
    fs::write(
        includes.join("skillet.bu"),
        "variant: fcos\nversion: 1.6.0\nstorage:\n  files:\n    - path: /var/usrlocal/bin/skillet\n      contents:\n        local: files/skillet\n    - path: /etc/other\n      contents:\n        inline: preserved\n",
    )
    .unwrap();
    fs::write(
        includes.join("skillet-clamps.bu"),
        "variant: fcos\nversion: 1.6.0\nstorage:\n  files:\n    - path: /var/usrlocal/bin/skillet-clamps\n      contents:\n        local: files/skillet-clamps\n",
    )
    .unwrap();
    fs::write(
        includes.join("passwd.bu"),
        "variant: fcos\nversion: 1.6.0\npasswd:\n  users:\n    - name: giacomo\n      ssh_authorized_keys:\n        - ssh-ed25519 existing\nstorage:\n  files:\n    - path: /usr/local/bin/force_pw_change.sh\n      contents:\n        inline: script\nsystemd:\n  units:\n    - name: force-pw-change.service\n      enabled: true\n",
    )
    .unwrap();
    fs::write(includes.join("extra.bu"), "variant: fcos\nversion: 1.6.0\n").unwrap();
    let host_binary = temp.path().join("skillet-clamps");
    let generic_binary = temp.path().join("skillet");
    let image = temp.path().join("image.qcow2");
    fs::write(&host_binary, b"host executable").unwrap();
    fs::write(&generic_binary, b"generic executable").unwrap();
    fs::write(&image, b"image bytes").unwrap();
    fs::create_dir_all(run.ssh.identity.parent().unwrap()).unwrap();
    fs::set_permissions(
        run.ssh.identity.parent().unwrap(),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let keygen = Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", ""])
        .arg("-f")
        .arg(&run.ssh.identity)
        .status()
        .unwrap();
    assert!(keygen.success());
    (
        temp,
        store,
        run,
        config,
        includes,
        host_binary,
        generic_binary,
        image,
    )
}

#[test]
fn staging_specializes_only_the_owned_copy_and_hashes_artifacts() {
    let (_temp, store, run, config, includes, host_binary, generic_binary, image) = fixture();
    stage_butane_source(
        &store,
        &run,
        &config,
        &includes,
        &host_binary,
        &generic_binary,
        &image,
    )
    .unwrap();
    let staged = store.run_dir(&run.identity).join("source");
    let host: Value =
        serde_yml::from_str(&fs::read_to_string(staged.join("clamps.bu")).unwrap()).unwrap();
    assert_eq!(
        host["storage"]["files"][0]["contents"]["inline"].as_str(),
        Some(run.guest_hostname.as_str())
    );
    let shared: Value =
        serde_yml::from_str(&fs::read_to_string(staged.join("includes/skillet.bu")).unwrap())
            .unwrap();
    assert_eq!(shared["storage"]["files"].as_sequence().unwrap().len(), 1);
    let host_include: Value = serde_yml::from_str(
        &fs::read_to_string(staged.join("includes/skillet-clamps.bu")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        host_include["storage"]["files"]
            .as_sequence()
            .unwrap()
            .len(),
        0
    );
    let passwd: Value =
        serde_yml::from_str(&fs::read_to_string(staged.join("includes/passwd.bu")).unwrap())
            .unwrap();
    let user = &passwd["passwd"]["users"][0];
    assert_eq!(user["ssh_authorized_keys"].as_sequence().unwrap().len(), 2);
    assert_eq!(passwd["storage"]["files"].as_sequence().unwrap().len(), 1);
    assert_eq!(passwd["systemd"]["units"].as_sequence().unwrap().len(), 0);
    assert_eq!(
        passwd["storage"]["files"][0]["path"].as_str(),
        Some("/etc/sudoers.d/90-skillet-vm")
    );
    assert_eq!(
        fs::read(staged.join("files/skillet-clamps")).unwrap(),
        b"host executable"
    );
    assert_eq!(
        fs::metadata(staged.join("files/skillet-clamps"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    assert!(store.run_dir(&run.identity).join("run.conf").is_file());
    assert!(store
        .run_dir(&run.identity)
        .join("skillet-generic.sha256")
        .is_file());
    assert!(fs::read_to_string(config)
        .unwrap()
        .contains("inline: clamps"));
}

#[test]
fn staging_converts_octal_mode_strings_to_butane_numbers() {
    let (_temp, store, run, config, includes, host_binary, generic_binary, image) = fixture();
    fs::write(
        &config,
        "variant: fcos\nversion: 1.6.0\nstorage:\n  files:\n    - path: /etc/hostname\n      mode: '0644'\n      contents:\n        inline: clamps\n",
    )
    .unwrap();
    stage_butane_source(
        &store,
        &run,
        &config,
        &includes,
        &host_binary,
        &generic_binary,
        &image,
    )
    .unwrap();
    let staged = store.run_dir(&run.identity).join("source/clamps.bu");
    let yaml: Value = serde_yml::from_str(&fs::read_to_string(staged).unwrap()).unwrap();
    assert_eq!(yaml["storage"]["files"][0]["mode"].as_u64(), Some(0o644));
}

#[test]
fn staging_is_idempotent_and_refuses_progressed_runs_or_linked_sources() {
    let (_temp, store, run, config, includes, host_binary, generic_binary, image) = fixture();
    let arguments = || {
        (
            &store,
            &run,
            &config,
            &includes,
            &host_binary,
            &generic_binary,
            &image,
        )
    };
    let args = arguments();
    stage_butane_source(args.0, args.1, args.2, args.3, args.4, args.5, args.6).unwrap();
    let source_dir = store.run_dir(&run.identity).join("source");
    let passwd_path = source_dir.join("includes/passwd.bu");
    let staged_once = fs::read(&passwd_path).unwrap();
    let args = arguments();
    stage_butane_source(args.0, args.1, args.2, args.3, args.4, args.5, args.6).unwrap();
    assert_eq!(fs::read(&passwd_path).unwrap(), staged_once);

    let symlink_config = config.with_file_name("linked-clamps.bu");
    symlink(&config, &symlink_config).unwrap();
    assert!(stage_butane_source(
        &store,
        &run,
        &symlink_config,
        &includes,
        &host_binary,
        &generic_binary,
        &image
    )
    .is_err());
    store.mark_started(&run.identity).unwrap();
    assert!(stage_butane_source(
        &store,
        &run,
        &config,
        &includes,
        &host_binary,
        &generic_binary,
        &image
    )
    .is_err());
}

#[test]
fn staging_accepts_account_only_passwd_includes_and_tpm_root() {
    let (_tmp, store, mut run, config, includes, host, generic, image) = fixture();
    fs::write(includes.join("passwd.bu"),"variant: fcos\nversion: 1.6.0\npasswd:\n  users:\n    - name: giacomo\n      ssh_authorized_keys: []\n").unwrap();
    let mut value: Value = serde_yml::from_str(&fs::read_to_string(&config).unwrap()).unwrap();
    value["storage"].as_mapping_mut().unwrap().insert(Value::String("filesystems".into()),serde_yml::from_str("- device: /dev/disk/by-partlabel/root\n  format: btrfs\n  label: root\n  wipe_filesystem: true\n").unwrap());
    fs::write(&config, serde_yml::to_string(&value).unwrap()).unwrap();
    run.root_profile = crate::install::RootProfile::Tpm;
    store.save(&run).unwrap();
    for _ in 0..2 {
        stage_butane_source(&store, &run, &config, &includes, &host, &generic, &image).unwrap();
    }
    let path = store.run_dir(&run.identity).join("source/clamps.bu");
    let staged: Value = serde_yml::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        staged["storage"]["filesystems"][0]["device"].as_str(),
        Some("/dev/mapper/root")
    );
    let passwd = fs::read_to_string(
        store
            .run_dir(&run.identity)
            .join("source/includes/passwd.bu"),
    )
    .unwrap();
    assert!(passwd.contains("90-skillet-vm"));
}
