use super::*;

fn artifact(name: &str, path: &str) -> String {
    serde_json::json!({ "reason": "compiler-artifact", "target": { "name": name, "kind": ["bin"] }, "executable": path }).to_string()
}

#[test]
fn selects_reported_custom_target_and_profile_without_scanning_stale_binaries() {
    let output = format!(
        "{}\n{}\n{{\"reason\":\"build-finished\",\"success\":true}}\n",
        artifact("skillet", "/custom/target/other-profile/skillet"),
        artifact(
            "skillet-fixture",
            "/custom/target/other-profile/skillet-fixture"
        )
    );
    let result = reported_artifacts(
        output.as_bytes(),
        &["skillet", "skillet-fixture"],
        Path::new("/workspace"),
    )
    .unwrap();
    assert_eq!(
        result["skillet"],
        Path::new("/custom/target/other-profile/skillet")
    );
    assert_eq!(
        result["skillet-fixture"],
        Path::new("/custom/target/other-profile/skillet-fixture")
    );
}

#[test]
fn missing_ambiguous_and_malformed_artifacts_fail() {
    assert!(reported_artifacts(
        b"{\"reason\":\"build-finished\"}\n",
        &["skillet"],
        Path::new("/workspace")
    )
    .is_err());
    let duplicate = format!(
        "{}\n{}",
        artifact("skillet", "/a/skillet"),
        artifact("skillet", "/b/skillet")
    );
    assert!(
        reported_artifacts(duplicate.as_bytes(), &["skillet"], Path::new("/workspace")).is_err()
    );
    assert!(reported_artifacts(b"not JSON", &["skillet"], Path::new("/workspace")).is_err());
}

#[test]
fn relative_artifacts_resolve_against_the_cargo_working_directory() {
    let result = reported_artifacts(
        artifact("skillet", "custom/debug/skillet").as_bytes(),
        &["skillet"],
        Path::new("/workspace"),
    )
    .unwrap();
    assert_eq!(
        result["skillet"],
        Path::new("/workspace/custom/debug/skillet")
    );
}
