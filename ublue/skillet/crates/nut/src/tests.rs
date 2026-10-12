use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};
use std::sync::atomic::Ordering;
const CONFIG: Config = Config {
    name: "cyberpower",
    vendor: 0x0764,
    product: 0x0501,
    description: "CyberPower CP1500 AVR",
};

#[test]
fn absent_hardware_converges_without_starting_and_preserves_password() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    assert_eq!(
        apply(&system, &files, CONFIG).unwrap(),
        Outcome::ConfiguredWithoutDevice
    );
    let before = files.read_file(Path::new(PASSWORD)).unwrap().unwrap();
    let all = files.files.lock().unwrap();
    assert_eq!(all["/etc/ups/nut.conf"], b"MODE=standalone\n");
    assert_eq!(
        all["/etc/ups/upsd.conf"],
        b"LISTEN 127.0.0.1 3493\nLISTEN ::1 3493\n"
    );
    let driver = String::from_utf8(all["/etc/ups/ups.conf"].clone()).unwrap();
    assert!(driver.contains("vendorid = 0764"));
    assert!(driver.contains("productid = 0501"));
    let monitor = String::from_utf8(all["/etc/ups/upsmon.conf"].clone()).unwrap();
    assert!(monitor.contains(" primary\n"));
    assert!(monitor.contains("POWERDOWNFLAG /run/nut/killpower"));
    let meta = files.metadata.lock().unwrap();
    assert_eq!(meta["/etc/ups/upsd.users"].0, Some(0o640));
    assert_eq!(meta[PASSWORD].0, Some(0o600));
    drop(meta);
    drop(all);
    apply(&system, &files, CONFIG).unwrap();
    assert_eq!(
        files.read_file(Path::new(PASSWORD)).unwrap().unwrap(),
        before
    );
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 0);
    assert_eq!(system.start_count.load(Ordering::SeqCst), 0);
    assert_eq!(system.usb_refresh_count.load(Ordering::SeqCst), 1);
}
#[test]
fn attached_hardware_activates_and_repeat_apply_does_not_restart() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    system
        .usb_devices
        .lock()
        .unwrap()
        .insert((CONFIG.vendor, CONFIG.product));
    assert_eq!(apply(&system, &files, CONFIG).unwrap(), Outcome::Active);
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 3);
    apply(&system, &files, CONFIG).unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), 3);
    assert_eq!(system.start_count.load(Ordering::SeqCst), 0);
    system.usb_devices.lock().unwrap().clear();
    apply(&system, &files, CONFIG).unwrap();
    assert!(system.service_is_active("nut-monitor.service").unwrap());
}
#[test]
fn interrupted_activation_retries_without_replacing_credentials() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    system
        .usb_devices
        .lock()
        .unwrap()
        .insert((CONFIG.vendor, CONFIG.product));
    system.fail_restart_once.store(true, Ordering::SeqCst);
    assert!(apply(&system, &files, CONFIG).is_err());
    let before = files.read_file(Path::new(PASSWORD)).unwrap().unwrap();
    assert_eq!(apply(&system, &files, CONFIG).unwrap(), Outcome::Active);
    assert_eq!(
        files.read_file(Path::new(PASSWORD)).unwrap().unwrap(),
        before
    );
    files
        .ensure_file(
            Path::new(PASSWORD),
            b"malformed",
            Some(0o600),
            &Ownership::default(),
        )
        .unwrap();
    assert!(matches!(
        apply(&system, &files, CONFIG),
        Err(NutError::InvalidCredential)
    ));
    assert_eq!(
        files.read_file(Path::new(PASSWORD)).unwrap().unwrap(),
        b"malformed"
    );
}
#[test]
fn caller_inputs_cannot_inject_units_or_configuration() {
    for config in [
        Config {
            name: "-ups",
            ..CONFIG
        },
        Config {
            name: "ups\n[evil]",
            ..CONFIG
        },
        Config {
            description: "bad\"\nsetting",
            ..CONFIG
        },
    ] {
        assert!(config.ups_config().is_err());
        let files = MockFiles::new();
        let system = MockSystem::new();
        assert!(apply(&system, &files, config).is_err());
        assert!(files.files.lock().unwrap().is_empty());
    }
}

