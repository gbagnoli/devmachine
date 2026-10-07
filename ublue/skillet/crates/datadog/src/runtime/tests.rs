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
        "User=0:0",
        "--pid=host",
        "--cgroupns=host",
        "--security-opt=label=disable",
        "AddCapability=",
        "DD_HOSTNAME=clamps-test-monitoring",
        "Secret=datadog_api_key,type=env,target=DD_API_KEY",
        "/sys/kernel/debug",
    ] {
        assert!(quadlet.contains(expected), "missing {expected}: {quadlet}");
    }
    assert!(!quadlet.contains("PublishPort="));
    assert!(!quadlet.contains("Environment=DD_API_KEY="));
    assert!(!quadlet.contains(":Z"));
    assert!(state
        .values()
        .all(|bytes| !String::from_utf8_lossy(bytes).contains("0123456789abcdef")));
    let systemd =
        String::from_utf8_lossy(&state["/etc/skillet/datadog/conf.d/systemd.d/conf.yaml"]);
    assert!(systemd.contains("btrbk.timer"));
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
    assert!(input.render_for_environment("prod").is_err());
    let input = Input::parse(&payload("0123456789abcdef0123456789abcdef")).unwrap();
    assert!(input
        .render_for_environment("test")
        .unwrap()
        .contains("env:test"));
}
