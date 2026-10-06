use super::*;
use serde_json::json;
use std::cell::Cell;

fn catalog(default: &Value, hosts: &Value) -> String {
    json!({"version": 1, "services": {"example": {"default": default, "hosts": hosts}}}).to_string()
}

#[test]
fn embedded_ddns_template_resolves_each_environment_and_validates_consumer_schema() {
    for environment in ["prod", "test"] {
        let expected =
            format!("skillet/environments/{environment}/hosts/clamps/cloudflare/ddns-dns-name");
        let rendered = render_catalog(TEMPLATE_CATALOG, "ddns", "clamps", environment, &|path| {
            assert_eq!(path, expected);
            Ok(Some("edge".into()))
        })
        .unwrap();
        let config = skillet_ddns::config::PrivateConfig::parse(&rendered).unwrap();
        assert_eq!(config.records.len(), 1);
        assert_eq!(config.records[0].name, "edge");
        assert!(!config.records[0].proxied);
        assert!(!config.takeover_existing);
    }
}

#[test]
fn host_override_replaces_default_without_merging_or_reading_default_secrets() {
    let source = catalog(
        &json!({"default_only": {"$secret": "skillet/missing"}, "records": [1, 2]}),
        &json!({"clamps": {"records": [3]}}),
    );
    let rendered = render_catalog(&source, "example", "clamps", "test", &|_| {
        panic!("replaced defaults must not access the vault")
    })
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&rendered).unwrap(),
        json!({"records": [3]})
    );
}

#[test]
fn nested_secrets_are_escaped_strings_and_never_evaluated_recursively() {
    let secret = "\"\\\n{\"$secret\":\"skillet/other\"}";
    let source = catalog(
        &json!({"values": [{"nested": {"$secret": "skillet/{environment}/{host}/value"}}],
               "literal": "secret{skillet/value} {host}"}),
        &json!({}),
    );
    let reads = Cell::new(0);
    let rendered = render_catalog(&source, "example", "clamps", "test", &|path| {
        assert_eq!(path, "skillet/test/clamps/value");
        reads.set(reads.get() + 1);
        Ok(Some(secret.into()))
    })
    .unwrap();
    let result: Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(result["values"][0]["nested"], secret);
    assert_eq!(result["literal"], "secret{skillet/value} {host}");
    assert_eq!(reads.get(), 1);
}

#[test]
fn malformed_references_fail_before_lookup() {
    for marker in [
        json!({"$secret": 12}),
        json!({"$secret": "skillet/value", "extra": true}),
        json!({"$secret": "elsewhere/value"}),
        json!({"$secret": "skillet/{instance}/value"}),
        json!({"$secret": "skillet//value"}),
        json!({"$secret": "skillet/../value"}),
        json!({"$secret": "skillet/./value"}),
    ] {
        let source = catalog(&marker, &json!({}));
        assert!(matches!(
            render_catalog(&source, "example", "clamps", "test", &|_| {
                panic!("invalid references must not reach the vault")
            }),
            Err(ConfigurationTemplateError::InvalidReference)
        ));
    }
}

#[test]
fn invalid_context_and_catalog_fail_before_lookup() {
    for (host, environment) in [("../clamps", "test"), ("clamps", ""), ("clamps", "test/x")] {
        assert!(matches!(
            render_catalog(TEMPLATE_CATALOG, "ddns", host, environment, &|_| {
                panic!("invalid context must not reach the vault")
            }),
            Err(ConfigurationTemplateError::InvalidContext)
        ));
    }
    assert!(matches!(
        render_catalog(
            "{\"version\":2,\"services\":{}}",
            "ddns",
            "clamps",
            "test",
            &|_| Ok(None)
        ),
        Err(ConfigurationTemplateError::Version)
    ));
    assert!(matches!(
        render_catalog(TEMPLATE_CATALOG, "unknown", "clamps", "test", &|_| Ok(None)),
        Err(ConfigurationTemplateError::MissingTemplate { .. })
    ));
}

#[test]
fn missing_entries_fail_and_invalid_secret_values_still_require_consumer_validation() {
    let source = catalog(&json!({"$secret": "skillet/missing"}), &json!({}));
    assert!(matches!(
        render_catalog(&source, "example", "clamps", "test", &|_| Ok(None)),
        Err(ConfigurationTemplateError::MissingSecret(path)) if path == "skillet/missing"
    ));
    let secret = "private.invalid/record";
    let rendered = render_catalog(TEMPLATE_CATALOG, "ddns", "clamps", "test", &|_| {
        Ok(Some(secret.into()))
    })
    .unwrap();
    let error = skillet_ddns::config::PrivateConfig::parse(&rendered)
        .err()
        .unwrap();
    assert!(!error.to_string().contains(secret));
}
