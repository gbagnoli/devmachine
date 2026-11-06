use super::*;

// Decode the systemd word, then account for ExecStart specifier expansion.
fn delivered_definition(label: &str) -> Value {
    let quoted = label.strip_prefix("Label=").unwrap();
    let decoded: String = serde_json::from_str(quoted).unwrap();
    let expanded = decoded.replace("%%", "%");
    let definition = expanded.strip_prefix("com.datadoghq.ad.checks=").unwrap();
    serde_json::from_str(definition).unwrap()
}

#[test]
fn http_probe_preserves_autodiscovery_host_after_systemd_expansion() {
    let label = http("syncthing", "http://%%host%%:8384/rest/noauth/health");
    let definition = delivered_definition(&label);
    let instance = &definition["http_check"]["instances"][0];
    assert_eq!(instance["url"], "http://%%host%%:8384/rest/noauth/health");
    assert_eq!(instance["tags"], json!(["service:syncthing"]));
}

#[test]
fn process_probe_preserves_json_strings_and_has_no_exact_match() {
    let label = process("example", "command --flag=\"value\"");
    let definition = delivered_definition(&label);
    let instance = &definition["process"]["instances"][0];
    assert_eq!(
        instance["search_string"],
        json!(["command --flag=\"value\""])
    );
    assert_eq!(instance["exact_match"], false);
}
