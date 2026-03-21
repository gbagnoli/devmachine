use super::*;
use skillet_core::test_utils::{MockFiles, MockSystem};

#[test]
fn test_hardening_applies_sysctl() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    apply(&system, &files).unwrap();
    assert!(files
        .files
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .contains_key("/etc/sysctl.d/99-hardening.conf"));
}
