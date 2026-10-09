use super::*;
use std::cell::{Cell, RefCell};

struct Fake {
    state: RefCell<Option<String>>,
    fingerprint: RefCell<String>,
    expired: Cell<bool>,
    expirations: Cell<usize>,
    policies: Cell<usize>,
    fail_expiry: Cell<bool>,
}
impl Default for Fake {
    fn default() -> Self {
        Self {
            state: RefCell::new(None),
            fingerprint: RefCell::new("a".repeat(64)),
            expired: Cell::new(false),
            expirations: Cell::new(0),
            policies: Cell::new(0),
            fail_expiry: Cell::new(false),
        }
    }
}
impl AccessRuntime for Fake {
    fn password(&self, _: &str) -> Result<PasswordState, CliCommonError> {
        Ok(PasswordState {
            fingerprint: self.fingerprint.borrow().clone(),
            expired: self.expired.get(),
        })
    }
    fn read_state(&self, _: &str) -> Result<Option<String>, CliCommonError> {
        Ok(self.state.borrow().clone())
    }
    fn write_state(&self, _: &str, state: &str) -> Result<(), CliCommonError> {
        *self.state.borrow_mut() = Some(state.to_string());
        Ok(())
    }
    fn require_sudo_password(&self, _: &str) -> Result<(), CliCommonError> {
        self.policies.set(self.policies.get() + 1);
        Ok(())
    }
    fn expire_password(&self, _: &str) -> Result<(), CliCommonError> {
        assert!(self
            .state
            .borrow()
            .as_deref()
            .is_some_and(|s| s.starts_with("pending:")));
        assert!(self.policies.get() > 0);
        if self.fail_expiry.get() {
            return Err(config("expiry failed"));
        }
        self.expirations.set(self.expirations.get() + 1);
        self.expired.set(true);
        Ok(())
    }
}
#[test]
fn finalization_requires_password_and_expires_once() {
    let runtime = Fake::default();
    finalize(&runtime, "admin").unwrap();
    runtime.expired.set(false);
    *runtime.fingerprint.borrow_mut() = "b".repeat(64);
    finalize(&runtime, "admin").unwrap();
    assert_eq!(runtime.expirations.get(), 1);
    assert_eq!(runtime.policies.get(), 2);
    assert_eq!(runtime.state.borrow().as_deref(), Some("complete\n"));
}
#[test]
fn interrupted_expiry_is_retryable() {
    let runtime = Fake::default();
    runtime.fail_expiry.set(true);
    assert!(finalize(&runtime, "admin").is_err());
    runtime.fail_expiry.set(false);
    finalize(&runtime, "admin").unwrap();
    assert_eq!(runtime.expirations.get(), 1);
}
#[test]
fn recovery_preserves_password_changed_after_interruption() {
    for changed in [false, true] {
        let runtime = Fake::default();
        *runtime.state.borrow_mut() = Some(format!("pending:{}\n", "a".repeat(64)));
        if changed {
            *runtime.fingerprint.borrow_mut() = "b".repeat(64);
        } else {
            runtime.expired.set(true);
        }
        finalize(&runtime, "admin").unwrap();
        assert_eq!(runtime.expirations.get(), 0);
    }
}
#[test]
fn account_names_cannot_inject_policy_or_options() {
    for user in ["", "root", "-admin", "admin\nroot", "admin ALL", "../admin"] {
        assert!(validate_user(user).is_err());
    }
    assert!(validate_user("admin-2").is_ok());
    assert!(sudo_policy("admin").contains("PASSWD: ALL"));
    assert!(!sudo_policy("admin").contains("NOPASSWD"));
}
#[test]
fn shared_host_cli_accepts_finalization() {
    use clap::Parser as _;
    assert!(
        crate::HostArgs::try_parse_from(["host", "access", "finalize", "--user", "admin"]).is_ok()
    );
}

#[test]
fn malformed_journal_does_not_change_policy_or_account() {
    let runtime = Fake::default();
    *runtime.state.borrow_mut() = Some("invalid".to_string());
    assert!(finalize(&runtime, "admin").is_err());
    assert_eq!(runtime.policies.get(), 0);
    assert_eq!(runtime.expirations.get(), 0);
}
