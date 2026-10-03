use super::*;
use crate::{manifest::tests::legacy_run, transport::GuestCommand};
use std::{
    cell::{Cell, RefCell},
    fs,
    os::unix::process::ExitStatusExt as _,
    process::Output,
};

struct FakeTransport {
    hashes: ArtifactHashes,
    fail_install: Cell<bool>,
    wrong_hash: Cell<bool>,
    present: Cell<bool>,
    calls: RefCell<Vec<(String, Vec<String>)>>,
}
impl GuestTransport for FakeTransport {
    fn execute(&self, command: &GuestCommand<'_>, _: Option<&[u8]>) -> Result<Output> {
        self.calls.borrow_mut().push((
            command.program.into(),
            command
                .arguments
                .iter()
                .map(|argument| (*argument).into())
                .collect(),
        ));
        let failed = (command.program == "sudo" && self.fail_install.get())
            || (command.program == "test" && (command.arguments[0] == "-L" || !self.present.get()));
        if command.program == "sudo"
            && !failed
            && command.arguments.last().unwrap().ends_with("/skillet")
        {
            self.present.set(true);
        }
        let stdout = if command.program == "sha256sum" {
            let hash = if self.wrong_hash.get() {
                "bad"
            } else if command.arguments[0].ends_with("skillet-fixture") {
                &self.hashes.host
            } else {
                &self.hashes.generic
            };
            format!("{hash}  {}\n", command.arguments[0]).into_bytes()
        } else if command.program == "stat" {
            b"755:0:0\n".to_vec()
        } else {
            Vec::new()
        };
        Ok(Output {
            status: std::process::ExitStatus::from_raw(i32::from(failed) << 8),
            stdout,
            stderr: Vec::new(),
        })
    }
    fn upload(&self, _: &Path, destination: &str) -> Result<()> {
        self.calls
            .borrow_mut()
            .push(("upload".into(), vec![destination.into()]));
        Ok(())
    }
}

#[test]
fn delivery_uses_selected_profile_and_verifies_both_binaries_without_changing_capture() {
    let (_tmp, store, identity) = legacy_run();
    let dir = store.run_dir(&identity);
    let host = dir.join("host-artifact");
    let generic = dir.join("generic-artifact");
    fs::write(&host, "new host executable").unwrap();
    fs::write(&generic, "new generic executable").unwrap();
    let run = store.load(&identity).unwrap();
    let original = run.captured.clone();
    let hashes = ArtifactHashes {
        host: sha256(&host).unwrap(),
        generic: sha256(&generic).unwrap(),
    };
    let transport = FakeTransport {
        hashes: hashes.clone(),
        fail_install: Cell::new(false),
        wrong_hash: Cell::new(false),
        present: Cell::new(false),
        calls: RefCell::default(),
    };
    assert_eq!(deliver(&run, &transport, &host, &generic).unwrap(), hashes);
    assert_eq!(run.captured, original);
    assert!(transport
        .calls
        .borrow()
        .iter()
        .any(|(program, args)| program == "sudo"
            && args.last().unwrap() == "/var/usrlocal/bin/skillet-fixture"));
    assert_eq!(
        transport
            .calls
            .borrow()
            .iter()
            .filter(|(program, _)| program == "sha256sum")
            .count(),
        2
    );
    transport.calls.borrow_mut().clear();
    assert_eq!(deliver(&run, &transport, &host, &generic).unwrap(), hashes);
    assert!(!transport
        .calls
        .borrow()
        .iter()
        .any(|(program, _)| program == "upload" || program == "sudo"));
}

#[test]
fn failed_install_or_wrong_hash_does_not_claim_deployment_success() {
    let (_tmp, store, identity) = legacy_run();
    let host = store.run_dir(&identity).join("host-artifact");
    fs::write(&host, "fixture").unwrap();
    let run = store.import(&identity).unwrap();
    let hashes = ArtifactHashes {
        host: sha256(&host).unwrap(),
        generic: sha256(&host).unwrap(),
    };
    let transport = FakeTransport {
        hashes,
        fail_install: Cell::new(true),
        wrong_hash: Cell::new(false),
        present: Cell::new(false),
        calls: RefCell::default(),
    };
    assert!(deliver(&run, &transport, &host, &host).is_err());
    assert!(store.load(&identity).unwrap().deployed.is_none());
    transport.fail_install.set(false);
    transport.wrong_hash.set(true);
    assert!(deliver(&run, &transport, &host, &host).is_err());
    assert!(store.load(&identity).unwrap().deployed.is_none());
    transport.wrong_hash.set(false);
    assert!(deliver(&run, &transport, &host, &host).is_ok());
}

#[test]
fn disposal_in_progress_refuses_delivery_before_remote_mutation() {
    let (_tmp, store, identity) = legacy_run();
    let mut run = store.load(&identity).unwrap();
    run.phase = Phase::ExternalCleanupComplete;
    let transport = FakeTransport {
        hashes: run.captured.clone().unwrap(),
        fail_install: Cell::new(false),
        wrong_hash: Cell::new(false),
        present: Cell::new(false),
        calls: RefCell::default(),
    };
    assert!(deliver(&run, &transport, Path::new("/absent"), Path::new("/absent")).is_err());
    assert!(transport.calls.borrow().is_empty());
}
