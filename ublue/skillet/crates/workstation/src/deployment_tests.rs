use super::*;
use std::{cell::Cell, os::unix::process::ExitStatusExt as _, process::Output};
struct Guest {
    identity: &'static str,
    calls: Cell<usize>,
}
impl GuestTransport for Guest {
    fn execute(&self, command: &GuestCommand<'_>, _: Option<&[u8]>) -> Result<Output> {
        assert_eq!(command.program, "cat");
        assert_eq!(command.arguments, ["/etc/skillet/host"]);
        self.calls.set(self.calls.get() + 1);
        Ok(Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: self.identity.as_bytes().to_vec(),
            stderr: Vec::new(),
        })
    }
    fn upload(&self, _: &Path, _: &str) -> Result<()> {
        panic!("identity mismatch must not upload")
    }
}
#[test]
fn mismatch_prevents_build_and_delivery() {
    let guest = Guest {
        identity: "joey\n",
        calls: Cell::new(0),
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
    };
    assert!(verify_host("unknown", &guest).is_err());
    assert_eq!(guest.calls.get(), 0);
}

#[test]
fn version_inspection_rejects_unknown_profile_without_contact() {
    let guest = Guest {
        identity: "clamps\n",
        calls: Cell::new(0),
    };
    assert!(installed_versions("../unknown", &guest).is_err());
    assert_eq!(guest.calls.get(), 0);
}
