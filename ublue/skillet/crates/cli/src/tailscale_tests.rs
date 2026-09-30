use super::{curl_quote, device_record, form_encode, SMOKE_TAG};
use serde_json::json;

#[test]
fn form_encoding_escapes_oauth_credentials() {
    assert_eq!(form_encode("a b+c/%"), "a%20b%2Bc%2F%25");
}

#[test]
fn curl_config_quotes_newlines_and_quotes() {
    assert_eq!(curl_quote("one\ntwo\"\\"), "\"one\\ntwo\\\"\\\\\"");
}

#[test]
fn device_record_requires_expected_tag_and_identity_fields() {
    let value = json!({
        "id": "node-id",
        "hostname": "clamps-test-smoke",
        "addresses": ["100.64.0.5", "fd7a:115c:a1e0::5"],
        "tags": [SMOKE_TAG]
    });
    let record = device_record(&value, SMOKE_TAG).expect("valid tagged device");
    assert_eq!(record.id, "node-id");
    assert_eq!(record.hostname, "clamps-test-smoke");
    assert_eq!(record.addresses.len(), 2);
    assert!(device_record(&value, "tag:skillet-server").is_none());
}
