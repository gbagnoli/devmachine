use super::*;

#[test]
fn workstation_defaults_preserve_trust_and_openssh_identity_selection() {
    let target = connection(
        "admin@example.invalid",
        22,
        None,
        None,
        Some(Path::new("/tmp/home")),
    )
    .unwrap();
    assert_eq!(target.known_hosts, Path::new("/tmp/home/.ssh/known_hosts"));
    assert!(target.identity.is_none());
    let target = connection(
        "admin@example.invalid",
        2202,
        Some(Path::new("/tmp/key")),
        Some(Path::new("/tmp/known")),
        None,
    )
    .unwrap();
    assert_eq!(target.identity.as_deref(), Some(Path::new("/tmp/key")));
    assert_eq!(target.port, 2202);
    assert_eq!(target.known_hosts, Path::new("/tmp/known"));
    assert!(connection("admin@example.invalid", 22, None, None, None).is_err());
    assert!(connection("invalid", 22, None, None, Some(Path::new("/tmp"))).is_err());
}
