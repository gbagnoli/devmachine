use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};
use std::sync::atomic::Ordering;

fn fixture() -> PodmanConfig {
    let mut extra_config = BTreeMap::new();
    extra_config.insert(
        "Install".to_string(),
        vec!["WantedBy=multi-user.target".to_string()],
    );
    PodmanConfig {
        name: "unit-fixture".to_string(),
        image: "example.invalid/fixture:1".to_string(),
        network_attachments: Vec::new(),
        port_publications: Vec::new(),
        storage_dependency: None,
        process_identity: ProcessIdentity::ImageDefault,
        namespace_mapping: None,
        volumes: Vec::new(),
        secrets: vec![QuadletSecret {
            secret_name: "dummy".to_string(),
            target: SecretTarget::File {
                target_path: "/run/secrets/dummy".to_string(),
                mode: None,
                uid: None,
                gid: None,
            },
        }],
        config_revisions: Vec::new(),
        extra_config,
    }
}

fn clamps_network() -> PodmanNetwork {
    PodmanNetwork {
        unit_name: "clamps".to_string(),
        options: vec![
            "NetworkName=clamps".to_string(),
            "IPv6=true".to_string(),
            "DisableDNS=false".to_string(),
            "Driver=bridge".to_string(),
        ],
    }
}

#[test]
fn shared_network_is_created_separately_and_attached_idempotently() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    system.ensure_podman_secret("dummy", "first").unwrap();

    ensure_network(&system, &files, &clamps_network()).unwrap();
    let mut config = fixture();
    config
        .network_attachments
        .push(NetworkAttachment::Bridge("clamps".to_string()));
    container(&system, &files, config).unwrap();

    let managed_files = files.files.lock().unwrap();
    assert!(
        !managed_files.contains_key("/etc/containers/containers.conf.d/90-skillet-aardvark.conf")
    );
    let network = managed_files
        .get("/etc/containers/systemd/clamps.network")
        .unwrap();
    let network = String::from_utf8_lossy(network);
    assert!(network.contains("DisableDNS=false"));
    assert!(network.contains("Driver=bridge"));
    assert!(network.contains("IPv6=true"));
    assert!(network.contains("NetworkName=clamps"));

    let quadlet = managed_files
        .get("/etc/containers/systemd/unit-fixture.container")
        .unwrap();
    assert!(String::from_utf8_lossy(quadlet).contains("Network=clamps.network"));
    drop(managed_files);

    let mut repeated = fixture();
    repeated
        .network_attachments
        .push(NetworkAttachment::Bridge("clamps".to_string()));
    container(&system, &files, repeated).unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 1);
}

#[test]
fn host_dns_listener_policy_is_independently_idempotent() {
    let files = MockFiles::new();
    assert!(ensure_dns_listener_port(&files, 54).unwrap());
    assert!(!ensure_dns_listener_port(&files, 54).unwrap());
    assert_eq!(
        files.files.lock().unwrap()["/etc/containers/containers.conf.d/90-skillet-aardvark.conf"],
        b"[network]\ndns_bind_port=54\n"
    );
}

#[test]
fn typed_network_and_dual_stack_port_publications_render_quadlet_directives() {
    let system = MockSystem::new();
    system.ensure_podman_secret("dummy", "first").unwrap();
    let files = MockFiles::new();
    let mut config = fixture();
    config
        .network_attachments
        .push(NetworkAttachment::Bridge("clamps".to_string()));
    config.port_publications = vec![
        PortPublication {
            host_address: std::net::Ipv6Addr::UNSPECIFIED.into(),
            host_port: 53,
            container_port: 53,
            protocol: PortProtocol::Udp,
        },
        PortPublication {
            host_address: std::net::Ipv4Addr::UNSPECIFIED.into(),
            host_port: 53,
            container_port: 53,
            protocol: PortProtocol::Tcp,
        },
    ];
    container(&system, &files, config).unwrap();
    let quadlet = String::from_utf8(
        files.files.lock().unwrap()["/etc/containers/systemd/unit-fixture.container"].clone(),
    )
    .unwrap();
    assert!(quadlet.contains("Network=clamps.network"));
    assert!(quadlet.contains("PublishPort=[::]:53:53/udp"));
    assert!(quadlet.contains("PublishPort=0.0.0.0:53:53/tcp"));
}

