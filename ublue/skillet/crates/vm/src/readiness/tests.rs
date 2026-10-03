use super::*;
use crate::manifest::{tests::legacy_run, ArtifactHashes};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    os::unix::process::ExitStatusExt as _,
    process::{ExitStatus, Output},
};

#[derive(Default)]
struct FakeClock(Cell<Duration>);
impl WaitClock for FakeClock {
    fn elapsed(&self) -> Duration {
        self.0.get()
    }
    fn sleep(&self, duration: Duration) {
        self.0.set(self.0.get() + duration);
    }
}

#[allow(clippy::struct_excessive_bools)] // Independent failures exercised at each stage.
struct FakeGuest {
    files: RefCell<BTreeMap<String, String>>,
    calls: RefCell<Vec<String>>,
    no_ssh: bool,
    no_signed: bool,
    failed_bootstrap: bool,
    fail_unit: bool,
    profile: &'static str,
    origin: String,
    fault: &'static str,
}
impl Default for FakeGuest {
    fn default() -> Self {
        Self {
            files: RefCell::default(),
            calls: RefCell::default(),
            no_ssh: false,
            no_signed: false,
            failed_bootstrap: false,
            fail_unit: false,
            profile: "fixture",
            origin: "ostree-image-signed:docker://registry/fixture:latest".into(),
            fault: "none",
        }
    }
}

fn output(code: i32, stdout: impl Into<Vec<u8>>) -> Output {
    Output {
        status: ExitStatus::from_raw(code << 8),
        stdout: stdout.into(),
        stderr: Vec::new(),
    }
}

