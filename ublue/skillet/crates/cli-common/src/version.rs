//! Build identity shared by workstation and guest entry points.
use std::sync::OnceLock;

/// Cargo release version plus source revision and working-tree state.
pub fn version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| {
        format_version(
            env!("CARGO_PKG_VERSION"),
            option_env!("VERGEN_GIT_SHA"),
            option_env!("VERGEN_GIT_DIRTY"),
        )
    })
}

fn format_version(release: &str, sha: Option<&str>, dirty: Option<&str>) -> String {
    let revision = sha
        .filter(|value| value.len() >= 12 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .and_then(|value| value.get(..12));
    let Some(revision) = revision else {
        return format!("{release}+git.unknown");
    };
    let suffix = match dirty {
        Some("true") => ".dirty",
        Some("false") => "",
        _ => ".state-unknown",
    };
    format!("{release}+git.{revision}{suffix}")
}

#[cfg(test)]
#[path = "version_tests.rs"]
mod tests;
