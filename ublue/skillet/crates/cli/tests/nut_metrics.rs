//! Real CLI transport coverage; no vault, provider API or running UPS required.
use std::{
    io::{BufRead as _, BufReader, Write as _},
    net::TcpListener,
    os::unix::net::UnixDatagram,
    process::Command,
    thread,
    time::Duration,
};

fn server(response: &'static str) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut request = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut request)
            .unwrap();
        assert_eq!(request, "LIST VAR test-ups\n");
        stream.write_all(response.as_bytes()).unwrap();
    });
    (address, worker)
}

#[test]
fn help_and_dry_run_work_without_credentials_or_a_datadog_socket() {
    let help = Command::new(env!("CARGO_BIN_EXE_skillet"))
        .args(["metrics", "nut", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("--dogstatsd-socket"));
    let (address, worker) = server("BEGIN LIST VAR test-ups\nVAR test-ups ups.status \"OL\"\nVAR test-ups battery.charge \"100\"\nEND LIST VAR test-ups\n");
    let output = Command::new(env!("CARGO_BIN_EXE_skillet"))
        .args([
            "metrics",
            "nut",
            "--ups",
            "test-ups",
            "--server",
            &address,
            "--dry-run",
        ])
        .env_remove("CREDENTIALS_DIRECTORY")
        .output()
        .unwrap();
    worker.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("nut.battery.charge:100|g"));
    assert!(text.contains("_sc|nut.communication|0|"));
}

#[test]
fn poll_failure_submits_failure_check_and_exits_nonzero() {
    let temp = tempfile::tempdir().unwrap();
    let socket_path = temp.path().join("dsd.socket");
    let receiver = UnixDatagram::bind(&socket_path).unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let (address, worker) = server("ERR DATA-STALE\n");
    let output = Command::new(env!("CARGO_BIN_EXE_skillet"))
        .args([
            "metrics",
            "nut",
            "--ups",
            "test-ups",
            "--server",
            &address,
            "--dogstatsd-socket",
        ])
        .arg(&socket_path)
        .output()
        .unwrap();
    worker.join().unwrap();
    assert!(!output.status.success());
    let mut packet = [0u8; 1024];
    let length = receiver.recv(&mut packet).unwrap();
    assert_eq!(
        &packet[..length],
        b"_sc|nut.communication|2|#service:nut,ups:test-ups"
    );
}

#[test]
fn successful_poll_delivers_the_complete_datagram_set() {
    let temp = tempfile::tempdir().unwrap();
    let socket_path = temp.path().join("dsd.socket");
    let receiver = UnixDatagram::bind(&socket_path).unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let receive_worker = thread::spawn(move || {
        let mut packets = Vec::new();
        for _ in 0..11 {
            let mut buffer = [0u8; 1024];
            let length = receiver.recv(&mut buffer).unwrap();
            packets.push(String::from_utf8(buffer[..length].to_vec()).unwrap());
        }
        packets
    });
    let (address, worker) = server("BEGIN LIST VAR test-ups\nVAR test-ups ups.status \"OB\"\nVAR test-ups battery.charge \"70\"\nEND LIST VAR test-ups\n");
    let output = Command::new(env!("CARGO_BIN_EXE_skillet"))
        .args([
            "metrics",
            "nut",
            "--ups",
            "test-ups",
            "--server",
            &address,
            "--dogstatsd-socket",
        ])
        .arg(&socket_path)
        .output()
        .unwrap();
    worker.join().unwrap();
    let packets = receive_worker.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(packets
        .iter()
        .any(|packet| packet.starts_with("nut.battery.charge:70|g|")));
    assert!(packets
        .iter()
        .any(|packet| packet.starts_with("_sc|nut.power|1|")));
}

struct AgentContainer(String);
impl Drop for AgentContainer {
    fn drop(&mut self) {
        let _ = Command::new("podman").args(["rm", "-f", &self.0]).output();
    }
}