#[test]
fn telemetry_timer_is_explicit_private_and_idempotent() {
    let files = MockFiles::new();
    let system = MockSystem::new();
    apply(&system, &files, CONFIG).unwrap();
    assert!(!files
        .files
        .lock()
        .unwrap()
        .contains_key("/etc/systemd/system/skillet-nut-metrics.timer"));
    apply_metrics(&system, &files, CONFIG, Path::new("/run/test/dsd.socket")).unwrap();
    let state = files.files.lock().unwrap();
    let service =
        String::from_utf8_lossy(&state["/etc/systemd/system/skillet-nut-metrics.service"]);
    let bootstrap = include_str!("../../../../butane/includes/skillet.bu");
    let executable = service
        .lines()
        .find_map(|line| line.strip_prefix("ExecStart="))
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    assert!(bootstrap.contains(&format!("- path: {executable}\n")));
    assert!(service.contains("/var/usrlocal/bin/skillet metrics nut --ups cyberpower"));
    assert!(service.contains("ConditionPathExists=/dev/skillet-ups-cyberpower"));
    assert!(service.contains("ConditionPathExists=/run/test/dsd.socket"));
    assert!(service.contains("ProtectSystem=strict"));
    assert!(!service.contains("Requires="));
    assert!(!service.contains("Wants="));
    drop(state);
    let before = system.restart_count.load(Ordering::SeqCst);
    apply_metrics(&system, &files, CONFIG, Path::new("/run/test/dsd.socket")).unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), before);
    assert!(!system.service_is_active("nut-server.service").unwrap());
    assert!(!system.service_is_active("datadog-agent.service").unwrap());
    system.fail_restart_once.store(true, Ordering::SeqCst);
    assert!(apply_metrics(
        &system,
        &files,
        CONFIG,
        Path::new("/run/changed/dsd.socket")
    )
    .is_err());
    apply_metrics(
        &system,
        &files,
        CONFIG,
        Path::new("/run/changed/dsd.socket"),
    )
    .unwrap();
    assert!(apply_metrics(&system, &files, CONFIG, Path::new("/run/inject%H.socket")).is_err());
}

