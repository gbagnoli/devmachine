use super::*;
use std::{cell::Cell, os::unix::process::ExitStatusExt as _, path::Path, process::Output};
struct Guest {
    identity: &'static str,
    calls: Cell<usize>,
    code: i32,
}
impl GuestTransport for Guest {
    fn execute(&self, command: &GuestCommand<'_>, _: Option<&[u8]>) -> skillet_vm::Result<Output> {
        assert_eq!(command.program, "cat");
        assert_eq!(command.arguments, ["/etc/skillet/host"]);
        self.calls.set(self.calls.get() + 1);
        Ok(Output {
            status: std::process::ExitStatus::from_raw(self.code << 8),
            stdout: self.identity.as_bytes().to_vec(),
            stderr: b"Permission denied (publickey).\n".to_vec(),
        })
    }
    fn upload(&self, _: &Path, _: &str) -> skillet_vm::Result<()> {
        panic!("identity mismatch must not upload")
    }
}
#[test]
fn mismatch_prevents_credential_delivery() {
    let guest = Guest {
        identity: "joey\n",
        calls: Cell::new(0),
        code: 0,
    };
    assert!(verify_host("clamps", &guest)
        .unwrap_err()
        .to_string()
        .contains("identity differs"));
    assert_eq!(guest.calls.get(), 1);
}
#[test]
fn unknown_profile_prevents_guest_contact() {
    let guest = Guest {
        identity: "clamps\n",
        calls: Cell::new(0),
        code: 0,
    };
    assert!(verify_host("unknown", &guest).is_err());
    assert_eq!(guest.calls.get(), 0);
}

#[test]
fn version_inspection_rejects_unknown_profile_without_contact() {
    let guest = Guest {
        identity: "clamps\n",
        calls: Cell::new(0),
        code: 0,
    };
    assert!(installed_versions("../unknown", &guest).is_err());
    assert_eq!(guest.calls.get(), 0);
}

#[test]
fn ssh_failure_is_not_a_host_identity_mismatch() {
    let guest = Guest {
        identity: "",
        calls: Cell::new(0),
        code: 255,
    };
    let error = verify_host("clamps", &guest).unwrap_err();
    assert!(matches!(error, DeploymentError::Connection { .. }));
    let message = error.to_string();
    assert!(message.contains("publickey"));
    assert!(message.contains("ssh-add"));
    assert!(!message.contains("identity differs"));
    assert!(!message.contains("VM state"));
    let guest = Guest {
        identity: "",
        calls: Cell::new(0),
        code: 1,
    };
    assert!(matches!(
        verify_host("clamps", &guest),
        Err(DeploymentError::IdentityRead { .. })
    ));
}

#[test]
fn agent_setup_hints_cover_missing_empty_and_advertised_sockets_without_mutating_environment() {
    use std::ffi::OsStr;
    for socket in [None, Some(OsStr::new(""))] {
        let hint = ssh_agent_hint(socket);
        assert!(hint.contains("SSH_AUTH_SOCK is unset or empty"));
        assert!(hint.contains("ssh-agent -s"));
        assert!(hint.contains("ssh-add /path/to/your/key"));
        assert!(hint.contains("same shell"));
    }
    let hint = ssh_agent_hint(Some(OsStr::new("/tmp/example-agent.sock")));
    assert!(hint.contains("SSH_AUTH_SOCK is set"));
    assert!(hint.contains("ssh-add -l"));
    assert!(!hint.contains("eval"));
    assert!(!hint.contains("example-agent.sock"));
}

type ActivationCall = (Vec<String>, Option<Vec<u8>>);
#[derive(Default)]
struct ActivationGuest {
    calls: std::cell::RefCell<Vec<ActivationCall>>,
    effective: std::cell::RefCell<String>,
    fail_start: bool,
}
impl GuestTransport for ActivationGuest {
    fn execute(
        &self,
        command: &GuestCommand<'_>,
        input: Option<&[u8]>,
    ) -> skillet_vm::Result<Output> {
        self.calls.borrow_mut().push((
            std::iter::once(command.program.to_string())
                .chain(command.arguments.iter().map(|arg| (*arg).to_string()))
                .collect(),
            input.map(<[u8]>::to_vec),
        ));
        let mut stdout = if command.program == "systemctl" {
            b"LoadState=loaded\nConditionResult=yes\nResult=success\nExecMainStatus=0\nExecMainStartTimestampMonotonic=1\n".to_vec()
        } else if command.arguments.contains(&"--show-cursor") {
            b"-- cursor: fixture\n".to_vec()
        } else {
            Vec::new()
        };
        if matches!(command.arguments, ["-n", "tee", _]) {
            if let Some(input) = input {
                let text = std::str::from_utf8(input).unwrap();
                *self.effective.borrow_mut() = text
                    .lines()
                    .filter_map(|line| line.strip_prefix("ExecStart="))
                    .filter(|line| !line.is_empty())
                    .collect::<String>();
            }
        } else if matches!(
            command.arguments,
            [
                "-n",
                "systemctl",
                "show",
                "--property=ExecStart",
                "--value",
                _
            ]
        ) {
            stdout = self.effective.borrow().as_bytes().to_vec();
        }
        let failed =
            self.fail_start && matches!(command.arguments, ["-n", "systemctl", "start", _]);
        Ok(Output {
            status: std::process::ExitStatus::from_raw(i32::from(failed) << 8),
            stdout,
            stderr: Vec::new(),
        })
    }
    fn upload(&self, _: &Path, _: &str) -> skillet_vm::Result<()> {
        Ok(())
    }
}

