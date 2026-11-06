use super::*;
use crate::manifest::tests::legacy_run;
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    fs,
    os::unix::process::ExitStatusExt as _,
    process::Output,
};

#[derive(Default)]
struct FakeTransport {
    files: RefCell<BTreeMap<String, String>>,
    fail_install: Cell<bool>,
    wrong_hash: Cell<bool>,
    fail_generic_rename: Cell<bool>,
    calls: RefCell<Vec<(String, Vec<String>)>>,
}
impl GuestTransport for FakeTransport {
    fn execute(&self, command: &GuestCommand<'_>, _: Option<&[u8]>) -> Result<Output> {
        self.calls.borrow_mut().push((
            command.program.into(),
            command.arguments.iter().map(|arg| (*arg).into()).collect(),
        ));
        let (program, args) = if command.program == "sudo" {
            if self.fail_install.get() {
                return Ok(output(1, ""));
            }
            (command.arguments[1], &command.arguments[2..])
        } else {
            (command.program, command.arguments)
        };
        let stdout = match (program, args) {
            ("test", ["-L", _]) => return Ok(output(1, "")),
            ("test", ["-e" | "-f", path]) => {
                return Ok(output(
                    i32::from(!self.files.borrow().contains_key(*path)),
                    "",
                ))
            }
            ("install", ["-o", "0", "-g", "0", "-m", "0755", source, destination]) => {
                let hash = self
                    .files
                    .borrow()
                    .get(*source)
                    .cloned()
                    .ok_or_else(|| Error::Invalid("unstaged test file".into()))?;
                self.files.borrow_mut().insert((*destination).into(), hash);
                String::new()
            }
            ("mv", ["-T", source, destination]) => {
                if self.fail_generic_rename.get() && *destination == "/var/usrlocal/bin/skillet" {
                    return Ok(output(1, ""));
                }
                let hash = self
                    .files
                    .borrow_mut()
                    .remove(*source)
                    .ok_or_else(|| Error::Invalid("missing test rename source".into()))?;
                self.files.borrow_mut().insert((*destination).into(), hash);
                String::new()
            }
            ("restorecon", ["-F", _]) => String::new(),
            ("sha256sum", [path]) => {
                let hash = self
                    .files
                    .borrow()
                    .get(*path)
                    .cloned()
                    .ok_or_else(|| Error::Invalid("missing test file".into()))?;
                format!(
                    "{}  {path}\n",
                    if self.wrong_hash.get() { "bad" } else { &hash }
                )
            }
            ("stat", ["--format=%a:%u:%g", path]) if self.files.borrow().contains_key(*path) => {
                "755:0:0\n".into()
            }
            ("rm", ["-f", paths @ ..]) => {
                for path in paths {
                    self.files.borrow_mut().remove(*path);
                }
                String::new()
            }
            _ => return Err(Error::Invalid("unsupported delivery test operation".into())),
        };
        Ok(output(0, &stdout))
    }
    fn upload(&self, source: &Path, destination: &str) -> Result<()> {
        self.calls
            .borrow_mut()
            .push(("upload".into(), vec![destination.into()]));
        self.files
            .borrow_mut()
            .insert(destination.into(), sha256(source)?);
        Ok(())
    }
}
fn output(code: i32, stdout: &str) -> Output {
    Output {
        status: std::process::ExitStatus::from_raw(code << 8),
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    }
}

