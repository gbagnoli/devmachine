use super::*;

#[test]
fn runtime_paths_keep_native_and_flatpak_sessions_distinct() {
    let uid = 1234;
    let standard = standard_runtime_dir(uid);
    assert_eq!(standard, PathBuf::from("/run/user/1234"));
    assert_eq!(
        runtime_socket(&standard.join("skillet-test-libvirt"), "virtqemud-sock"),
        PathBuf::from("/run/user/1234/skillet-test-libvirt/libvirt/virtqemud-sock")
    );
    assert_eq!(
        runtime_socket(&standard.join("skvm"), "virtstoraged-sock"),
        PathBuf::from("/run/user/1234/skvm/libvirt/virtstoraged-sock")
    );
}

#[test]
fn runtime_selection_preserves_the_qemu_session_uri() {
    for backend in [Backend::Native, Backend::Flatpak] {
        let connection = Connection {
            backend,
            uri: URI.into(),
            runtime_dir: match backend {
                Backend::Native => standard_runtime_dir(1234),
                Backend::Flatpak => standard_runtime_dir(1234).join("skvm"),
            },
        };
        assert!(connection.validate(1234).is_ok());
        assert_eq!(connection.uri, "qemu:///session");
    }
}
