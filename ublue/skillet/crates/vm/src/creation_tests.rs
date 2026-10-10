use super::*;
use crate::{backend::DomainSnapshot, Connection};
use std::{
    cell::{Cell, RefCell},
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

struct FakeBackend {
    domain: RefCell<Option<DomainSnapshot>>,
    calls: RefCell<Vec<&'static str>>,
    fail_start: Cell<bool>,
}

impl VmBackend for FakeBackend {
    fn inspect(&self, _: &VmRun) -> Result<Option<DomainSnapshot>> {
        Ok(self.domain.borrow().clone())
    }
    fn define(&self, run: &VmRun, _: &Path) -> Result<()> {
        self.calls.borrow_mut().push("define");
        *self.domain.borrow_mut() = Some(snapshot(run, "shut off"));
        Ok(())
    }
    fn start(&self, _: &VmRun) -> Result<()> {
        self.calls.borrow_mut().push("start");
        if self.fail_start.get() {
            return Err(Error::Command {
                operation: "start".into(),
                code: Some(1),
            });
        }
        self.domain.borrow_mut().as_mut().unwrap().state = "running".into();
        Ok(())
    }
    fn reboot(&self, _: &VmRun) -> Result<()> {
        unreachable!()
    }
    fn stop(&self, _: &VmRun) -> Result<()> {
        unreachable!()
    }
    fn undefine(&self, _: &VmRun) -> Result<()> {
        unreachable!()
    }
}

fn snapshot(run: &VmRun, state: &str) -> DomainSnapshot {
    DomainSnapshot {
        boot: None,
        name: run.identity.domain_name(),
        uuid: run.uuid,
        state: state.into(),
        disks: vec![run.disk.clone(), run.ignition.clone()],
    }
}

fn fixture() -> (tempfile::TempDir, ManifestStore, RunIdentity, PathBuf) {
    fixture_for(Backend::Native)
}

fn fixture_for(backend: Backend) -> (tempfile::TempDir, ManifestStore, RunIdentity, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let store = ManifestStore::new(temp.path(), crate::current_uid()).unwrap();
    let identity = RunIdentity::new("clamps", "create-test").unwrap();
    let run = store
        .prepare_intent(
            &identity,
            Connection {
                backend,
                uri: "qemu:///session".into(),
                runtime_dir: match backend {
                    Backend::Native => PathBuf::from(format!("/run/user/{}", crate::current_uid())),
                    Backend::Flatpak => {
                        PathBuf::from(format!("/run/user/{}/skvm", crate::current_uid()))
                    }
                },
            },
            2205,
            "source-revision",
        )
        .unwrap();
    fs::write(&run.disk, b"disk").unwrap();
    fs::create_dir_all(run.ignition.parent().unwrap()).unwrap();
    fs::write(&run.ignition, b"ignition").unwrap();
    let dir = store.run_dir(&identity);
    fs::write(dir.join("skillet.sha256"), format!("{}\n", "a".repeat(64))).unwrap();
    fs::write(
        dir.join("skillet-generic.sha256"),
        format!("{}\n", "b".repeat(64)),
    )
    .unwrap();
    let emulator = temp.path().join("qemu-system-x86_64");
    fs::write(&emulator, b"qemu").unwrap();
    fs::set_permissions(&emulator, fs::Permissions::from_mode(0o700)).unwrap();
    (temp, store, identity, emulator)
}

#[test]
fn native_creation_defines_records_and_starts_owned_domain() {
    let (_temp, store, identity, emulator) = fixture();
    let backend = FakeBackend {
        domain: RefCell::new(None),
        calls: RefCell::default(),
        fail_start: Cell::new(false),
    };
    let run = create_native(
        &store,
        &identity,
        &backend,
        &emulator,
        "virsh\npodman\nyq\n",
    )
    .unwrap();
    assert_eq!(run.phase, Phase::Started);
    assert_eq!(*backend.calls.borrow(), ["define", "start"]);
    assert_eq!(
        fs::read_to_string(store.run_dir(&identity).join("domain.uuid")).unwrap(),
        format!("{}\n", run.uuid)
    );
    assert_eq!(
        fs::read_to_string(store.run_dir(&identity).join("tool-versions.txt")).unwrap(),
        "virsh\npodman\nyq\n"
    );
}

#[test]
fn native_creation_recovers_defined_domain_and_is_idempotent_when_running() {
    let (_temp, store, identity, emulator) = fixture();
    let preparing = store.load(&identity).unwrap();
    let backend = FakeBackend {
        domain: RefCell::new(Some(snapshot(&preparing, "shut off"))),
        calls: RefCell::default(),
        fail_start: Cell::new(false),
    };
    let run = create_native(&store, &identity, &backend, &emulator, "versions\n").unwrap();
    assert_eq!(run.phase, Phase::Started);
    assert_eq!(*backend.calls.borrow(), ["start"]);
    let run = create_native(&store, &identity, &backend, &emulator, "versions\n").unwrap();
    assert_eq!(run.phase, Phase::Started);
    assert_eq!(*backend.calls.borrow(), ["start"]);
}

#[test]
fn native_creation_rejects_unowned_existing_domain_without_mutating_it() {
    let (_temp, store, identity, emulator) = fixture();
    let run = store.load(&identity).unwrap();
    let mut foreign = snapshot(&run, "shut off");
    foreign.uuid = Uuid::new_v4();
    let backend = FakeBackend {
        domain: RefCell::new(Some(foreign)),
        calls: RefCell::default(),
        fail_start: Cell::new(false),
    };
    assert!(create_native(&store, &identity, &backend, &emulator, "versions\n").is_err());
    assert!(backend.calls.borrow().is_empty());
    assert_eq!(store.load(&identity).unwrap().phase, Phase::Preparing);
}

#[test]
fn failed_start_retains_defined_state_for_retry() {
    let (_temp, store, identity, emulator) = fixture();
    let backend = FakeBackend {
        domain: RefCell::new(None),
        calls: RefCell::default(),
        fail_start: Cell::new(true),
    };
    assert!(create_native(&store, &identity, &backend, &emulator, "versions\n").is_err());
    assert_eq!(store.load(&identity).unwrap().phase, Phase::Defined);
    assert!(!store.run_dir(&identity).join("domain.uuid").exists());

    backend.fail_start.set(false);
    let run = create_native(&store, &identity, &backend, &emulator, "versions\n").unwrap();
    assert_eq!(run.phase, Phase::Started);
    assert_eq!(*backend.calls.borrow(), ["define", "start", "start"]);
}

#[test]
fn flatpak_creation_uses_one_shot_definition_then_shared_recovery() {
    let (_temp, store, identity, _) = fixture_for(Backend::Flatpak);
    let backend = FakeBackend {
        domain: RefCell::new(None),
        calls: RefCell::default(),
        fail_start: Cell::new(false),
    };
    let run = create_flatpak(
        &store,
        &identity,
        &backend,
        "virt-install\nvirsh\npodman\nyq\n",
        |run| backend.define(run, Path::new("/flatpak-creator-test")),
    )
    .unwrap();
    assert_eq!(run.phase, Phase::Started);
    assert_eq!(*backend.calls.borrow(), ["define", "start"]);
}
