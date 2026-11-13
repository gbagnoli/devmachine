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

#[test]
fn version_output_is_bounded_and_terminal_safe() {
    assert_eq!(
        super::version_text(b"skillet 0.1.0+git.abcdef\n").unwrap(),
        "skillet 0.1.0+git.abcdef"
    );
    for bytes in [b"".as_slice(), b"bad\nsecond", b"\x1b[31mred", b"\xff"] {
        assert!(super::version_text(bytes).is_err());
    }
    assert!(super::version_text(&[b'a'; 201]).is_err());
}

#[test]
fn nested_cargo_removes_parent_metadata_and_preserves_build_configuration() {
    let metadata = [
        "CARGO_PKG_NAME",
        "CARGO_PKG_VERSION",
        "CARGO_PKG_VERSION_PATCH",
        "CARGO_MANIFEST_DIR",
        "CARGO_MANIFEST_PATH",
        "CARGO_MANIFEST_LINKS",
        "CARGO_BIN_EXE_skillet",
        "CARGO_FEATURE_TEST",
        "CARGO_CFG_TARGET_ARCH",
        "CARGO_CRATE_NAME",
        "CARGO_BIN_NAME",
    ];
    let configuration = [
        "CARGO_TARGET_DIR",
        "CARGO_HOME",
        "CARGO_BUILD_TARGET",
        "CARGO_BUILD_JOBS",
        "CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_PROFILE_RELEASE_LTO",
        "CARGO_REGISTRIES_CRATES_IO_PROTOCOL",
        "RUSTUP_TOOLCHAIN",
        "RUSTFLAGS",
        "CC",
        "PATH",
    ];
    let mut command = Command::new("cargo");
    for key in metadata.iter().chain(&configuration) {
        command.env(key, "sentinel");
    }
    remove_parent_cargo_metadata(&mut command, metadata.iter().chain(&configuration));
    let overrides: BTreeMap<_, _> = command.get_envs().collect();
    for key in metadata {
        assert_eq!(overrides[OsStr::new(key)], None, "retained {key}");
    }
    for key in configuration {
        assert_eq!(
            overrides[OsStr::new(key)],
            Some(OsStr::new("sentinel")),
            "removed {key}"
        );
    }
}

#[test]
fn repeated_nested_builds_keep_cargo_artifacts_fresh() {
    let fixture = tempfile::tempdir().unwrap();
    std::fs::write(
        fixture.path().join("Cargo.toml"),
        "[package]\nname = \"cache-probe\"\nversion = \"0.0.0\"\nedition = \"2021\"\n[workspace]\n[lib]\npath = \"lib.rs\"\n",
    )
    .unwrap();
    std::fs::write(fixture.path().join("lib.rs"), "pub const VALUE: u8 = 1;\n").unwrap();
    std::fs::write(
        fixture.path().join("build.rs"),
        "fn main() { println!(\"cargo:rerun-if-env-changed=CARGO_PKG_NAME\"); println!(\"cargo:rerun-if-env-changed=CARGO_MANIFEST_DIR\"); }\n",
    )
    .unwrap();
    for (index, parent) in [Some("workstation"), Some("guest"), None]
        .into_iter()
        .enumerate()
    {
        let mut command = Command::new("cargo");
        command.current_dir(fixture.path()).args([
            "build",
            "--offline",
            "--message-format=json-render-diagnostics",
        ]);
        command.env("CARGO_TARGET_DIR", fixture.path().join("target"));
        if let Some(parent) = parent {
            command.env("CARGO_PKG_NAME", parent);
            command.env("CARGO_MANIFEST_DIR", format!("/outer/{parent}"));
        }
        remove_parent_cargo_metadata(
            &mut command,
            std::env::vars_os()
                .map(|(key, _)| key)
                .chain(["CARGO_PKG_NAME".into(), "CARGO_MANIFEST_DIR".into()]),
        );
        let output = crate::process::capture(command, std::time::Duration::from_secs(60)).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let artifacts: Vec<serde_json::Value> = output
            .stdout
            .split(|byte| *byte == b'\n')
            .filter_map(|line| serde_json::from_slice::<serde_json::Value>(line).ok())
            .filter(|message| message["reason"] == "compiler-artifact")
            .collect();
        assert_eq!(
            artifacts.len(),
            2,
            "expected library and build-script artifacts"
        );
        if index > 0 {
            assert!(
                artifacts.iter().all(|message| message["fresh"] == true),
                "{artifacts:?}"
            );
        }
    }
}
