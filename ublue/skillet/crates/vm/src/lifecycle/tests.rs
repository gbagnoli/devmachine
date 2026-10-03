use super::*;
use crate::{backend::DomainSnapshot, manifest::tests::legacy_run};
use std::cell::{Cell, RefCell};

struct FakeBackend {
    domain: RefCell<Option<DomainSnapshot>>,
    unavailable: Cell<bool>,
    fail_undefine: Cell<bool>,
    calls: RefCell<Vec<&'static str>>,
}
impl FakeBackend {
    fn new(run: &VmRun, present: bool) -> Self {
        Self {
            domain: RefCell::new(present.then(|| DomainSnapshot {
                name: run.identity.domain_name(),
                uuid: run.uuid,
                state: "running".into(),
                disks: vec![run.disk.clone(), run.ignition.clone()],
            })),
            unavailable: Cell::new(false),
            fail_undefine: Cell::new(false),
            calls: RefCell::default(),
        }
    }
}
impl VmBackend for FakeBackend {
    fn inspect(&self, _: &VmRun) -> Result<Option<DomainSnapshot>> {
        if self.unavailable.get() {
            return Err(Error::Command {
                operation: "list".into(),
                code: Some(1),
            });
        }
        Ok(self.domain.borrow().clone())
    }
    fn define(&self, _: &VmRun, _: &std::path::Path) -> Result<()> {
        unreachable!()
    }
    fn start(&self, _: &VmRun) -> Result<()> {
        unreachable!()
    }
    fn reboot(&self, _: &VmRun) -> Result<()> {
        unreachable!()
    }
    fn stop(&self, run: &VmRun) -> Result<()> {
        self.domain.borrow().as_ref().unwrap().validate_owned(run)?;
        self.calls.borrow_mut().push("stop");
        self.domain.borrow_mut().as_mut().unwrap().state = "shut off".into();
        Ok(())
    }
    fn undefine(&self, run: &VmRun) -> Result<()> {
        self.domain.borrow().as_ref().unwrap().validate_owned(run)?;
        if self.fail_undefine.get() {
            return Err(Error::Command {
                operation: "undefine".into(),
                code: Some(1),
            });
        }
        self.calls.borrow_mut().push("undefine");
        *self.domain.borrow_mut() = None;
        Ok(())
    }
}

#[test]
fn destruction_orders_external_domain_then_owned_artifacts() {
    let (tmp, store, identity) = legacy_run();
    let run = store.load(&identity).unwrap();
    let backend = FakeBackend::new(&run, true);
    let unrelated = tmp.path().join("unrelated");
    fs::write(&unrelated, "keep").unwrap();
    destroy(&store, &identity, &backend, |_| {
        assert!(backend.calls.borrow().is_empty());
        assert!(store.run_dir(&identity).exists());
        Ok(())
    })
    .unwrap();
    assert_eq!(*backend.calls.borrow(), ["stop", "undefine"]);
    assert!(!store.run_dir(&identity).exists());
    assert_eq!(fs::read_to_string(unrelated).unwrap(), "keep");
}

#[test]
fn already_absent_domain_still_runs_external_cleanup() {
    let (_tmp, store, identity) = legacy_run();
    let run = store.load(&identity).unwrap();
    let backend = FakeBackend::new(&run, false);
    let cleaned = Cell::new(false);
    destroy(&store, &identity, &backend, |_| {
        cleaned.set(true);
        Ok(())
    })
    .unwrap();
    assert!(cleaned.get());
    assert!(backend.calls.borrow().is_empty());
    assert!(!store.run_dir(&identity).exists());
}

#[test]
fn failed_external_cleanup_preserves_state_and_retry_converges() {
    let (_tmp, store, identity) = legacy_run();
    let run = store.load(&identity).unwrap();
    let backend = FakeBackend::new(&run, true);
    let journal = store.run_dir(&identity).join("tailscale-pending");
    fs::write(&journal, "pending").unwrap();
    assert!(destroy(&store, &identity, &backend, |_| Err(
        Error::ExternalCleanup("fixture failure".into())
    ))
    .is_err());
    assert_eq!(store.load(&identity).unwrap().phase, Phase::Started);
    assert!(journal.exists());
    assert!(backend.calls.borrow().is_empty());
    destroy(&store, &identity, &backend, |_| {
        fs::remove_file(&journal)?;
        Ok(())
    })
    .unwrap();
    assert!(!store.run_dir(&identity).exists());
}

#[test]
fn local_cleanup_retry_does_not_repeat_completed_external_cleanup() {
    let (_tmp, store, identity) = legacy_run();
    let run = store.load(&identity).unwrap();
    let backend = FakeBackend::new(&run, true);
    backend.fail_undefine.set(true);
    let cleanup_calls = Cell::new(0);
    assert!(destroy(&store, &identity, &backend, |_| {
        cleanup_calls.set(cleanup_calls.get() + 1);
        Ok(())
    })
    .is_err());
    assert_eq!(
        store.load(&identity).unwrap().phase,
        Phase::ExternalCleanupComplete
    );
    assert_eq!(cleanup_calls.get(), 1);
    backend.fail_undefine.set(false);
    destroy(&store, &identity, &backend, |_| {
        cleanup_calls.set(cleanup_calls.get() + 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(cleanup_calls.get(), 1);
    assert!(!store.run_dir(&identity).exists());
}

#[test]
fn inaccessible_or_mismatched_domain_refuses_external_and_local_cleanup() {
    for mismatch in [false, true] {
        let (_tmp, store, identity) = legacy_run();
        let run = store.load(&identity).unwrap();
        let backend = FakeBackend::new(&run, true);
        if mismatch {
            backend.domain.borrow_mut().as_mut().unwrap().uuid = uuid::Uuid::new_v4();
        } else {
            backend.unavailable.set(true);
        }
        assert!(destroy(&store, &identity, &backend, |_| panic!(
            "must not clean unverified ownership"
        ))
        .is_err());
        assert!(store.run_dir(&identity).join("run.conf").exists());
        assert!(!store.run_dir(&identity).join("vm.json").exists());
        assert!(backend.calls.borrow().is_empty());
    }
}

#[test]
fn changed_ownership_after_external_cleanup_is_refused() {
    let (_tmp, store, identity) = legacy_run();
    let run = store.load(&identity).unwrap();
    let backend = FakeBackend::new(&run, true);
    assert!(destroy(&store, &identity, &backend, |_| {
        backend.domain.borrow_mut().as_mut().unwrap().uuid = uuid::Uuid::new_v4();
        Ok(())
    })
    .is_err());
    assert_eq!(
        store.load(&identity).unwrap().phase,
        Phase::ExternalCleanupComplete
    );
    assert!(backend.calls.borrow().is_empty());
}

#[test]
fn concurrent_disposal_is_refused_before_effects() {
    let (_tmp, store, identity) = legacy_run();
    let run = store.load(&identity).unwrap();
    let backend = FakeBackend::new(&run, true);
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(store.run_dir(&identity).join(".vm.lock"))
        .unwrap();
    let _lock = Flock::lock(file, FlockArg::LockExclusiveNonblock).unwrap();
    assert!(matches!(
        destroy(&store, &identity, &backend, |_| panic!("busy")),
        Err(Error::Busy)
    ));
    assert!(backend.calls.borrow().is_empty());
}
