use super::*;
use std::{os::unix::net::UnixStream, sync::atomic::AtomicUsize};

fn request(broker: &Broker) -> Vec<u8> {
    let mut stream = UnixStream::connect(&broker.socket).unwrap();
    stream.write_all(b"skillet-sudo-askpass-v1\n").unwrap();
    let mut output = Vec::new();
    stream.read_to_end(&mut output).unwrap();
    output
}

#[test]
fn password_is_run_scoped_reused_and_reprompted_after_authentication_rejection() {
    let prompts = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&prompts);
    let broker = Broker::start(Arc::new(move || {
        count.fetch_add(1, Ordering::SeqCst);
        Ok(Zeroizing::new("synthetic-password".into()))
    }))
    .unwrap();
    assert_eq!(
        fs::metadata(&broker.socket).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(request(&broker), b"synthetic-password\n");
    broker.operation.fetch_add(1, Ordering::SeqCst);
    assert_eq!(request(&broker), b"synthetic-password\n");
    assert_eq!(prompts.load(Ordering::SeqCst), 1);
    assert_eq!(request(&broker), b"synthetic-password\n");
    assert_eq!(prompts.load(Ordering::SeqCst), 2);
    let directory = broker.socket.parent().unwrap().to_owned();
    drop(broker);
    assert!(!directory.exists());
}

#[test]
fn invalid_passwords_never_leave_the_broker_or_enter_error_text() {
    let broker = Broker::start(Arc::new(|| Ok(Zeroizing::new("secret\nextra".into())))).unwrap();
    assert_eq!(request(&broker), [] as [u8; 0]);
    let error = broker.error.lock().unwrap();
    assert!(!error.as_ref().unwrap().contains("secret"));
}

#[test]
fn cleanup_paths_must_match_a_fresh_private_mktemp_directory() {
    assert!(valid_directory("/var/tmp/skillet-sudo-Ab1234567890"));
    for path in [
        "/var/tmp/skillet-sudo-",
        "/var/tmp/skillet-sudo-Ab1234567890/other",
        "/var/tmp/skillet-sudo-../../etc",
        "/etc/skillet-sudo-Ab1234567890",
    ] {
        assert!(!valid_directory(path));
    }
}