#[test]
fn invalid_or_conflicting_typed_container_settings_fail_before_effects() {
    let system = MockSystem::new();
    system.ensure_podman_secret("dummy", "first").unwrap();
    let files = MockFiles::new();
    let mut config = fixture();
    config.network_attachments = vec![
        NetworkAttachment::Host,
        NetworkAttachment::Bridge("clamps".to_string()),
    ];
    assert!(matches!(
        container(&system, &files, config),
        Err(PodmanError::ConflictingContainerDirective("Network"))
    ));
    assert!(files.files.lock().unwrap().is_empty());

    let mut config = fixture();
    config.port_publications.push(PortPublication {
        host_address: std::net::Ipv4Addr::UNSPECIFIED.into(),
        host_port: 0,
        container_port: 53,
        protocol: PortProtocol::Udp,
    });
    assert!(matches!(
        container(&system, &files, config),
        Err(PodmanError::InvalidPortPublication)
    ));
    assert!(files.files.lock().unwrap().is_empty());
}

#[test]
fn shared_data_mount_dependency_is_rendered_and_invalid_dependency_fails_early() {
    let system = MockSystem::new();
    system.ensure_podman_secret("dummy", "first").unwrap();
    let files = MockFiles::new();
    let mut config = fixture();
    config.storage_dependency = Some(MountDependency::shared_service_data());
    container(&system, &files, config).unwrap();
    let quadlet = String::from_utf8(
        files.files.lock().unwrap()["/etc/containers/systemd/unit-fixture.container"].clone(),
    )
    .unwrap();
    for directive in [
        "Requires=skillet-data-prepare.service",
        "After=skillet-data-prepare.service",
        "BindsTo=var-lib-data.mount",
        "After=var-lib-data.mount",
        "AssertPathIsMountPoint=/var/lib/data",
    ] {
        assert!(quadlet.contains(directive), "missing {directive}");
    }

    let empty_files = MockFiles::new();
    let mut invalid = fixture();
    invalid.storage_dependency = Some(MountDependency {
        mount_path: std::path::PathBuf::from("/var/lib/data/../other"),
        mount_unit: "var-lib-data.mount".to_string(),
        prepare_unit: None,
    });
    assert!(matches!(
        container(&system, &empty_files, invalid),
        Err(PodmanError::InvalidStorageDependency("mount path"))
    ));
    assert!(empty_files.files.lock().unwrap().is_empty());
}

#[test]
fn changing_a_created_network_fails_before_replacing_its_quadlet() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    system.ensure_podman_secret("dummy", "first").unwrap();

    ensure_network(&system, &files, &clamps_network()).unwrap();

    let original = files
        .files
        .lock()
        .unwrap()
        .get("/etc/containers/systemd/clamps.network")
        .cloned()
        .unwrap();
    let mut changed = clamps_network();
    changed.options.push("Internal=true".to_string());
    assert!(matches!(
        ensure_network(&system, &files, &changed),
        Err(PodmanError::NetworkConfigChanged(name)) if name == "clamps"
    ));
    assert_eq!(
        files
            .files
            .lock()
            .unwrap()
            .get("/etc/containers/systemd/clamps.network"),
        Some(&original)
    );
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 0);
}

#[test]
fn repeat_apply_and_stopped_service() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    system.ensure_podman_secret("dummy", "first").unwrap();
    assert!(container(&system, &files, fixture()).unwrap());
    assert!(!container(&system, &files, fixture()).unwrap());
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 1);
    system.service_stop("unit-fixture").unwrap();
    assert!(!container(&system, &files, fixture()).unwrap());
    assert_eq!(system.start_count.load(Ordering::SeqCst), 1);
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 1);
    let quadlet = String::from_utf8(
        files.files.lock().unwrap()["/etc/containers/systemd/unit-fixture.container"].clone(),
    )
    .unwrap();
    assert!(!quadlet.lines().any(|line| line.starts_with("User=")));
}