fn runner() -> TemporaryRunner {
    let token = "0123456789abcdef0123456789abcdef".to_string();
    TemporaryRunner {
        host: "donbot".into(),
        path: format!("/var/usrlocal/bin/.skillet-remote-apply-{token}"),
        upload: "/var/usrlocal/bin/fixture-upload".into(),
        token,
        generic_version: "fixture".into(),
    }
}

#[test]
fn current_runner_activation_overrides_only_declared_units_and_cleans_up_failures() {
    for fail_start in [false, true] {
        let guest = ActivationGuest {
            fail_start,
            ..ActivationGuest::default()
        };
        let runner = runner();
        let current = crate::apply_runner_transport::ApplyRunnerTransport::new(&guest, &runner);
        let result = current.execute(
            &GuestCommand {
                program: "/usr/bin/sudo",
                arguments: &["-n", "systemctl", "start", "skillet-caddy-apply.service"],
            },
            None,
        );
        assert_eq!(result.is_err(), fail_start);
        assert!(guest.effective.borrow().contains(&format!(
            "{} apply --host-file /etc/skillet/host --phase caddy",
            runner.path()
        )));
        let calls = guest.calls.borrow();
        assert!(calls.iter().any(|(call, _)| call
            .iter()
            .any(|arg| arg.starts_with("/run/systemd/system/skillet-caddy-apply.service.d/zzzz-"))
            && call.contains(&"rm".into())));
        assert!(!calls.iter().any(|(call, _)| call
            .iter()
            .any(|arg| arg.starts_with("/etc/systemd/system/"))));
    }
}

#[test]
fn current_runner_preserves_payload_stdin_and_does_not_wrap_application_services() {
    let guest = ActivationGuest::default();
    let runner = runner();
    let current = crate::apply_runner_transport::ApplyRunnerTransport::new(&guest, &runner);
    let command = GuestCommand {
        program: "/usr/bin/sudo",
        arguments: &["-n", "tee", "/run/fixture-secret"],
    };
    current
        .execute(&command, Some(b"protected-payload"))
        .unwrap();
    assert_eq!(guest.calls.borrow().len(), 1);
    assert_eq!(
        guest.calls.borrow()[0].1.as_deref(),
        Some(b"protected-payload".as_slice())
    );
    current
        .execute(
            &GuestCommand {
                program: "sudo",
                arguments: &["-n", "systemctl", "start", "jellyfin.service"],
            },
            None,
        )
        .unwrap();
    assert_eq!(guest.calls.borrow().len(), 2);
    let count = guest.calls.borrow().len();
    assert!(current
        .execute(
            &GuestCommand {
                program: "sudo",
                arguments: &["-n", "systemctl", "start", "skillet-full-apply.service"]
            },
            Some(b"unexpected")
        )
        .is_err());
    assert_eq!(guest.calls.borrow().len(), count);
}

#[test]
fn current_runner_stages_implicit_consumers_without_changing_payload_bytes() {
    let guest = ActivationGuest::default();
    let runner = runner();
    let current = crate::apply_runner_transport::ApplyRunnerTransport::new(&guest, &runner);
    current
        .execute(
            &GuestCommand {
                program: "/usr/bin/sudo",
                arguments: &[
                    "-n",
                    "/var/usrlocal/bin/skillet-donbot",
                    "credential",
                    "install",
                    "tailscale_auth_key",
                    "skillet-full-apply.service",
                ],
            },
            Some(b"private-bytes"),
        )
        .unwrap();
    let calls = guest.calls.borrow();
    assert_eq!(calls[0].1.as_deref(), Some(b"private-bytes".as_slice()));
    assert_eq!(calls[0].0.last().unwrap(), "--no-start");
    assert!(guest.effective.borrow().contains("--phase full"));
    assert!(calls.iter().any(|(call, _)| call.contains(&"start".into())));
}

#[test]
fn personal_syncthing_temporary_command_keeps_its_instance_selector() {
    let args =
        crate::host_apply::temporary_arguments("skillet-syncthing-personal-initialize.service")
            .unwrap();
    assert!(args
        .windows(2)
        .any(|pair| pair == ["--instance", "personal"]));
}

#[test]
fn current_runner_wraps_native_initializer_dependencies_and_cleans_both_units() {
    for fail_start in [false, true] {
        let guest = ActivationGuest {
            fail_start,
            ..ActivationGuest::default()
        };
        let runner = runner();
        let result = activate_with_runner(
            &guest,
            "skillet-gokapi-apply.service",
            runner.path(),
            runner.token(),
            crate::host_apply::temporary_arguments("skillet-gokapi-apply.service").unwrap(),
        );
        assert_eq!(result.is_err(), fail_start);
        let calls = guest.calls.borrow();
        for unit in [
            "skillet-apply.service",
            "skillet-gokapi-initialize.service",
            "skillet-gokapi-apply.service",
        ] {
            let path = format!(
                "/run/systemd/system/{unit}.d/zzzz-skillet-remote-apply-{}.conf",
                runner.token()
            );
            assert!(calls
                .iter()
                .any(|(args, input)| args[1..] == ["-n", "tee", &path] && input.is_some()));
            assert!(calls
                .iter()
                .any(|(args, _)| args[1..] == ["-n", "rm", "-f", &path]));
        }
        let initializer = calls
            .iter()
            .position(|(args, _)| {
                args.iter()
                    .any(|v| v.contains("skillet-gokapi-initialize.service.d/"))
            })
            .unwrap();
        let start = calls
            .iter()
            .position(|(args, _)| {
                args[1..] == ["-n", "systemctl", "start", "skillet-gokapi-apply.service"]
            })
            .unwrap();
        assert!(initializer < start);
    }
}
