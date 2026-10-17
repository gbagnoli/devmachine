use super::*;
#[test]
fn missing_or_stale_backup_is_not_reported_as_zero_age() {
    assert_eq!(render(100, false, None).len(), 1);
    let missing = render(100, true, None);
    assert!(missing
        .iter()
        .any(|s| s.starts_with("_sc|homeassistant.backup.fresh|2")));
    assert_eq!(missing.len(), 2);
    let s = State {
        completed: 1,
        exported: 1,
    };
    assert!(render(200_000, true, Some(&s))
        .iter()
        .any(|s| s.starts_with("_sc|homeassistant.backup.fresh|2")));
    assert!(render(100, true, Some(&s))
        .iter()
        .any(|s| s.starts_with("_sc|homeassistant.backup.fresh|0")));
}
