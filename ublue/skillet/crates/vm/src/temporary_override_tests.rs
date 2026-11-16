use super::*;
use std::{
    cell::{Cell, RefCell},
    os::unix::process::ExitStatusExt as _,
    path::Path,
};
struct Guest {
    missing: bool,
    calls: RefCell<Vec<Vec<String>>>,
}
impl GuestTransport for Guest {
    fn execute(&self, command: &GuestCommand<'_>, _: Option<&[u8]>) -> Result<Output> {
        self.calls
            .borrow_mut()
            .push(command.arguments.iter().map(|arg| (*arg).into()).collect());
        let stdout = if command.arguments.contains(&"--property=LoadState") {
            if self.missing {
                b"not-found\n".to_vec()
            } else {
                b"loaded\n".to_vec()
            }
        } else {
            Vec::new()
        };
        Ok(Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout,
            stderr: Vec::new(),
        })
    }
    fn upload(&self, _: &Path, _: &str) -> Result<()> {
        unreachable!()
    }
}
#[test]
fn only_explicitly_missing_dependency_can_defer_effective_command_verification() {
    for missing in [false, true] {
        for deferred in [false, true] {
            let guest = Guest {
                missing,
                calls: RefCell::default(),
            };
            let called = Cell::new(false);
            let operation = || {
                called.set(true);
                Ok(())
            };
            let result = with_exec_start_impl(
                &guest,
                "fixture.service",
                "0123456789abcdef0123456789abcdef",
                "/var/usrlocal/bin/.skillet-remote-apply-0123456789abcdef0123456789abcdef",
                &["media", "reconcile"],
                deferred,
                operation,
            );
            assert_eq!(result.is_ok(), missing && deferred);
            assert_eq!(called.get(), missing && deferred);
            assert!(guest.calls.borrow().iter().any(|args| args.starts_with(&[
                "-n".into(),
                "rm".into(),
                "-f".into()
            ])));
        }
    }
}
