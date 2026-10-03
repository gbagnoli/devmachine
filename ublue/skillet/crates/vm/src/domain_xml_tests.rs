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

    let path = write_native_domain_xml(&store, &run, Path::new("/usr/bin/true")).unwrap();
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
