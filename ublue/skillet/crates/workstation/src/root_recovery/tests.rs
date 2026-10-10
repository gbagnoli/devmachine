use super::*;
use std::{cell::RefCell, os::unix::process::ExitStatusExt, process::Output};
const UUID: &str = "12345678-1234-1234-1234-123456789abc";
#[derive(Default)]
struct Store {
    record: Option<Record>,
    fail: bool,
}
impl RecoveryStore for Store {
    fn load(&self, _: &str) -> Result<Option<Record>, RecoveryError> {
        Ok(self.record.as_ref().map(|r| Record {
            key: Zeroizing::new(r.key.to_string()),
            volume: r.volume.clone(),
            verified: r.verified,
            header: r.header.clone(),
        }))
    }
    fn save(&mut self, _: &str, record: &Record) -> Result<(), RecoveryError> {
        if self.fail {
            return Err(RecoveryError::Invalid("save failed"));
        }
        self.record = Some(Record {
            key: Zeroizing::new(record.key.to_string()),
            volume: record.volume.clone(),
            verified: record.verified,
            header: record.header.clone(),
        });
        Ok(())
    }
}
#[derive(Default)]
struct Guest {
    calls: RefCell<Vec<String>>,
    fail: bool,
}
impl GuestTransport for Guest {
    fn upload(&self, _: &Path, _: &str) -> skillet_vm::Result<()> {
        self.calls.borrow_mut().push("upload".into());
        Ok(())
    }
    fn execute(
        &self,
        command: &GuestCommand<'_>,
        input: Option<&[u8]>,
    ) -> skillet_vm::Result<Output> {
        let action = command.arguments.last().copied().unwrap_or("");
        self.calls.borrow_mut().push(action.into());
        if let Some(input) = input {
            assert_eq!(input.len(), 128);
            assert!(!command.arguments.iter().any(|a| a.len() == 128));
        }
        let output = if command.program == "readlink" {
            "/dev/sda4"
        } else if command.program == "cat" {
            "clamps"
        } else if command.program == "findmnt" {
            "/dev/mapper/root[/var]"
        } else if command.arguments.contains(&"status") {
            "type: LUKS2\n device: /dev/sda4"
        } else if command.arguments.contains(&"luksUUID") {
            UUID
        } else {
            ""
        };
        Ok(Output {
            status: std::process::ExitStatus::from_raw(
                i32::from(self.fail && action == "enroll") * 256,
            ),
            stdout: if action == "header" {
                let mut bytes = vec![0; 4096];
                bytes[..8].copy_from_slice(b"LUKS\xba\xbe\x00\x02");
                bytes
            } else {
                output.as_bytes().to_vec()
            },
            stderr: vec![],
        })
    }
}
fn run(store: &mut Store, guest: &Guest) -> Result<(), RecoveryError> {
    provision("clamps", "path", Path::new("helper"), store, guest, || {
        Ok("a".repeat(128))
    })
}
#[test]
fn new_key_is_saved_verified_and_repeat_is_verify_only() {
    let mut store = Store::default();
    let guest = Guest::default();
    run(&mut store, &guest).unwrap();
    assert!(store.record.as_ref().unwrap().verified);
    guest.calls.borrow_mut().clear();
    run(&mut store, &guest).unwrap();
    assert!(!guest.calls.borrow().iter().any(|s| s == "enroll"));
}
#[test]
fn interrupted_enrollment_reuses_pending_key() {
    let mut store = Store::default();
    assert!(run(
        &mut store,
        &Guest {
            fail: true,
            ..Guest::default()
        }
    )
    .is_err());
    assert!(!store.record.as_ref().unwrap().verified);
    provision(
        "clamps",
        "path",
        Path::new("helper"),
        &mut store,
        &Guest::default(),
        || panic!("must reuse saved key"),
    )
    .unwrap();
    assert!(store.record.unwrap().verified);
}
#[test]
fn failed_persistence_and_volume_mismatch_never_enroll() {
    let mut store = Store {
        fail: true,
        ..Store::default()
    };
    let guest = Guest::default();
    assert!(run(&mut store, &guest).is_err());
    assert!(!guest.calls.borrow().iter().any(|s| s == "upload"));
    store.fail = false;
    store.record = Some(Record {
        key: Zeroizing::new("a".repeat(128)),
        volume: "another".into(),
        verified: true,
        header: None,
    });
    assert!(run(&mut store, &guest).is_err());
    assert!(!guest.calls.borrow().iter().any(|s| s == "upload"));
}

