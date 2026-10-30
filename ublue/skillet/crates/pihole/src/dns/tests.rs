use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};

fn input() -> Input {
    Input {
        version: 1,
        records: vec![Record {
            names: vec!["printer.home.example".into(), "laser.home.example".into()],
            addresses: vec![
                "192.0.2.20".parse().unwrap(),
                "fd12:3456:789a::20".parse().unwrap(),
            ],
        }],
    }
}
#[test]
fn dual_stack_hosts_preserve_unrelated_settings_and_comments() {
    let input = input();
    let hosts = input.hosts().unwrap();
    assert_eq!(hosts.len(), 2);
    let source = b"# retained\n[dns]\nupstreams = [\"9.9.9.9\"]\nhosts = []\n[webserver.api]\npassword = \"retained\"\n";
    let (text, changed) = render(source, &hosts).unwrap();
    assert!(changed);
    assert!(text.contains("# retained"));
    assert!(text.contains("password = \"retained\""));
    assert!(text.contains("upstreams = [\"9.9.9.9\"]"));
    assert!(!render(text.as_bytes(), &hosts).unwrap().1);
    assert!(render(b"not = [toml", &hosts).is_err());
}
#[test]
fn validation_rejects_conflicts_injection_and_wrong_schema() {
    let mut bad = input();
    bad.records.push(bad.records[0].clone());
    assert!(bad.validate().is_err());
    for name in ["bad name", "*.example", "-bad", "a\nb", "192.0.2.2"] {
        let mut bad = input();
        bad.records[0].names = vec![name.into()];
        assert!(bad.validate().is_err());
    }
    for ip in ["0.0.0.0", "::", "ff02::1", "255.255.255.255"] {
        let mut bad = input();
        bad.records[0].addresses = vec![ip.parse().unwrap()];
        assert!(bad.validate().is_err());
    }
    assert!(Input::parse(r#"{"version":2,"records":[]}"#).is_err());
    assert!(Input::parse(&input().payload().unwrap()).is_ok());
}
#[test]
fn repeated_apply_is_noop_and_interrupted_activation_is_retried() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    files
        .ensure_file(
            Path::new(CONFIG),
            b"[dns]\nhosts = []\n",
            Some(0o640),
            &Ownership::default(),
        )
        .unwrap();
    apply(&system, &files, &input()).unwrap();
    let log = system
        .restart_count
        .load(std::sync::atomic::Ordering::SeqCst);
    apply(&system, &files, &input()).unwrap();
    assert_eq!(
        system
            .restart_count
            .load(std::sync::atomic::Ordering::SeqCst),
        log
    );
    files
        .ensure_file(
            &Path::new(STATE).join("applied"),
            b"interrupted",
            Some(0o644),
            &Ownership::default(),
        )
        .unwrap();
    apply(&system, &files, &input()).unwrap();
    assert!(
        system
            .restart_count
            .load(std::sync::atomic::Ordering::SeqCst)
            > log
    );
    let empty = Input {
        version: 1,
        records: vec![],
    };
    apply(&system, &files, &empty).unwrap();
    assert!(
        String::from_utf8(files.read_file(Path::new(CONFIG)).unwrap().unwrap())
            .unwrap()
            .contains("hosts = []")
    );
}

#[test]
fn failed_restart_keeps_pending_configuration_for_retry() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    files
        .ensure_file(
            Path::new(CONFIG),
            b"[dns]\nhosts = []\n",
            Some(0o640),
            &Ownership::default(),
        )
        .unwrap();
    system
        .fail_restart_once
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(apply(&system, &files, &input()).is_err());
    assert!(files
        .read_file(&Path::new(STATE).join("applied"))
        .unwrap()
        .is_none());
    assert!(!system.service_is_active("pihole.service").unwrap());
    apply(&system, &files, &input()).unwrap();
    assert!(system.service_is_active("pihole.service").unwrap());
}
