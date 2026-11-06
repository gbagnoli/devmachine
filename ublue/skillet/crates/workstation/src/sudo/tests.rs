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
    let broker = Broker::start(
        Some(Arc::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(Zeroizing::new("synthetic-password".into()))
        })),
        None,
    )
    .unwrap();
    assert_eq!(
        fs::metadata(&broker.socket).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(request(&broker), b"synthetic-password\n");
    broker.complete(0, true).unwrap();
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
    let broker = Broker::start(
        Some(Arc::new(|| Ok(Zeroizing::new("secret\nextra".into())))),
        None,
    )
    .unwrap();
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

#[derive(Clone)]
struct ChangingIdentity(Arc<AtomicUsize>);
impl CredentialIdentity for ChangingIdentity {
    fn credential_scope(&self) -> Result<Option<[u8; 32]>> {
        Ok(Some(
            [if self.0.load(Ordering::SeqCst) == 0 {
                1
            } else {
                2
            }; 32],
        ))
    }
}
impl GuestTransport for ChangingIdentity {
    fn execute(&self, _: &GuestCommand<'_>, _: Option<&[u8]>) -> Result<Output> {
        panic!("changed trust must stop before guest execution")
    }
    fn upload(&self, _: &Path, _: &str) -> Result<()> {
        panic!("changed trust must stop before guest upload")
    }
}
impl ReverseUnixForward for ChangingIdentity {
    fn reverse_unix_forward(&self, _: &str, _: &Path) -> Result<UnixForward> {
        panic!("changed trust must stop before credential forwarding")
    }
}
#[test]
fn changed_ssh_identity_stops_before_any_privileged_effect() {
    let identity = Arc::new(AtomicUsize::new(0));
    let transport = SudoTransport::new(ChangingIdentity(identity.clone()), PathBuf::new(), None);
    assert!(transport.binding().unwrap().is_some());
    identity.store(1, Ordering::SeqCst);
    let error = transport
        .execute(
            &GuestCommand {
                program: "sudo",
                arguments: &["-n", "true"],
            },
            None,
        )
        .unwrap_err();
    assert!(error.to_string().contains("host trust changed"));
}
