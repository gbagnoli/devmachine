//! Service-owned, credential-free Datadog Autodiscovery declarations.
//!
//! Agent activation is separate from ordinary application convergence.
mod runtime;
pub use runtime::{apply, DatadogError, Input, RuntimeConfig, CREDENTIAL, IMAGE};
use serde_json::{json, Value};

fn label(integration: &str, instance: &Value) -> String {
    let definition = json!({integration: {"init_config": {}, "instances": [instance]}}).to_string();
    // Quote the complete value for Quadlet's systemd word parser. Quadlet
    // preserves specifiers; the generated service then expands %% to %.
    let escaped = definition
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    format!("Label=\"com.datadoghq.ad.checks={escaped}\"")
}

/// An unauthenticated internal HTTP probe. Values must be public configuration;
/// credentials must never be supplied through container labels.
pub fn http(name: &str, url: &str) -> String {
    label(
        "http_check",
        &json!({"name": name, "url": url, "timeout": 5,
        "tags": [format!("service:{name}")]}),
    )
}

/// Host-PID process liveness, distinct from application correctness.
pub fn process(name: &str, search: &str) -> String {
    label(
        "process",
        &json!({"name": name, "search_string": [search], "exact_match": false,
        "tags": [format!("service:{name}")]}),
    )
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
