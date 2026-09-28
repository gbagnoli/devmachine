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
        networks: Vec::new(),
        user: ContainerUser {
            container_uid: 0,
            container_gid: 0,
            host_user: None,
        },
        create_host_user: false,
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
fn shared_network_quadlet_is_written_and_attached_idempotently() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    system.ensure_podman_secret("dummy", "first").unwrap();

    let mut config = fixture();
    config.networks.push(clamps_network());
    container(&system, &files, config).unwrap();

    let managed_files = files.files.lock().unwrap();
    let dns_config = managed_files
        .get("/etc/containers/containers.conf.d/90-skillet-aardvark.conf")
        .unwrap();
    assert_eq!(dns_config, b"[network]\ndns_bind_port=54\n");
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
    repeated.networks.push(clamps_network());
    container(&system, &files, repeated).unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 1);
}

#[test]
fn changing_a_created_network_fails_before_replacing_its_quadlet() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    system.ensure_podman_secret("dummy", "first").unwrap();

    let mut initial = fixture();
    initial.networks.push(clamps_network());
    container(&system, &files, initial).unwrap();

    let original = files
        .files
        .lock()
        .unwrap()
        .get("/etc/containers/systemd/clamps.network")
        .cloned()
        .unwrap();
    let mut changed = clamps_network();
    changed.options.push("Internal=true".to_string());
    let mut config = fixture();
    config.networks.push(changed);
    assert!(matches!(
        container(&system, &files, config),
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
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 1);
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
