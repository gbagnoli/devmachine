use super::*;
use crate::{backend::parse_domain, Backend, Connection, RunIdentity};
use std::path::PathBuf;

#[test]
fn native_xml_keeps_the_existing_boot_network_and_owned_disk_contract() {
    let tmp = tempfile::Builder::new()
        .prefix("vm&xml-")
        .tempdir()
        .unwrap();
    let store = ManifestStore::new(tmp.path(), crate::current_uid()).unwrap();
    let identity = RunIdentity::new("fixture", "xml").unwrap();
    let run = store
        .prepare_intent(
            &identity,
            Connection {
                backend: Backend::Native,
                uri: "qemu:///session".into(),
                runtime_dir: PathBuf::from(format!("/run/user/{}", crate::current_uid())),
            },
            2207,
            "fixture-revision",
        )
        .unwrap();
    fs::write(&run.disk, "disk").unwrap();
    fs::create_dir_all(run.ignition.parent().unwrap()).unwrap();
    fs::write(&run.ignition, "ignition").unwrap();

    let path = write_domain_xml(&store, &run, Path::new("/usr/bin/true")).unwrap();
    let xml = fs::read_to_string(path).unwrap();
    assert!(xml.contains("<backend type=\"passt\"/>"));
    assert!(xml.contains("<range start=\"2207\" to=\"22\"/>"));
    assert!(xml.contains("<sysinfo type=\"fwcfg\">"));
    assert!(xml.contains("<readonly/>"));
    assert!(xml.contains("vm&amp;xml"), "{xml}");
    parse_domain(&xml, "defined".into())
        .unwrap()
        .validate_owned(&run)
        .unwrap();
}

#[test]
fn encrypted_xml_owns_firmware_and_tpm_and_refuses_other_state() {
    let tmp = tempfile::tempdir().unwrap();
    let store = ManifestStore::new(tmp.path(), crate::current_uid()).unwrap();
    let identity = RunIdentity::new("fixture", "encrypted").unwrap();
    let run = store
        .prepare_intent_with_profile(
            &identity,
            Connection {
                backend: Backend::Native,
                uri: "qemu:///session".into(),
                runtime_dir: format!("/run/user/{}", crate::current_uid()).into(),
            },
            2208,
            "fixture-revision",
            crate::install::RootProfile::Tpm,
        )
        .unwrap();
    fs::write(&run.disk, "disk").unwrap();
    fs::create_dir_all(run.ignition.parent().unwrap()).unwrap();
    fs::write(&run.ignition, "ignition").unwrap();
    let path = write_domain_xml(&store, &run, Path::new("/usr/bin/true")).unwrap();
    let xml = fs::read_to_string(path).unwrap();
    let snapshot = parse_domain(&xml, "defined".into()).unwrap();
    snapshot.validate_owned(&run).unwrap();
    assert!(xml.contains("name=\"enrolled-keys\""));
    assert!(xml.contains("<sha256/>"));
    assert!(xml.contains("serial.log"));
    assert_eq!(
        fs::metadata(store.run_dir(&identity).join("serial.log"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    let mut flatpak = run.clone();
    flatpak.connection.backend = Backend::Flatpak;
    let xml = render(&flatpak, Path::new("/not-a-native-emulator")).unwrap();
    assert!(!xml.contains("<emulator>"));
    parse_domain(&xml, "defined".into())
        .unwrap()
        .validate_owned(&flatpak)
        .unwrap();
    let modified = xml.replace("/tpm\"", "/other-tpm\"");
    assert!(parse_domain(&modified, "defined".into())
        .unwrap()
        .validate_owned(&run)
        .is_err());
    let mut old_json = serde_json::to_value(&run).unwrap();
    old_json.as_object_mut().unwrap().remove("root_profile");
    let legacy: crate::VmRun = serde_json::from_value(old_json).unwrap();
    assert_eq!(
        legacy.root_profile,
        crate::install::RootProfile::Unencrypted
    );
}

#[test]
fn recovery_artifacts_cannot_follow_symlinks() {
    let (_tmp, store, identity) = crate::manifest::tests::legacy_run();
    let mut run = store.import(&identity).unwrap();
    run.root_profile = crate::install::RootProfile::Tpm;
    let recovery = store.run_dir(&identity).join("recovery");
    std::os::unix::fs::symlink("/tmp", recovery).unwrap();
    assert!(store.validate(&run, &identity).is_err());
}