#[test]
fn numeric_process_identity_does_not_implicitly_create_namespace_mapping() {
    let system = MockSystem::new();
    system.ensure_podman_secret("dummy", "first").unwrap();
    let files = MockFiles::new();
    let mut config = fixture();
    config.process_identity = ProcessIdentity::Numeric {
        uid: 1001,
        gid: 1002,
    };
    container(&system, &files, config).unwrap();
    let quadlet = String::from_utf8(
        files.files.lock().unwrap()["/etc/containers/systemd/unit-fixture.container"].clone(),
    )
    .unwrap();
    assert!(quadlet.contains("User=1001:1002"));
    assert!(!quadlet.contains("UIDMap="));
    assert!(!quadlet.contains("GIDMap="));
    assert!(files.files.lock().unwrap().get("/etc/subuid").is_none());
}

#[test]
fn named_identity_is_typed_and_conflicting_raw_identity_fails_before_effects() {
    let system = MockSystem::new();
    system.ensure_podman_secret("dummy", "first").unwrap();
    let files = MockFiles::new();
    let mut config = fixture();
    config.process_identity = ProcessIdentity::Named {
        user: "service-user".to_string(),
        group: Some("service-group".to_string()),
    };
    container(&system, &files, config).unwrap();
    let quadlet = String::from_utf8(
        files.files.lock().unwrap()["/etc/containers/systemd/unit-fixture.container"].clone(),
    )
    .unwrap();
    assert!(quadlet.contains("User=service-user:service-group"));

    let empty_files = MockFiles::new();
    let mut conflicting = fixture();
    conflicting
        .extra_config
        .entry("Container".to_string())
        .or_default()
        .push("User=other".to_string());
    assert!(matches!(
        container(&system, &empty_files, conflicting),
        Err(PodmanError::ConflictingIdentityDirective("User"))
    ));
    assert!(empty_files.files.lock().unwrap().is_empty());
}

#[test]
fn secret_rotation_and_interrupted_restart_recover() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    system.ensure_podman_secret("dummy", "first").unwrap();
    container(&system, &files, fixture()).unwrap();
    system.ensure_podman_secret("dummy", "second").unwrap();
    system.fail_restart_once.store(true, Ordering::SeqCst);
    assert!(container(&system, &files, fixture()).is_err());
    assert!(container(&system, &files, fixture()).is_ok());
    let after_recovery = system.restart_count.load(Ordering::SeqCst);
    assert!(container(&system, &files, fixture()).is_ok());
    assert_eq!(system.restart_count.load(Ordering::SeqCst), after_recovery);
}

#[test]
fn interrupted_reload_retries_unchanged_quadlet() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    system.ensure_podman_secret("dummy", "first").unwrap();
    system.fail_reload_once.store(true, Ordering::SeqCst);
    assert!(container(&system, &files, fixture()).is_err());
    assert!(container(&system, &files, fixture()).is_ok());
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 1);
}

#[test]
fn consumed_file_change_restarts_even_when_quadlet_is_unchanged() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    system.ensure_podman_secret("dummy", "first").unwrap();
    let mut first = fixture();
    first.config_revisions = vec![b"one".to_vec()];
    container(&system, &files, first).unwrap();
    let mut second = fixture();
    second.config_revisions = vec![b"two".to_vec()];
    assert!(!container(&system, &files, second).unwrap());
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 2);
    let mut repeat = fixture();
    repeat.config_revisions = vec![b"two".to_vec()];
    container(&system, &files, repeat).unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 2);
}

