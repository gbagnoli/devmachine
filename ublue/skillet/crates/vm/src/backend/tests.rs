use super::*;
use crate::manifest::tests::legacy_run;
use std::{cell::RefCell, collections::VecDeque, time::Instant};

#[derive(Default)]
struct FakeExecutor {
    responses: RefCell<VecDeque<VirshOutput>>,
    calls: RefCell<Vec<VirshInvocation>>,
}

#[derive(Default)]
struct FakeVirtInstallExecutor {
    responses: RefCell<VecDeque<VirshOutput>>,
    calls: RefCell<Vec<VirtInstallInvocation>>,
}

impl VirtInstallExecutor for FakeVirtInstallExecutor {
    fn execute(&self, invocation: &VirtInstallInvocation) -> Result<VirshOutput> {
        self.calls.borrow_mut().push(invocation.clone());
        Ok(self.responses.borrow_mut().pop_front().unwrap())
    }
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
fn capabilities_select_executable_x86_64_hvm_emulator() {
    let (_tmp, store, identity) = legacy_run();
    let run = store.load(&identity).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let emulator = dir.path().join("qemu-system-x86_64");
    std::fs::write(&emulator, b"test emulator").unwrap();
    std::fs::set_permissions(&emulator, std::fs::Permissions::from_mode(0o700)).unwrap();
    let capabilities = format!(
        "<capabilities><guest><os_type>hvm</os_type><arch name='aarch64'><emulator>/not/selected</emulator></arch><arch name='x86_64'><emulator>{}</emulator></arch></guest></capabilities>",
        emulator.display()
    );
    let backend = VirshBackend::new(
        run.connection.clone(),
        run.owner_uid,
        Path::new("/wrapper"),
        executor(vec![output(&capabilities)]),
    )
    .unwrap();
    assert_eq!(backend.x86_64_emulator(&run).unwrap(), emulator);
}

#[test]
fn flatpak_virt_install_preserves_uuid_paths_network_and_recorded_runtime() {
    let (_tmp, store, identity) = legacy_run();
    let mut run = store.load(&identity).unwrap();
    run.connection.backend = Backend::Flatpak;
    run.connection.runtime_dir = format!("/run/user/{}/skvm", run.owner_uid).into();
    fs::write(&run.disk, "disk").unwrap();
    fs::create_dir_all(run.ignition.parent().unwrap()).unwrap();
    fs::write(&run.ignition, "ignition").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let wrapper = directory.path().join("virt-install");
    fs::write(&wrapper, "fixture wrapper").unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700)).unwrap();
    let executor = FakeVirtInstallExecutor {
        responses: RefCell::new(vec![output("virt-install 4.0\n"), output("")].into()),
        calls: RefCell::default(),
    };
    let creator = FlatpakVirtInstall::new(&run, &wrapper, executor).unwrap();
    assert_eq!(creator.version().unwrap(), "virt-install 4.0\n");
    creator.define(&run).unwrap();
    let calls = creator.executor.calls.borrow();
    let invocation = &calls[1];
    let arguments: Vec<_> = invocation
        .arguments
        .iter()
        .map(|argument| argument.to_str().unwrap())
        .collect();
    let uuid = run.uuid.to_string();
    assert_eq!(invocation.program, wrapper);
    assert_eq!(
        invocation.environment[&OsString::from("TEST_VM_LIBVIRT_RUNTIME_DIR")],
        run.connection.runtime_dir.as_os_str()
    );
    assert!(arguments
        .windows(2)
        .any(|pair| pair == ["--uuid", uuid.as_str()]));
    assert!(arguments
        .windows(2)
        .any(|pair| pair == ["--network", "none"]));
    assert!(arguments.iter().any(|argument| {
        argument.starts_with("--qemu-commandline=-fw_cfg ")
            && argument.contains(&format!("hostfwd=tcp:127.0.0.1:{}-:22", run.ssh.port))
    }));
    assert!(arguments.iter().any(|argument| {
        argument
            == &format!(
                "path={},format=raw,bus=virtio,readonly=on",
                run.ignition.display()
            )
    }));
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