fn podman(args: &[&str]) -> Vec<u8> {
    let output = Command::new("podman").args(args).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn start_agent(name: &str, mount: &str) {
    podman(&[
        "run",
        "-d",
        "--name",
        name,
        "--network",
        "none",
        "--entrypoint",
        "agent",
        "-v",
        mount,
        "-e",
        "DD_API_KEY=00000000000000000000000000000000",
        "-e",
        "DD_HOSTNAME=skillet-nut-test",
        "-e",
        "DD_TAGS=env:test region:lab",
        "-e",
        "DD_USE_DOGSTATSD=true",
        "-e",
        "DD_DOGSTATSD_PORT=0",
        "-e",
        "DD_DOGSTATSD_SOCKET=/run/skillet-dogstatsd/dsd.socket",
        "-e",
        "DD_DOGSTATSD_METRICS_STATS_ENABLE=true",
        "-e",
        "DD_APM_ENABLED=false",
        "-e",
        "DD_PROCESS_AGENT_ENABLED=false",
        "-e",
        "DD_SYSTEM_PROBE_ENABLED=false",
        "-e",
        "DD_LOGS_ENABLED=false",
        "-e",
        "DD_ENABLE_METADATA_COLLECTION=false",
        skillet_datadog::IMAGE,
        "run",
    ]);
}

#[test]
#[ignore = "requires Podman and the pinned official Agent image; no real key or network access"]
fn real_agent_accepts_nut_metrics_over_uds_without_udp() {
    use std::{os::unix::fs::PermissionsExt as _, time::Instant};
    let temp = tempfile::tempdir().unwrap();
    std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let name = format!(
        "skillet-nut-agent-{}",
        temp.path()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .trim_start_matches('.')
    );
    let _cleanup = AgentContainer(name.clone());
    let mount = format!("{}:/run/skillet-dogstatsd:rw,Z", temp.path().display());
    start_agent(&name, &mount);
    let socket = temp.path().join("dsd.socket");
    let deadline = Instant::now() + Duration::from_secs(50);
    while !socket.exists() {
        assert!(
            Instant::now() < deadline,
            "Agent did not create its Unix socket"
        );
        thread::sleep(Duration::from_millis(200));
    }
    let (address, worker) = server("BEGIN LIST VAR test-ups\nVAR test-ups ups.status \"OL\"\nVAR test-ups battery.charge \"100\"\nEND LIST VAR test-ups\n");
    let output = Command::new(env!("CARGO_BIN_EXE_skillet"))
        .args([
            "metrics",
            "nut",
            "--ups",
            "test-ups",
            "--server",
            &address,
            "--dogstatsd-socket",
        ])
        .arg(&socket)
        .output()
        .unwrap();
    worker.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    loop {
        let bytes = podman(&["exec", &name, "agent", "status", "-j"]);
        let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let stats = &report["dogstatsdStats"];
        if stats["MetricPackets"].as_u64().unwrap_or(0) >= 9
            && stats["ServiceCheckPackets"].as_u64().unwrap_or(0) >= 2
        {
            for key in ["MetricParseErrors", "ServiceCheckParseErrors", "UdpPackets"] {
                assert_eq!(stats[key], 0);
            }
            break;
        }
        assert!(
            Instant::now() < deadline,
            "Agent did not parse metrics and service checks"
        );
        thread::sleep(Duration::from_millis(200));
    }
    let stats = String::from_utf8(podman(&["exec", &name, "agent", "dogstatsd-stats"])).unwrap();
    assert!(stats.contains("nut.battery.charge"));
    assert!(stats.contains("service:nut ups:test-ups"));
    let udp = String::from_utf8(podman(&[
        "exec",
        &name,
        "cat",
        "/proc/net/udp",
        "/proc/net/udp6",
    ]))
    .unwrap();
    assert_eq!(udp.lines().count(), 2, "unexpected UDP listener: {udp}");
}
