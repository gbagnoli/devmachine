use super::{render, Link};

#[test]
fn pihole_dashboard_uses_same_origin_api_proxy() {
    let config = render(
        "clamps",
        &[Link {
            name: "pihole".into(),
            hostname: "pihole.clamps.private.example.invalid".into(),
            path: Some("/admin/"),
            endpoint: Some("/pihole-api".into()),
            service_type: Some("PiHole"),
            icon: None,
            logo: None,
            api_version: Some(6),
            api_key: None,
        }],
    )
    .unwrap();

    assert!(config.contains("url: https://pihole.clamps.private.example.invalid/admin/"));
    assert!(config.contains("endpoint: /pihole-api"));
    assert!(config.contains("apiVersion: 6"));
    assert!(!config.contains("apikey:"));
}
