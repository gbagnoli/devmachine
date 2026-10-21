use super::*;
use std::{cell::Cell, os::unix::process::ExitStatusExt as _, process::Output};
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
fn mismatch_prevents_build_and_delivery() {
    let guest = Guest {
        identity: "joey\n",
        calls: Cell::new(0),
        code: 0,
    };
    assert!(
        deploy("clamps", Path::new("/nonexistent-workspace"), &guest)
            .unwrap_err()
            .to_string()
            .contains("identity differs")
    );
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