use std::process::Command;
struct Container(String);
impl Drop for Container {
    fn drop(&mut self) {
        let _ = Command::new("podman").args(["rm", "-f", &self.0]).output();
    }
}
fn command(args: &[&str]) -> Vec<u8> {
    let result = Command::new("podman").args(args).output().unwrap();
    assert!(
        result.status.success(),
        "podman operation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}

fn write_consumer_configuration(root: &Path) {
    let files = MockFiles::new();
    let system = MockSystem::new();
    apply(&system, &files, CONFIG).unwrap();
    apply_metrics(&system, &files, CONFIG, Path::new("/run/test/dsd.socket")).unwrap();
    for (path, bytes) in files.files.lock().unwrap().iter() {
        if !path.starts_with("/etc/") {
            continue;
        }
        let relative = Path::new(path).strip_prefix("/etc/").unwrap();
        let local = root.join(relative);
        std::fs::create_dir_all(local.parent().unwrap()).unwrap();
        std::fs::write(&local, bytes).unwrap();
    }
}

fn simulate_consumer_configuration(root: &Path) {
    std::fs::write(
        root.join("ups/ups.conf"),
        "[cyberpower]\n driver = dummy-ups\n port = /etc/ups/simulated.dev\n mode = dummy-once\n",
    )
    .unwrap();
    std::fs::write(
        root.join("ups/simulated.dev"),
        "ups.status: OL\nbattery.charge: 100\n",
    )
    .unwrap();
    let monpath = root.join("ups/upsmon.conf");
    let monitor = std::fs::read_to_string(&monpath)
        .unwrap()
        .replace("/usr/bin/systemctl poweroff", "/usr/bin/true");
    std::fs::write(monpath, monitor).unwrap();
}

fn verify_consumer_units(name: &str) {
    command(&[
        "exec",
        name,
        "udevadm",
        "verify",
        "/etc/udev/rules.d/70-skillet-ups.rules",
    ]);
    // Unit verification checks the executable exists but never starts it. The
    // actual static CLI/protocol is exercised separately against the Agent.
    command(&["exec", name, "mkdir", "-p", "/var/usrlocal/bin"]);
    command(&[
        "exec",
        name,
        "ln",
        "-s",
        "/usr/bin/true",
        "/var/usrlocal/bin/skillet",
    ]);
    command(&[
        "exec",
        name,
        "systemd-analyze",
        "--man=no",
        "verify",
        "nut-driver@cyberpower.service",
        "nut-server.service",
        "nut-monitor.service",
        "skillet-nut-metrics.service",
        "skillet-nut-metrics.timer",
    ]);
}

#[test]
#[ignore = "requires the Fedora NUT test image and Podman"]
fn real_fedora_nut_accepts_config_and_monitors_a_safe_dummy_ups() {
    use std::time::{Duration, Instant};
    let temp = tempfile::tempdir().unwrap();
    let name = format!("skillet-nut-consumer-{}", uuid::Uuid::new_v4().simple());
    let _cleanup = Container(name.clone());
    command(&[
        "run",
        "-d",
        "--name",
        &name,
        "localhost/skillet-nut-runtime",
        "sleep",
        "300",
    ]);
    write_consumer_configuration(temp.path());
    let source = format!("{}/.", temp.path().display());
    let dest = format!("{name}:/etc/");
    command(&["cp", &source, &dest]);
    command(&["exec", &name, "chown", "-R", "root:nut", "/etc/ups"]);
    command(&["exec", &name, "chmod", "-R", "u=rwX,g=rX,o=", "/etc/ups"]);
    command(&[
        "exec",
        &name,
        "systemd-tmpfiles",
        "--create",
        "/usr/lib/tmpfiles.d/nut-common-tmpfiles.conf",
    ]);
    verify_consumer_units(&name);
    command(&["exec", &name, "upsdrvctl", "-t", "start", "cyberpower"]);
    let selected = command(&[
        "exec",
        &name,
        "/usr/libexec/nut-driver-enumerator.sh",
        "--get-device-for-service",
        "cyberpower",
    ]);
    assert_eq!(String::from_utf8_lossy(&selected).trim(), "cyberpower");
    // Only this disposable consumer receives simulation input. It has no USB
    // device passthrough; shutdown is replaced by a harmless command.
    simulate_consumer_configuration(temp.path());
    command(&["cp", &source, &dest]);
    command(&["exec", &name, "chown", "-R", "root:nut", "/etc/ups"]);
    command(&["exec", &name, "chmod", "-R", "u=rwX,g=rX,o=", "/etc/ups"]);
    command(&[
        "exec",
        &name,
        "/usr/libexec/nut/dummy-ups",
        "-a",
        "cyberpower",
    ]);
    command(&["exec", &name, "upsd"]);
    let start = Instant::now();
    loop {
        let output = Command::new("podman")
            .args(["exec", &name, "upsc", "cyberpower@127.0.0.1", "ups.status"])
            .output()
            .unwrap();
        if output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "OL" {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "dummy UPS never became readable"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    assert_eq!(
        String::from_utf8_lossy(&command(&[
            "exec",
            &name,
            "upsc",
            "cyberpower@[::1]",
            "ups.status"
        ]))
        .trim(),
        "OL"
    );
    command(&["exec", &name, "upsmon"]);
    std::thread::sleep(Duration::from_secs(1));
    command(&["exec", &name, "upsmon", "-c", "reload"]);
    let logs = command(&["exec", &name, "upsdrvctl", "status", "cyberpower"]);
    assert!(String::from_utf8_lossy(&logs).contains("cyberpower"));
    // Server startup refuses invalid syntax/listeners; verify only declared
    // loopback addresses remain configured, never publish a container host port.
    assert_eq!(
        std::fs::read_to_string(temp.path().join("ups/upsd.conf")).unwrap(),
        "LISTEN 127.0.0.1 3493\nLISTEN ::1 3493\n"
    );
}
