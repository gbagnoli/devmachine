use super::*;

#[test]
fn template_availability_requires_both_sources_and_is_sorted() {
    let dir = tempfile::tempdir().unwrap();
    let butane = dir.path().join("butane");
    let workspace = dir.path().join("workspace");
    fs::create_dir(&butane).unwrap();
    fs::create_dir_all(workspace.join("crates/hosts/complete")).unwrap();
    fs::create_dir_all(workspace.join("crates/hosts/crate-only")).unwrap();
    fs::write(butane.join("complete.bu"), "fixture").unwrap();
    fs::write(butane.join("template-only.bu"), "fixture").unwrap();
    fs::write(butane.join("Invalid.bu"), "fixture").unwrap();
    fs::write(
        workspace.join("crates/hosts/complete/Cargo.toml"),
        "fixture",
    )
    .unwrap();
    fs::write(
        workspace.join("crates/hosts/crate-only/Cargo.toml"),
        "fixture",
    )
    .unwrap();
    assert_eq!(
        templates(&butane, &workspace).unwrap(),
        BTreeMap::from([
            ("complete".into(), true),
            ("crate-only".into(), false),
            ("template-only".into(), false)
        ])
    );
}

#[test]
fn runs_are_discovered_without_reading_or_rewriting_partial_manifests() {
    let dir = tempfile::tempdir().unwrap();
    let store = ManifestStore::new(dir.path(), users::get_current_uid()).unwrap();
    for name in [
        "fixture-test-2",
        "fixture-test-a",
        "other-test-a",
        "fixture-test-Invalid",
    ] {
        fs::create_dir(dir.path().join(name)).unwrap();
    }
    let runs = recorded_runs(&store, "fixture").unwrap();
    assert_eq!(
        runs.iter().map(RunIdentity::instance).collect::<Vec<_>>(),
        ["2", "a"]
    );
    assert!(!store.run_dir(&runs[0]).join("vm.json").exists());
    assert!(recorded_runs(&store, "../escape").is_err());
}
