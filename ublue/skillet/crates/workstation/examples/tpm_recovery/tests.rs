use super::*;
use std::{os::unix::process::ExitStatusExt as _, process::Output};
struct Guest {
    blocked: &'static str,
}
impl GuestTransport for Guest {
    fn upload(&self, _: &Path, _: &str) -> skillet_vm::Result<()> {
        panic!("preflight must not upload")
    }
    fn execute(
        &self,
        command: &GuestCommand<'_>,
        input: Option<&[u8]>,
    ) -> skillet_vm::Result<Output> {
        assert!(input.is_none());
        let blocked = !self.blocked.is_empty() && command.arguments.contains(&self.blocked);
        Ok(Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: if blocked {
                b"existing resource\n".to_vec()
            } else {
                vec![]
            },
            stderr: vec![],
        })
    }
}
#[test]
fn base_only_preflight_rejects_containers_and_delivered_credentials() {
    require_base_only(&Guest { blocked: "" }).unwrap();
    for blocked in ["ps", "secret", "find"] {
        assert!(require_base_only(&Guest { blocked }).is_err());
    }
}
#[test]
fn readiness_evidence_is_not_provider_ownership() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("vm.json"), "manifest").unwrap();
    fs::write(dir.path().join("final-status.json"), "readiness").unwrap();
    refuse_provider_journals(dir.path()).unwrap();
    fs::write(dir.path().join("cloudflare.json"), "ownership").unwrap();
    assert!(refuse_provider_journals(dir.path()).is_err());
}
