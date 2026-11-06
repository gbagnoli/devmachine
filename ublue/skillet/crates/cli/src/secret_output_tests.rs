use super::*;

#[test]
fn check_output_lists_entry_names_without_values() {
    let report = CheckReport {
        checked: 1,
        present: vec!["skillet/environments/prod/hosts/donbot/airtrail/aerodatabox-api-key".into()],
        missing: Vec::new(),
    };
    let rendered = render(&report, &[], false);
    assert!(rendered.contains("Present entries (names only)"));
    assert!(
        rendered.contains("skillet/environments/prod/hosts/donbot/airtrail/aerodatabox-api-key")
    );
    assert!(!rendered.contains("provider-key-value"));
}