impl GuestTransport for FakeGuest {
    fn execute(&self, command: &GuestCommand<'_>, _: Option<&[u8]>) -> Result<Output> {
        self.calls
            .borrow_mut()
            .push(format!("{} {:?}", command.program, command.arguments));
        let (program, args) = if command.program == "sudo" {
            if self.no_ssh {
                return Ok(output(255, ""));
            }
            (&command.arguments[1], &command.arguments[2..])
        } else {
            (&command.program, command.arguments)
        };
        match (self.fault, *program, args) {
            ("selinux", "getenforce", []) => return Ok(output(0, "Permissive\n")),
            ("resolver", "readlink", ["-f", "/etc/resolv.conf"]) => {
                return Ok(output(0, "/wrong/resolver\n"))
            }
            ("result", "systemctl", ["show", "-p", "Result", ..]) => {
                return Ok(output(0, "exit-code\n"))
            }
            ("mask", "systemctl", ["is-enabled", ..]) => return Ok(output(0, "enabled\n")),
            ("brew", "/home/linuxbrew/.linuxbrew/bin/brew", _) => return Ok(output(1, "")),
            ("link", "readlink", ["-f", "/home/giacomo/.config/Brewfile"]) => {
                return Ok(output(0, "/wrong/Brewfile\n"))
            }
            ("artifact", "systemctl", ["restart", "skillet-apply.service"]) => {
                self.files
                    .borrow_mut()
                    .insert("/var/usrlocal/bin/skillet".into(), "bad".into());
            }
            _ => {}
        }
        let stdout = match (*program, args) {
            ("test", ["-e", "/run/ucore-bootstrap-ready"]) => return Ok(output(i32::from(self.no_signed), "")),
            ("systemctl", ["is-failed", "--quiet", "ucore-bootstrap.service"]) => return Ok(output(i32::from(!self.failed_bootstrap), "")),
            ("test", ["-L", _]) => return Ok(output(1, "")),
            ("test", ["-e" | "-f", path]) if path.starts_with("/var/usrlocal/") => {
                return Ok(output(i32::from(!self.files.borrow().contains_key(*path)), ""));
            }
            ("test", ["-e", path]) if path.starts_with("/var/lib/ucore-bootstrap/") => String::new(),
            ("install", ["-m", "0755", source, destination]) => {
                let hash = self.files.borrow().get(*source).cloned().ok_or_else(|| Error::Invalid("unstaged fake file".into()))?;
                self.files.borrow_mut().insert((*destination).into(), hash);
                String::new()
            }
            ("sha256sum", [path]) => {
                let hash = self.files.borrow().get(*path).cloned().ok_or_else(|| Error::Invalid("missing fake file".into()))?;
                format!("{hash}  {path}\n")
            }
            ("stat", ["--format=%a:%u:%g", path]) if self.files.borrow().contains_key(*path) => "755:0:0\n".into(),
            ("rm", ["-f", paths @ ..]) => { for path in paths { self.files.borrow_mut().remove(*path); } String::new() }
            ("systemctl", ["restart" | "start" | "reset-failed", _rest @ ..]) => {
                return Ok(output(i32::from(self.fail_unit), ""));
            }
            ("rpm-ostree", ["status", "--json"]) => serde_json::json!({"deployments":[{"booted":true,"container-image-reference":self.origin}]}).to_string(),
            ("rpm-ostree", ["status"]) => "Signed deployment\n".into(),
            ("journalctl", _) => "Sanitized fixture journal\n".into(),
            ("cat", ["/proc/sys/kernel/random/boot_id"]) => "fixture-boot\n".into(),
            ("cat", ["/etc/skillet/host"]) => format!("{}\n", self.profile),
            ("getenforce", []) => "Enforcing\n".into(),
            ("readlink", ["-f", "/etc/resolv.conf"]) => "/run/NetworkManager/resolv.conf\n".into(),
            ("readlink", ["-f", path]) if path.contains("Brewfile") => "/home/giacomo/.local/src/dotfiles/brew/Brewfile.core\n".into(),
            ("readlink", ["-f", path]) if path.contains("authorized_keys") => "/home/giacomo/.local/src/dotfiles/ssh/authorized_keys\n".into(),
            ("getent", ["hosts", "ghcr.io"]) => "192.0.2.1 ghcr.io\n".into(),
            ("systemctl", ["show", "-p", "Result", "--value", _]) => "success\n".into(),
            ("systemctl", ["show", "-p", "ExecMainStatus", "--value", _]) => "0\n".into(),
            ("systemctl", ["cat", "skillet-apply.service"]) => "ExecStart=/var/usrlocal/bin/skillet apply --host-file /etc/skillet/host --phase base\n".into(),
            ("systemctl", ["status", "--no-pager", "skillet-apply.service"]) => "Active: active (exited)\n".into(),
            ("cat", ["/etc/resolv.conf"]) => "nameserver 192.0.2.53\n".into(),
            ("systemctl", ["is-enabled", "fixture-masked.service"]) => return Ok(output(1, "masked\n")),
            ("true", []) | ("/home/linuxbrew/.linuxbrew/bin/brew", ["bundle", "check", "--file", _]) => String::new(),
            _ => return Err(Error::Invalid(format!("unsupported fake operation: {program} {args:?}"))),
        };
        Ok(output(0, stdout))
    }
    fn upload(&self, source: &Path, destination: &str) -> Result<()> {
        self.files
            .borrow_mut()
            .insert(destination.into(), sha256(source)?);
        self.calls.borrow_mut().push("upload".into());
        Ok(())
    }
}

fn fixture() -> (
    tempfile::TempDir,
    crate::ManifestStore,
    VmRun,
    ReadinessPolicy,
) {
    let (tmp, store, identity) = legacy_run();
    let dir = store.run_dir(&identity).join("source/files");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("skillet-fixture"), "captured host").unwrap();
    fs::write(dir.join("skillet"), "captured generic").unwrap();
    let mut run = store.import(&identity).unwrap();
    run.captured = Some(ArtifactHashes {
        host: sha256(&dir.join("skillet-fixture")).unwrap(),
        generic: sha256(&dir.join("skillet")).unwrap(),
    });
    store.save(&run).unwrap();
    let policy = ReadinessPolicy {
        signed_image: "registry/fixture".into(),
        resolver_target: "/run/NetworkManager/resolv.conf".into(),
        masked_units: vec!["fixture-masked.service".into()],
        phase_timeout: Duration::from_secs(20),
    };
    (tmp, store, run, policy)
}

