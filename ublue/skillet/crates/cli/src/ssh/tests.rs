use super::*;

#[test]
fn workstation_alias_defers_user_port_identity_and_trust_paths_to_openssh() {
    let target = connection("clamps", None, None, None).unwrap();
    assert_eq!(target.address, "clamps");
    assert!(target.user.is_none());
    assert!(target.port.is_none());
    assert!(target.identity.is_none());
    assert!(target.known_hosts.is_none());
    let explicit = connection(
        "admin@example.invalid",
        Some(2202),
        Some(Path::new("/tmp/key")),
        Some(Path::new("/tmp/known")),
    )
    .unwrap();
    assert_eq!(explicit.user.as_deref(), Some("admin"));
    assert_eq!(explicit.port, Some(2202));
    assert_eq!(explicit.identity.as_deref(), Some(Path::new("/tmp/key")));
    assert_eq!(
        explicit.known_hosts.as_deref(),
        Some(Path::new("/tmp/known"))
    );
    assert!(connection("@bad", None, None, None).is_err());
    assert!(connection("bad@", None, None, None).is_err());
}
