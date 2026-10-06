use super::*;

const ZONE: &str = "0123456789abcdef0123456789abcdef";

#[test]
fn renders_updater_schema_and_matches_chef_policy() {
    let config =
        PrivateConfig::parse(r#"{"version":1,"records":[{"name":"edge","proxied":true}]}"#)
            .unwrap();
    config
        .validate_zone("example.com", &["syncthing.host.ui.example.com".into()])
        .unwrap();
    let rendered = config
        .payload(ZONE, "dummy-token")
        .unwrap()
        .render()
        .unwrap();
    let json: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(json["a"], true);
    assert_eq!(json["aaaa"], false);
    assert_eq!(json["ttl"], 300);
    assert_eq!(json["purgeUnknownRecords"], false);
    assert_eq!(json["cloudflare"][0]["subdomains"][0]["name"], "edge");
    assert_eq!(json["cloudflare"][0]["subdomains"][0]["proxied"], true);
    assert_eq!(
        Payload::parse(&rendered).unwrap().render().unwrap(),
        rendered
    );
}

#[test]
fn rejects_unsupported_schema_names_and_duplicates() {
    for input in [
        r#"{"version":2,"records":[{"name":"edge","proxied":false}]}"#,
        r#"{"version":1,"records":[]}"#,
        r#"{"version":1,"records":[{"name":"edge"}]}"#,
        r#"{"version":1,"records":[{"name":"edge","proxied":false}],"token":"secret"}"#,
        r#"{"version":1,"records":[{"name":"edge","proxied":false},{"name":"edge","proxied":true}]}"#,
    ] {
        assert!(PrivateConfig::parse(input).is_err());
    }
    for name in [
        "",
        "*",
        "@",
        "Edge",
        "-edge",
        "edge-",
        "edge..local",
        "edge.",
        "../../etc",
        "edge\n",
    ] {
        let input =
            serde_json::json!({"version":1,"records":[{"name":name,"proxied":false}]}).to_string();
        assert!(PrivateConfig::parse(&input).is_err(), "accepted {name:?}");
    }
}

#[test]
fn rejects_ui_overlap_and_overlong_resolved_names() {
    let config =
        PrivateConfig::parse(r#"{"version":1,"records":[{"name":"edge","proxied":false}]}"#)
            .unwrap();
    assert!(config
        .validate_zone("example.com", &["EDGE.EXAMPLE.COM".into()])
        .is_err());
    let zone = vec!["x".repeat(63); 4].join(".");
    assert!(config.validate_zone(&zone, &[]).is_err());
}

#[test]
fn errors_do_not_include_secret_values() {
    let Err(error) = Payload::parse(r#"{"cloudflare":"sensitive-do-not-print"}"#) else {
        panic!("invalid payload accepted");
    };
    assert!(!format!("{error:?} {error}").contains("sensitive-do-not-print"));
    let config =
        PrivateConfig::parse(r#"{"version":1,"records":[{"name":"edge","proxied":false}]}"#)
            .unwrap();
    assert!(config.payload("bad-zone", "dummy-token").is_err());
    let payload = config
        .payload(ZONE, "dummy-token")
        .unwrap()
        .render()
        .unwrap();
    let mut json: serde_json::Value = serde_json::from_str(&payload).unwrap();
    json["aaaa"] = true.into();
    assert!(Payload::parse(&json.to_string()).is_err());
    json["aaaa"] = false.into();
    json["purgeUnknownRecords"] = true.into();
    assert!(Payload::parse(&json.to_string()).is_err());
}
