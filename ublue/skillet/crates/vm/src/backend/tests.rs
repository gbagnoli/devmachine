use super::*;
use crate::manifest::tests::legacy_run;
use std::{cell::RefCell, collections::VecDeque};

#[derive(Default)]
struct FakeExecutor {
    responses: RefCell<VecDeque<VirshOutput>>,
    calls: RefCell<Vec<VirshInvocation>>,
}

impl VirshExecutor for FakeExecutor {
    fn execute(&self, invocation: &VirshInvocation) -> Result<VirshOutput> {
        self.calls.borrow_mut().push(invocation.clone());
        Ok(self
            .responses
            .borrow_mut()
            .pop_front()
            .expect("unexpected virsh call"))
    }
}

fn output(stdout: &str) -> VirshOutput {
    VirshOutput {
        success: true,
        code: Some(0),
        stdout: stdout.into(),
    }
}
fn executor(responses: Vec<VirshOutput>) -> FakeExecutor {
    FakeExecutor {
        responses: RefCell::new(responses.into()),
        calls: RefCell::default(),
    }
}
fn xml(run: &VmRun) -> String {
    format!("<domain><name>{}</name><uuid>{}</uuid><devices><disk device='disk'><source file='{}'/></disk><disk device='disk'><source file='{}'/></disk></devices></domain>", run.identity.domain_name(), run.uuid, run.disk.display(), run.ignition.display())
}

#[test]
fn inspect_owned_domain_uses_uuid_and_validates_disk_sources() {
    let (_tmp, store, identity) = legacy_run();
    let run = store.load(&identity).unwrap();
    let backend = VirshBackend::new(
        run.connection.clone(),
        run.owner_uid,
        Path::new("/wrapper"),
        executor(vec![
            output(&run.uuid.to_string()),
            output(&xml(&run)),
            output("running\n"),
        ]),
    )
    .unwrap();
    let snapshot = backend.inspect(&run).unwrap().unwrap();
    snapshot.validate_owned(&run).unwrap();
    assert_eq!(snapshot.state, "running");
    let calls = backend.executor.calls.borrow();
    assert_eq!(calls[0].program, Path::new("/usr/bin/virsh"));
    assert_eq!(
        calls[1].arguments.last().unwrap(),
        &OsString::from(run.uuid.to_string())
    );
    assert_eq!(
        calls[0].environment[&OsString::from("XDG_RUNTIME_DIR")],
        run.connection.runtime_dir.as_os_str()
    );
}

#[test]
fn missing_domain_requires_successful_lists_not_an_error_message() {
    let (_tmp, store, identity) = legacy_run();
    let run = store.load(&identity).unwrap();
    let backend = VirshBackend::new(
        run.connection.clone(),
        run.owner_uid,
        Path::new("/wrapper"),
        executor(vec![output(""), output("another-domain\n")]),
    )
    .unwrap();
    assert!(backend.inspect(&run).unwrap().is_none());
    let backend = VirshBackend::new(
        run.connection.clone(),
        run.owner_uid,
        Path::new("/wrapper"),
        executor(vec![VirshOutput {
            success: false,
            code: Some(1),
            stdout: String::new(),
        }]),
    )
    .unwrap();
    assert!(matches!(backend.inspect(&run), Err(Error::Command { .. })));
}

#[test]
fn reused_name_and_renamed_uuid_are_not_owned() {
    let (_tmp, store, identity) = legacy_run();
    let run = store.load(&identity).unwrap();
    let mut stranger = run.clone();
    stranger.uuid = Uuid::new_v4();
    let backend = VirshBackend::new(
        run.connection.clone(),
        run.owner_uid,
        Path::new("/wrapper"),
        executor(vec![
            output(""),
            output(&run.identity.domain_name()),
            output(&xml(&stranger)),
            output("running"),
        ]),
    )
    .unwrap();
    assert!(backend
        .inspect(&run)
        .unwrap()
        .unwrap()
        .validate_owned(&run)
        .is_err());
    let renamed = xml(&run).replace(&run.identity.domain_name(), "renamed-domain");
    assert!(parse_domain(&renamed, "running".into())
        .unwrap()
        .validate_owned(&run)
        .is_err());
    let wrong_disk = xml(&run).replace(&run.disk.display().to_string(), "/unrelated/disk");
    assert!(parse_domain(&wrong_disk, "running".into())
        .unwrap()
        .validate_owned(&run)
        .is_err());
}

#[test]
fn flatpak_adapter_uses_wrapper_and_recorded_runtime_and_uuid_actions() {
    let (_tmp, store, identity) = legacy_run();
    let mut run = store.load(&identity).unwrap();
    run.connection.backend = Backend::Flatpak;
    run.connection.runtime_dir = format!("/run/user/{}/skvm", run.owner_uid).into();
    let backend = VirshBackend::new(
        run.connection.clone(),
        run.owner_uid,
        Path::new("/source/bin/virsh"),
        executor(vec![output(""), output(""), output(""), output("")]),
    )
    .unwrap();
    backend.start(&run).unwrap();
    backend.reboot(&run).unwrap();
    backend.stop(&run).unwrap();
    backend.undefine(&run).unwrap();
    for call in backend.executor.calls.borrow().iter() {
        assert_eq!(call.program, Path::new("/source/bin/virsh"));
        assert_eq!(
            call.environment[&OsString::from("TEST_VM_LIBVIRT_RUNTIME_DIR")],
            run.connection.runtime_dir.as_os_str()
        );
        assert_eq!(
            call.arguments.last().unwrap(),
            &OsString::from(run.uuid.to_string())
        );
    }
}

#[test]
fn malformed_or_non_file_disks_fail_closed() {
    assert!(parse_domain("not XML", "running".into()).is_err());
    let (_tmp, store, identity) = legacy_run();
    let run = store.load(&identity).unwrap();
    let block = xml(&run).replace("source file=", "source dev=");
    assert!(parse_domain(&block, "running".into()).is_err());
}

#[test]
fn subprocess_arguments_are_literal_and_timeout_is_bounded() {
    let executor = ProcessExecutor {
        timeout: Duration::from_secs(2),
    };
    let invocation = VirshInvocation {
        program: "/usr/bin/printf".into(),
        arguments: vec!["%s".into(), "literal $(not-a-command); words".into()],
        environment: BTreeMap::new(),
    };
    assert_eq!(
        executor.execute(&invocation).unwrap().stdout,
        "literal $(not-a-command); words"
    );
    let executor = ProcessExecutor {
        timeout: Duration::from_millis(30),
    };
    let invocation = VirshInvocation {
        program: "/usr/bin/sleep".into(),
        arguments: vec!["5".into()],
        environment: BTreeMap::new(),
    };
    let start = Instant::now();
    assert!(matches!(executor.execute(&invocation), Err(Error::Timeout)));
    assert!(start.elapsed() < Duration::from_secs(2));
}
