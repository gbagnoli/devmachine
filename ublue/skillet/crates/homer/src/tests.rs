use super::{render, Link};

#[test]
fn pihole_dashboard_link_and_api_endpoint_can_use_different_paths() {
    let config = render(
        "clamps",
        &[Link {
            name: "pihole".into(),
            hostname: "pihole.clamps.private.example.invalid".into(),
            path: Some("/admin/"),
            endpoint: Some("https://pihole.clamps.private.example.invalid".into()),
            service_type: Some("PiHole"),
            logo: None,
            api_version: Some(6),
            api_key: None,
        }],
    )
    .unwrap();

    assert!(config.contains("url: https://pihole.clamps.private.example.invalid/admin/"));
    assert!(config.contains("endpoint: https://pihole.clamps.private.example.invalid"));
    assert!(config.contains("apiVersion: 6"));
    assert!(config.contains("apikey: ''") || config.contains("apikey: \"\""));
}
