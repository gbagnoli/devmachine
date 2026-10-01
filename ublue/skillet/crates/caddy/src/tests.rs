use super::CaddySites;

#[test]
fn rejects_invalid_duplicate_and_unknown_site_configuration() {
    for payload in [
        r#"{"pihole":"pihole.example.invalid\n}","syncthing":"sync.example.invalid"}"#,
        r#"{"pihole":"https://pihole.example.invalid","syncthing":"sync.example.invalid"}"#,
        r#"{"pihole":"same.example.invalid","syncthing":"same.example.invalid"}"#,
        r#"{"pihole":"pihole.example.invalid","syncthing":"sync.example.invalid","unknown":true}"#,
    ] {
        assert!(CaddySites::parse(payload).is_err());
    }
}

#[test]
fn staging_routes_use_bridge_dns_and_restrict_both_address_families() {
    let sites = CaddySites::parse(
        r#"{"pihole":"pihole.example.invalid","syncthing":"sync.example.invalid","acme_staging":true}"#,
    )
    .unwrap();
    let config = sites.render();
    assert!(config.contains("https://acme-staging-v02.api.letsencrypt.org/directory"));
    assert!(config.contains("acme_dns cloudflare {env.CF_API_TOKEN}"));
    assert!(config.contains("reverse_proxy pihole:8088"));
    assert!(config.contains("reverse_proxy syncthing:8384"));
    assert_eq!(config.matches("respond @outside_tailnet 403").count(), 2);
    assert_eq!(
        config.matches("100.64.0.0/10 fd7a:115c:a1e0::/48").count(),
        2
    );
    let production = CaddySites::parse(
        r#"{"pihole":"pihole.example.invalid","syncthing":"sync.example.invalid"}"#,
    )
    .unwrap();
    assert!(!production.render().contains("acme_ca"));
}
