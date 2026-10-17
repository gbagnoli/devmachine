use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};
use std::sync::atomic::Ordering;

fn payload(key: &str) -> String {
    json!({"version":1,"api_key":key,"site":"datadoghq.eu","tags":["env:test"]}).to_string()
}

fn mounted_files() -> MockFiles {
    let files = MockFiles::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    files
}

fn runtime(network_monitoring: bool) -> RuntimeConfig<'static> {
    RuntimeConfig {
        hostname: "clamps-test-monitoring",
        network_monitoring,
        monitored_units: &[],
    }
}

fn run(
    system: &MockSystem,
    files: &MockFiles,
    key: &str,
    network: bool,
) -> Result<(), DatadogError> {
    let units = vec!["syncthing.service".into(), "btrbk.timer".into()];
    let mut config = runtime(network);
    config.monitored_units = &units;
    apply(system, files, &payload(key), &config)
}

#[test]
fn rootful_secrets_and_host_observation_are_explicit() {
    let system = MockSystem::new();
    let files = mounted_files();
    run(&system, &files, "0123456789abcdef0123456789abcdef", true).unwrap();
    let state = files.files.lock().unwrap();
    let quadlet =
        String::from_utf8_lossy(&state["/etc/containers/systemd/datadog-agent.container"]);
    for expected in [
        "Network=host",
        "Image=registry.datadoghq.com/agent:7",
        "AutoUpdate=registry",
        "DNS=1.1.1.1",
        "DNS=1.0.0.1",
        "User=0:0",
        "--pid=host",
        "--cgroupns=host",
        "--security-opt=label=disable",
        "AddCapability=SYS_ADMIN SYS_RESOURCE SYS_PTRACE NET_ADMIN NET_BROADCAST NET_RAW IPC_LOCK CHOWN PERFMON",
        "Volume=/run/podman:/run/podman:ro",
        "Environment=DOCKER_HOST=unix:///run/podman/podman.sock",
        "Environment=DD_USE_DOGSTATSD=true",
        "Environment=DD_DOGSTATSD_PORT=0",
        "Environment=DD_DOGSTATSD_SOCKET=/run/skillet-dogstatsd/dsd.socket",
        "Volume=/run/skillet-dogstatsd:/run/skillet-dogstatsd:rw",
        "DD_HOSTNAME=clamps-test-monitoring",
        "Secret=datadog_api_key,type=env,target=DD_API_KEY",
        "/sys/kernel/debug",
    ] {
        assert!(quadlet.contains(expected), "missing {expected}: {quadlet}");
    }
    assert!(!quadlet.contains("PublishPort="));
    assert!(!quadlet.contains("seccomp=unconfined"));
    assert!(!quadlet.contains("--privileged"));
    assert!(quadlet.contains("Volume=/sys/kernel/debug:/sys/kernel/debug:ro"));
    assert!(quadlet.contains("/etc/datadog-agent/conf.d/btrfs.d:ro"));
    assert!(!quadlet.contains("/etc/datadog-agent/conf.d:ro"));
    assert!(!quadlet.contains("Environment=DD_API_KEY="));
    assert!(quadlet.contains("Environment=DD_SITE=datadoghq.eu"));
    assert!(!quadlet.contains("Secret=datadog_site"));
    assert!(!system
        .podman_secrets
        .lock()
        .unwrap()
        .contains("datadog_site"));
    assert!(!quadlet.contains(":Z"));
    assert!(state
        .values()
        .all(|bytes| !String::from_utf8_lossy(bytes).contains("0123456789abcdef")));
    let systemd =
        String::from_utf8_lossy(&state["/etc/skillet/datadog/conf.d/systemd.d/conf.yaml"]);
    assert!(systemd.contains("btrbk.timer"));
    assert_eq!(
        files.directory_metadata.lock().unwrap()[DOGSTATSD_DIR].0,
        Some(0o700)
    );
    assert!(state.contains_key("/etc/tmpfiles.d/skillet-dogstatsd.conf"));
}

#[test]
fn instantiated_units_are_accepted_but_unsafe_names_are_rejected() {
    for (unit, valid) in [
        ("nut-driver@cyberpower.service", true),
        ("bad name.service", false),
        ("bad%H.service", false),
        ("bad\nunit.service", false),
    ] {
        let system = MockSystem::new();
        let files = mounted_files();
        let units = vec![unit.into()];
        let mut config = runtime(false);
        config.monitored_units = &units;
        assert_eq!(
            apply(
                &system,
                &files,
                &payload("0123456789abcdef0123456789abcdef"),
                &config
            )
            .is_ok(),
            valid
        );
    }
}

#[test]
fn network_capabilities_are_optional() {
    let system = MockSystem::new();
    let files = mounted_files();
    run(&system, &files, "0123456789abcdef0123456789abcdef", false).unwrap();
    let state = files.files.lock().unwrap();
    let quadlet =
        String::from_utf8_lossy(&state["/etc/containers/systemd/datadog-agent.container"]);
    assert!(!quadlet.contains("AddCapability="));
    assert!(!quadlet.contains("/sys/kernel/debug"));
    assert!(quadlet.contains("DD_SYSTEM_PROBE_ENABLED=false"));
}

