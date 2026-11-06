use super::*;
use clap::CommandFactory as _;
const SHA: &str = "1234567890abcdef1234567890abcdef12345678";

#[test]
fn formats_clean_dirty_and_unknown_source() {
    assert_eq!(
        format_version("0.1.0", Some(SHA), Some("false")),
        "0.1.0+git.1234567890ab"
    );
    assert_eq!(
        format_version("0.1.0", Some(SHA), Some("true")),
        "0.1.0+git.1234567890ab.dirty"
    );
    for sha in [None, Some("VERGEN_IDEMPOTENT_OUTPUT"), Some("bad")] {
        assert_eq!(format_version("0.1.0", sha, None), "0.1.0+git.unknown");
    }
    assert_eq!(
        format_version("0.1.0", Some(SHA), None),
        "0.1.0+git.1234567890ab.state-unknown"
    );
}

#[test]
fn host_cli_uses_shared_version() {
    assert_eq!(crate::HostArgs::command().get_version(), Some(version()));
}

#[test]
fn host_version_displays_caller_identity() {
    assert_eq!(
        crate::host_command("example").render_version(),
        format!("skillet-example {}\n", version())
    );
}
