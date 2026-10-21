use super::*;
use std::{net::TcpListener, thread};

fn frame(lines: &str) -> String {
    format!("BEGIN LIST VAR test-ups\n{lines}END LIST VAR test-ups\n")
}

#[test]
fn finite_inventory_and_combined_status_are_rendered_without_identity_leaks() {
    let sample = parse(&frame("VAR test-ups ups.status \"OB LB\"\nVAR test-ups battery.charge \"19\"\nVAR test-ups battery.runtime \"120\"\nVAR test-ups device.serial \"private serial\"\nVAR test-ups driver.version \"2.8.1\"\n"), "test-ups").unwrap();
    let packets = render("test-ups", Some(&sample)).unwrap().join("\n");
    assert!(packets.contains("nut.battery.charge:19|g|#service:nut,ups:test-ups"));
    assert!(packets.contains("nut.battery.runtime:120|g"));
    assert!(packets.contains("nut.ups.status.online:0|g"));
    assert!(packets.contains("nut.ups.status.on_battery:1|g"));
    assert!(packets.contains("nut.ups.status.low_battery:1|g"));
    assert!(packets.contains("_sc|nut.power|2|"));
    assert!(!packets.contains("serial"));
    assert!(!packets.contains("driver.version"));
}

#[test]
fn malformed_stale_missing_and_nonfinite_readings_fail_closed() {
    for response in [
        "ERR DATA-STALE\n".to_string(),
        "BEGIN LIST VAR other\nEND LIST VAR other\n".to_string(),
        frame("VAR test-ups battery.charge \"100\"\n"),
        frame("VAR test-ups ups.status \"OL\"\nVAR test-ups battery.charge \"NaN\"\n"),
        frame("VAR test-ups ups.status \"OL\"\nVAR test-ups battery.charge \"inf\"\n"),
        frame("VAR test-ups ups.status \"OL\"\nVAR test-ups battery.charge \"100\"\nVAR test-ups battery.charge \"99\"\n"),
        frame("VAR test-ups ups.status \"OL\"\nVAR other ups.load \"10\"\n"),
        frame("VAR test-ups ups.status \"OL\"\nVAR test-ups ups.status \"OB\"\n"),
        frame("VAR test-ups ups.status \"\"\n"),
        frame("VAR test-ups ups.status \"OL\"\nVAR test-ups device.model \"bad\\n\"\n"),
        format!("{}extra\n", frame("VAR test-ups ups.status \"OL\"\n")),
    ] {
        assert!(parse(&response, "test-ups").is_err(), "accepted {response}");
    }
    assert!(unquote("\"escaped \\\"quote\\\" and \\\\slash\"").is_ok());
    assert!(parse(
        &frame("VAR test-ups ups.status \"OL\"\n").replace('\n', "\r\n"),
        "test-ups"
    )
    .is_ok());
}

#[test]
fn status_changes_reset_gauges_and_failure_does_not_invent_zero_readings() {
    for (status, power) in [
        ("OL", 0),
        ("OB", 1),
        ("OB LB", 2),
        ("OL FSD", 2),
        ("WAIT", 3),
    ] {
        let sample = parse(
            &frame(&format!("VAR test-ups ups.status \"{status}\"\n")),
            "test-ups",
        )
        .unwrap();
        assert!(render("test-ups", Some(&sample))
            .unwrap()
            .iter()
            .any(|s| s.starts_with(&format!("_sc|nut.power|{power}|"))));
    }
    assert_eq!(
        render("test-ups", None).unwrap(),
        ["_sc|nut.communication|2|#service:nut,ups:test-ups"]
    );
    for name in ["", "-ups", "ups\nLOGOUT", "ups,evil", "ups|g"] {
        assert!(render(name, None).is_err());
    }
}

#[test]
fn actual_tcp_request_is_read_only_and_datagrams_use_the_unix_socket() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut command = String::new();
        BufReader::new(stream.try_clone().unwrap())
            .read_line(&mut command)
            .unwrap();
        assert_eq!(command, "LIST VAR test-ups\n");
        stream
            .write_all(
                frame("VAR test-ups ups.status \"OL\"\nVAR test-ups ups.load \"11\"\n").as_bytes(),
            )
            .unwrap();
    });
    let snapshot = collect(address, "test-ups").unwrap();
    worker.join().unwrap();
    let packets = render("test-ups", Some(&snapshot)).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("dsd.socket");
    let receiver = UnixDatagram::bind(&path).unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let expected = packets.clone();
    let worker = thread::spawn(move || {
        for packet in &expected {
            let mut buffer = [0u8; 1024];
            let length = receiver.recv(&mut buffer).unwrap();
            assert_eq!(&buffer[..length], packet.as_bytes());
        }
    });
    submit(&path, &packets).unwrap();
    worker.join().unwrap();
    assert!(submit(&temp.path().join("absent.socket"), &packets).is_err());
}

#[test]
fn truncated_oversized_and_error_frames_are_bounded() {
    for response in [
        "ERR UNKNOWN-UPS\n".to_string(),
        "BEGIN LIST VAR test-ups\n".to_string(),
        format!("{}\n", "x".repeat(4097)),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut command = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut command)
                .unwrap();
            stream.write_all(response.as_bytes()).unwrap();
        });
        assert!(collect(address, "test-ups").is_err());
        worker.join().unwrap();
    }
}

#[test]
fn a_trickling_line_cannot_extend_the_read_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        for _ in 0..20 {
            if stream.write_all(b"x").is_err() {
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
    });
    let stream = TcpStream::connect(address).unwrap();
    let start = Instant::now();
    assert!(read_line_until(
        &mut BufReader::new(stream),
        start + Duration::from_millis(80)
    )
    .is_err());
    assert!(start.elapsed() < Duration::from_secs(1));
    worker.join().unwrap();
}