#[test]
fn selected_profile_passes_signed_boot_delivery_and_user_environment_checks() {
    let (_tmp, store, mut run, policy) = fixture();
    let guest = FakeGuest::default();
    let dir = store.run_dir(&run.identity);
    let clock = FakeClock::default();
    let mut messages = Vec::new();
    ready(
        &mut run,
        &store,
        &dir,
        &policy,
        &ReadinessIo {
            probe: &guest,
            operations: &guest,
            ownership: &|| Ok(()),
        },
        &clock,
        &mut |message| messages.push(message.to_owned()),
    )
    .unwrap();
    assert_eq!(run.phase, Phase::Ready);
    assert_eq!(store.load(&run.identity).unwrap().phase, Phase::Ready);
    assert!(messages
        .iter()
        .any(|message| message.contains("fixture-test")));
    assert_eq!(
        fs::metadata(dir.join("readiness.log"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(dir.join("final-status.json").is_file());
    let calls = guest.calls.borrow();
    assert!(calls
        .iter()
        .any(|call| call.contains("brew") && call.contains("bundle")));
    assert!(!calls.iter().any(|call| call.contains("--wait")));
    drop(calls);
    guest.calls.borrow_mut().clear();
    ready(
        &mut run,
        &store,
        &dir,
        &policy,
        &ReadinessIo {
            probe: &guest,
            operations: &guest,
            ownership: &|| Ok(()),
        },
        &clock,
        &mut |_| {},
    )
    .unwrap();
    assert!(!guest.calls.borrow().iter().any(|call| call == "upload"));
}

#[test]
fn ssh_and_signed_boot_timeouts_are_bounded_and_leave_private_diagnostics() {
    for signed in [false, true] {
        let (_tmp, store, mut run, policy) = fixture();
        run.phase = Phase::Ready;
        store.save(&run).unwrap();
        let guest = FakeGuest {
            no_ssh: !signed,
            no_signed: signed,
            ..FakeGuest::default()
        };
        let clock = FakeClock::default();
        let dir = store.run_dir(&run.identity);
        assert!(ready(
            &mut run,
            &store,
            &dir,
            &policy,
            &ReadinessIo {
                probe: &guest,
                operations: &guest,
                ownership: &|| Ok(()),
            },
            &clock,
            &mut |_| {}
        )
        .is_err());
        assert_eq!(clock.elapsed(), policy.phase_timeout);
        assert!(dir.join("readiness.log").is_file());
        assert_eq!(store.load(&run.identity).unwrap().phase, Phase::Started);
    }
}

#[test]
fn bootstrap_apply_or_profile_failure_never_claims_ready() {
    for guest in [
        FakeGuest {
            no_signed: true,
            failed_bootstrap: true,
            ..FakeGuest::default()
        },
        FakeGuest {
            fail_unit: true,
            ..FakeGuest::default()
        },
        FakeGuest {
            profile: "unrelated",
            ..FakeGuest::default()
        },
    ] {
        let (_tmp, store, mut run, policy) = fixture();
        run.phase = Phase::Ready;
        store.save(&run).unwrap();
        let dir = store.run_dir(&run.identity);
        assert!(ready(
            &mut run,
            &store,
            &dir,
            &policy,
            &ReadinessIo {
                probe: &guest,
                operations: &guest,
                ownership: &|| Ok(()),
            },
            &FakeClock::default(),
            &mut |_| {}
        )
        .is_err());
        assert!(dir.join("readiness.log").is_file());
        assert_eq!(store.load(&run.identity).unwrap().phase, Phase::Started);
    }
}

#[test]
fn changed_capture_or_cleanup_phase_refuses_all_guest_effects() {
    for closing in [false, true] {
        let (_tmp, store, mut run, policy) = fixture();
        if closing {
            run.phase = Phase::DomainRemoved;
        } else {
            fs::write(
                store.run_dir(&run.identity).join("source/files/skillet"),
                "tampered",
            )
            .unwrap();
        }
        let guest = FakeGuest::default();
        let dir = store.run_dir(&run.identity);
        assert!(ready(
            &mut run,
            &store,
            &dir,
            &policy,
            &ReadinessIo {
                probe: &guest,
                operations: &guest,
                ownership: &|| Ok(()),
            },
            &FakeClock::default(),
            &mut |_| {}
        )
        .is_err());
        assert!(guest.calls.borrow().is_empty());
    }
}

#[test]
fn signed_origin_requires_one_booted_expected_signed_image_or_digest() {
    let expected = "registry/fixture";
    for origin in [
        "ostree-image-signed:docker://registry/fixture:latest".into(),
        format!(
            "ostree-image-signed:docker://registry/fixture@sha256:{}",
            "a".repeat(64)
        ),
    ] {
        let status =
            serde_json::json!({"deployments":[{"booted":true,"container-image-reference":origin}]})
                .to_string();
        verify_signed_origin(status.as_bytes(), expected).unwrap();
    }
    for status in [
        serde_json::json!({"deployments":[]}),
        serde_json::json!({"deployments":[{"booted":true,"container-image-reference":"ostree-unverified-registry:registry/fixture:latest"}]}),
        serde_json::json!({"deployments":[{"booted":true,"container-image-reference":"ostree-image-signed:docker://registry/other:latest"}]}),
        serde_json::json!({"deployments":[{"booted":true},{"booted":true}]}),
    ] {
        assert!(verify_signed_origin(status.to_string().as_bytes(), expected).is_err());
    }
}

#[test]
fn guest_security_service_links_brew_and_artifact_assertions_fail_closed() {
    for fault in [
        "selinux", "resolver", "result", "mask", "brew", "link", "artifact",
    ] {
        let (_tmp, store, mut run, policy) = fixture();
        let guest = FakeGuest {
            fault,
            ..FakeGuest::default()
        };
        let dir = store.run_dir(&run.identity);
        assert!(
            ready(
                &mut run,
                &store,
                &dir,
                &policy,
                &ReadinessIo {
                    probe: &guest,
                    operations: &guest,
                    ownership: &|| Ok(()),
                },
                &FakeClock::default(),
                &mut |_| {}
            )
            .is_err(),
            "{fault} must fail"
        );
        assert!(dir.join("readiness.log").is_file());
        assert_eq!(store.load(&run.identity).unwrap().phase, Phase::Started);
    }
}

#[test]
fn ownership_loss_after_wait_refuses_delivery_and_contact_for_diagnostics() {
    let (_tmp, store, mut run, policy) = fixture();
    let guest = FakeGuest::default();
    let checks = Cell::new(0);
    let ownership = || {
        checks.set(checks.get() + 1);
        if checks.get() <= 3 {
            Ok(())
        } else {
            Err(Error::Invalid("domain changed".into()))
        }
    };
    let dir = store.run_dir(&run.identity);
    assert!(ready(
        &mut run,
        &store,
        &dir,
        &policy,
        &ReadinessIo {
            probe: &guest,
            operations: &guest,
            ownership: &ownership,
        },
        &FakeClock::default(),
        &mut |_| {}
    )
    .is_err());
    assert_eq!(guest.calls.borrow().len(), 1); // Only the initial SSH probe.
    assert!(!dir.join("readiness.log").exists());
}

#[test]
fn ownership_loss_after_long_brew_operation_stops_before_dotfiles_start() {
    let (_tmp, store, mut run, policy) = fixture();
    let guest = FakeGuest::default();
    let ownership = || {
        let brew_started = guest.calls.borrow().iter().any(|call| {
            call == "sudo [\"-n\", \"systemctl\", \"start\", \"brew-install.service\"]"
        });
        if brew_started {
            Err(Error::Invalid(
                "owned domain changed during Brew install".into(),
            ))
        } else {
            Ok(())
        }
    };
    let dir = store.run_dir(&run.identity);
    assert!(ready(
        &mut run,
        &store,
        &dir,
        &policy,
        &ReadinessIo {
            probe: &guest,
            operations: &guest,
            ownership: &ownership,
        },
        &FakeClock::default(),
        &mut |_| {},
    )
    .is_err());
    assert!(guest.calls.borrow().iter().any(|call| {
        call == "sudo [\"-n\", \"systemctl\", \"start\", \"brew-install.service\"]"
    }));
    assert!(!guest.calls.borrow().iter().any(|call| {
        call == "sudo [\"-n\", \"systemctl\", \"start\", \"dotfiles-install.service\"]"
    }));
    assert_eq!(store.load(&run.identity).unwrap().phase, Phase::Started);
}
