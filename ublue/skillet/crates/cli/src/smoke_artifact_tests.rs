use super::artifact_target_profile;

#[test]
fn smoke_artifact_selection_preserves_target_and_profile() {
    let (target, profile) = artifact_target_profile(std::path::Path::new(
        "/tmp/cargo-target/aarch64-unknown-linux-musl/release/skillet",
    ))
    .unwrap();
    assert_eq!(target, "aarch64-unknown-linux-musl");
    assert_eq!(profile, "release");
    let (target, profile) = artifact_target_profile(std::path::Path::new(
        "/tmp/cargo-target/x86_64-unknown-linux-musl/debug/skillet",
    ))
    .unwrap();
    assert_eq!(target, "x86_64-unknown-linux-musl");
    assert_eq!(profile, "dev");
}