#[test]
fn wrong_host_never_saves() {
    let mut store = Store::default();
    assert!(provision(
        "beezelbot",
        "path",
        Path::new("helper"),
        &mut store,
        &Guest::default(),
        || panic!("must not generate")
    )
    .is_err());
    assert!(store.record.is_none());
}

#[test]
fn printable_generated_key_has_full_entropy_and_no_newline() {
    let a = generate_key().unwrap();
    let b = generate_key().unwrap();
    assert_eq!(a.len(), 128);
    assert!(a
        .bytes()
        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)));
    assert_ne!(a, b);
}

#[test]
fn verified_key_is_not_silently_reenrolled_on_verification_failure() {
    struct FailedVerify(Guest);
    impl GuestTransport for FailedVerify {
        fn upload(&self, source: &Path, target: &str) -> skillet_vm::Result<()> {
            self.0.upload(source, target)
        }
        fn execute(
            &self,
            command: &GuestCommand<'_>,
            input: Option<&[u8]>,
        ) -> skillet_vm::Result<Output> {
            let mut output = self.0.execute(command, input)?;
            if command.arguments.last() == Some(&"verify") {
                output.status = std::process::ExitStatus::from_raw(256);
            }
            Ok(output)
        }
    }
    let guest = FailedVerify(Guest::default());
    let mut store = Store {
        record: Some(Record {
            key: Zeroizing::new("a".repeat(128)),
            volume: UUID.into(),
            verified: true,
            header: None,
        }),
        fail: false,
    };
    assert!(provision(
        "clamps",
        "path",
        Path::new("helper"),
        &mut store,
        &guest,
        || panic!("must reuse")
    )
    .is_err());
    assert!(store.record.unwrap().verified);
    assert!(!guest.0.calls.borrow().iter().any(|s| s == "enroll"));
}

#[test]
fn missing_header_is_migrated_without_rotating_verified_key() {
    let mut store = Store {
        record: Some(Record {
            key: Zeroizing::new("a".repeat(128)),
            volume: UUID.into(),
            verified: true,
            header: None,
        }),
        fail: false,
    };
    let guest = Guest::default();
    run(&mut store, &guest).unwrap();
    assert_eq!(
        store
            .record
            .as_ref()
            .unwrap()
            .header
            .as_ref()
            .unwrap()
            .len(),
        4096
    );
    assert!(!guest.calls.borrow().iter().any(|action| action == "enroll"));
    store.fail = true;
    run(&mut store, &guest).unwrap(); // unchanged header must not rewrite the vault
}

#[test]
fn invalid_header_keeps_pending_recovery_for_retry() {
    struct BadHeader(Guest);
    impl GuestTransport for BadHeader {
        fn upload(&self, source: &Path, target: &str) -> skillet_vm::Result<()> {
            self.0.upload(source, target)
        }
        fn execute(
            &self,
            command: &GuestCommand<'_>,
            input: Option<&[u8]>,
        ) -> skillet_vm::Result<Output> {
            let mut output = self.0.execute(command, input)?;
            if command.arguments.last() == Some(&"header") {
                output.stdout.clear();
            }
            Ok(output)
        }
    }
    let mut store = Store::default();
    assert!(provision(
        "clamps",
        "path",
        Path::new("helper"),
        &mut store,
        &BadHeader(Guest::default()),
        || Ok("a".repeat(128))
    )
    .is_err());
    assert!(!store.record.as_ref().unwrap().verified);
    assert!(store.record.as_ref().unwrap().header.is_none());
    run(&mut store, &Guest::default()).unwrap();
    assert!(store.record.unwrap().verified);
}