#[test]
fn delivery_verifies_both_binaries_and_repeat_delivery_is_a_noop() {
    let (_tmp, store, identity) = legacy_run();
    let host = store.run_dir(&identity).join("host-artifact");
    let generic = store.run_dir(&identity).join("generic-artifact");
    fs::write(&host, "new host executable").unwrap();
    fs::write(&generic, "new generic executable").unwrap();
    let run = store.load(&identity).unwrap();
    let original = run.captured.clone();
    let hashes = ArtifactHashes {
        host: sha256(&host).unwrap(),
        generic: sha256(&generic).unwrap(),
    };
    let guest = FakeTransport::default();
    assert_eq!(deliver(&run, &guest, &host, &generic).unwrap(), hashes);
    assert_eq!(run.captured, original);
    assert_eq!(
        guest
            .files
            .borrow()
            .get("/var/usrlocal/bin/skillet-fixture"),
        Some(&hashes.host)
    );
    assert_eq!(
        guest.files.borrow().get("/var/usrlocal/bin/skillet"),
        Some(&hashes.generic)
    );
    assert_eq!(guest.files.borrow().len(), 2);
    let calls = guest.calls.borrow();
    assert_eq!(
        calls
            .iter()
            .filter(|(program, _)| program == "upload")
            .count(),
        2
    );
    assert_eq!(
        calls
            .iter()
            .filter(|(program, _)| program == "sha256sum")
            .count(),
        4
    );
    for (program, args) in calls.iter() {
        if program == "sudo" && args[1] == "install" {
            assert!(args.last().unwrap().contains(".new-"));
            assert_eq!(&args[2..8], ["-o", "0", "-g", "0", "-m", "0755"]);
        }
    }
    drop(calls);
    guest.calls.borrow_mut().clear();
    assert_eq!(deliver(&run, &guest, &host, &generic).unwrap(), hashes);
    assert!(!guest
        .calls
        .borrow()
        .iter()
        .any(|(program, _)| program == "upload" || program == "sudo"));
}

#[test]
fn failed_install_or_wrong_hash_does_not_claim_deployment_success() {
    let (_tmp, store, identity) = legacy_run();
    let path = store.run_dir(&identity).join("artifact");
    fs::write(&path, "binary").unwrap();
    let run = store.import(&identity).unwrap();
    let guest = FakeTransport::default();
    guest.fail_install.set(true);
    assert!(deliver(&run, &guest, &path, &path).is_err());
    assert!(store.load(&identity).unwrap().deployed.is_none());
    guest.fail_install.set(false);
    guest.wrong_hash.set(true);
    assert!(deliver(&run, &guest, &path, &path).is_err());
    assert!(store.load(&identity).unwrap().deployed.is_none());
    guest.wrong_hash.set(false);
    assert!(deliver(&run, &guest, &path, &path).is_ok());
}

#[test]
fn partial_pair_failure_keeps_old_generic_and_is_retryable() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("artifact");
    fs::write(&path, "binary").unwrap();
    let guest = FakeTransport::default();
    guest
        .files
        .borrow_mut()
        .insert("/var/usrlocal/bin/skillet".into(), "old generic".into());
    guest.fail_generic_rename.set(true);
    assert!(deliver_host("fixture", &guest, &path, &path).is_err());
    assert_eq!(
        guest.files.borrow().get("/var/usrlocal/bin/skillet"),
        Some(&"old generic".into())
    );
    assert_eq!(
        guest
            .files
            .borrow()
            .get("/var/usrlocal/bin/skillet-fixture"),
        Some(&sha256(&path).unwrap())
    );
    guest.fail_generic_rename.set(false);
    guest.calls.borrow_mut().clear();
    assert!(deliver_host("fixture", &guest, &path, &path).is_ok());
    assert_eq!(
        guest
            .calls
            .borrow()
            .iter()
            .filter(|(program, _)| program == "upload")
            .count(),
        1
    );
}

#[test]
fn disposal_or_invalid_host_refuses_delivery_before_io() {
    let (_tmp, store, identity) = legacy_run();
    let mut run = store.load(&identity).unwrap();
    run.phase = Phase::ExternalCleanupComplete;
    let guest = FakeTransport::default();
    assert!(deliver(&run, &guest, Path::new("/absent"), Path::new("/absent")).is_err());
    assert!(deliver_host(
        "../invalid",
        &guest,
        Path::new("/absent"),
        Path::new("/absent")
    )
    .is_err());
    assert!(guest.calls.borrow().is_empty());
}

#[test]
fn corrupt_staged_artifact_is_not_promoted() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("artifact");
    fs::write(&path, "binary").unwrap();
    let guest = FakeTransport::default();
    guest.wrong_hash.set(true);
    assert!(deliver_host("fixture", &guest, &path, &path).is_err());
    assert!(!guest
        .calls
        .borrow()
        .iter()
        .any(|(program, args)| program == "sudo" && args[1] == "mv"));
}