#[test]
fn subordinate_ranges_are_read_from_injected_files_and_render_user_maps() {
    let system = MockSystem::new();
    system.ensure_podman_secret("dummy", "payload").unwrap();
    system.user_identities.lock().unwrap().insert(
        "service".to_string(),
        skillet_core::system::UserIdentity {
            name: "service".to_string(),
            uid: 1234,
            primary_gid: 2345,
        },
    );
    let files = MockFiles::new();
    files.files.lock().unwrap().insert(
        "/etc/subuid".to_string(),
        b"other:300000:65536\nservice:200000:65536\n".to_vec(),
    );
    files.files.lock().unwrap().insert(
        "/etc/subgid".to_string(),
        b"service:400000:65536\n".to_vec(),
    );
    let mut config = fixture();
    config.process_identity = ProcessIdentity::Numeric { uid: 999, gid: 999 };
    config.namespace_mapping = Some(UserNamespaceMapping {
        host_user: HostUser::Name("service".to_string()),
    });
    container(&system, &files, config).unwrap();
    let quadlet = String::from_utf8(
        files.files.lock().unwrap()["/etc/containers/systemd/unit-fixture.container"].clone(),
    )
    .unwrap();
    assert!(quadlet.contains("User=999:999"));
    assert!(quadlet.contains("UIDMap=0:200000:999"));
    assert!(quadlet.contains("UIDMap=999:1234:1"));
    assert!(quadlet.contains("GIDMap=0:400000:999"));
    assert!(quadlet.contains("GIDMap=999:2345:1"));
}

#[test]
fn missing_or_invalid_subordinate_ranges_fail_closed() {
    let missing = MockFiles::new();
    assert!(matches!(
        discover_subid_range(&missing, "/etc/subuid", "service", "UID"),
        Err(PodmanError::MissingSubordinateRange { kind: "UID", .. })
    ));

    let invalid = MockFiles::new();
    invalid.files.lock().unwrap().insert(
        "/etc/subuid".to_string(),
        b"service:not-a-number:5\n".to_vec(),
    );
    assert!(matches!(
        discover_subid_range(&invalid, "/etc/subuid", "service", "UID"),
        Err(PodmanError::InvalidSubordinateRange { .. })
    ));

    let duplicate = MockFiles::new();
    duplicate.files.lock().unwrap().insert(
        "/etc/subuid".to_string(),
        b"service:100000:65536\nservice:200000:65536\n".to_vec(),
    );
    assert!(matches!(
        discover_subid_range(&duplicate, "/etc/subuid", "service", "UID"),
        Err(PodmanError::InvalidSubordinateRange { .. })
    ));
}

#[test]
fn numeric_container_identity_must_fit_the_subordinate_range() {
    assert!(matches!(
        validate_subordinate_range(
            SubordinateRange {
                start: 100_000,
                size: 999
            },
            "service",
            999
        ),
        Err(PodmanError::SubordinateRangeTooSmall { .. })
    ));
}

#[test]
fn filesystem_fake_enforces_object_types_and_injected_failures() {
    let files = MockFiles::new();
    let file_path = std::path::Path::new("/tmp/mock-file");
    files
        .ensure_file(file_path, b"data", None, None, None)
        .unwrap();
    assert!(matches!(
        files.ensure_directory(file_path, None, None, None),
        Err(FileError::NotADirectory(_))
    ));

    let directory = std::path::Path::new("/tmp/mock-directory");
    files.ensure_directory(directory, None, None, None).unwrap();
    assert!(matches!(
        files.ensure_file(directory, b"data", None, None, None),
        Err(FileError::NotAFile(_))
    ));

    files.fail_file_write_once.store(true, Ordering::SeqCst);
    assert!(files
        .ensure_file(
            std::path::Path::new("/tmp/failing-file"),
            b"data",
            None,
            None,
            None
        )
        .is_err());
    files.fail_directory_once.store(true, Ordering::SeqCst);
    assert!(files
        .ensure_directory(
            std::path::Path::new("/tmp/failing-directory"),
            None,
            None,
            None
        )
        .is_err());
}