#[test]
fn startup_order_follows_caller_units_without_enabling_them() {
    let system = MockSystem::new();
    let files = mounted_files();
    let units = vec![
        "example-app.service".into(),
        "example-optional.service".into(),
    ];
    let mut config = runtime(false);
    config.monitored_units = &units;
    apply(
        &system,
        &files,
        &payload("0123456789abcdef0123456789abcdef"),
        &config,
    )
    .unwrap();
    let state = files.files.lock().unwrap();
    let quadlet =
        String::from_utf8_lossy(&state["/etc/containers/systemd/datadog-agent.container"]);
    assert!(quadlet.contains(
        "After=podman.socket network-online.target example-app.service example-optional.service"
    ));
    assert!(quadlet.lines().any(|line| line == "Requires=podman.socket"));
    assert!(!quadlet.contains("Wants="));
    assert!(!quadlet.contains("Requires=example"));
}

#[test]
fn rejects_agent_self_dependency_before_mutation() {
    let system = MockSystem::new();
    let files = mounted_files();
    let units = vec!["datadog-agent.service".into()];
    let mut config = runtime(false);
    config.monitored_units = &units;
    assert!(apply(
        &system,
        &files,
        &payload("0123456789abcdef0123456789abcdef"),
        &config,
    )
    .is_err());
    assert!(files.files.lock().unwrap().is_empty());
    assert!(system.podman_secrets.lock().unwrap().is_empty());
}

#[test]
fn repeats_are_noops_and_rotation_retries_failed_activation() {
    let system = MockSystem::new();
    let files = mounted_files();
    let first = "0123456789abcdef0123456789abcdef";
    let second = "1123456789abcdef0123456789abcdef";
    run(&system, &files, first, true).unwrap();
    let restarts = system.restart_count.load(Ordering::SeqCst);
    run(&system, &files, first, true).unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), restarts);
    system.fail_restart_once.store(true, Ordering::SeqCst);
    assert!(run(&system, &files, second, true).is_err());
    run(&system, &files, second, true).unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), restarts + 2);
}

#[test]
fn rejects_missing_storage_bad_credentials_and_conflicting_environment() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    assert!(run(&system, &files, "0123456789abcdef0123456789abcdef", true).is_err());
    assert!(run(&system, &files, "invalid", true).is_err());
    assert!(files.files.lock().unwrap().is_empty());
    assert!(system.podman_secrets.lock().unwrap().is_empty());
    let input = Input::parse(&payload("0123456789abcdef0123456789abcdef")).unwrap();
    assert!(input
        .render_for_environment("prod", "example-region")
        .is_err());
    let input = Input::parse(&payload("0123456789abcdef0123456789abcdef")).unwrap();
    assert!(input
        .render_for_environment("test", "example-region")
        .unwrap()
        .contains("env:test"));
}

#[test]
fn environment_and_region_follow_deployment_policy() {
    for (environment, production_region, expected_region) in [
        ("prod", "example-region", "example-region"),
        ("test", "example-region", "lab"),
        ("test", "another-region", "lab"),
    ] {
        let payload = json!({"version":1,"api_key":"0123456789abcdef0123456789abcdef",
            "site":"datadoghq.com","tags":["region:old", "region:duplicate", "role:sync"]})
        .to_string();
        let rendered = Input::parse(&payload)
            .unwrap()
            .render_for_environment(environment, production_region)
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(
            value["tags"],
            json!([
                "role:sync",
                format!("env:{environment}"),
                format!("region:{expected_region}")
            ])
        );
        let again = Input::parse(&rendered)
            .unwrap()
            .render_for_environment(environment, production_region)
            .unwrap();
        assert_eq!(rendered, again);
    }
    for (environment, region) in [("unknown", "valid"), ("prod", ""), ("test", "bad region")] {
        assert!(Input::parse(&payload("0123456789abcdef0123456789abcdef"))
            .unwrap()
            .render_for_environment(environment, region)
            .is_err());
    }
}

#[test]
fn resolver_configuration_is_typed_and_legacy_payloads_keep_independent_defaults() {
    let legacy = payload("0123456789abcdef0123456789abcdef");
    assert_eq!(
        Input::parse(&legacy).unwrap().dns_servers,
        default_dns_servers()
    );
    for resolvers in [
        json!([]),
        json!(["0.0.0.0"]),
        json!(["::"]),
        json!(["224.0.0.1"]),
        json!(["255.255.255.255"]),
        json!(["1.1.1.1", "1.1.1.1"]),
        json!(["not-an-ip"]),
    ] {
        let mut value: serde_json::Value = serde_json::from_str(&legacy).unwrap();
        value["dns_servers"] = resolvers;
        assert!(Input::parse(&value.to_string()).is_err());
    }
    let mut value: serde_json::Value = serde_json::from_str(&legacy).unwrap();
    value["dns_servers"] = json!(["9.9.9.9", "2620:fe::fe"]);
    let system = MockSystem::new();
    let files = mounted_files();
    let config = RuntimeConfig {
        hostname: "fixture",
        network_monitoring: false,
        monitored_units: &["example.service".into()],
    };
    apply(&system, &files, &value.to_string(), &config).unwrap();
    let state = files.files.lock().unwrap();
    let unit = String::from_utf8_lossy(&state["/etc/containers/systemd/datadog-agent.container"]);
    assert!(unit.contains("DNS=9.9.9.9"));
    assert!(unit.contains("DNS=2620:fe::fe"));
    assert!(!unit.contains("DNS=1.1.1.1"));
    assert!(!unit.contains("DD_SKIP_SSL_VALIDATION"));
}
