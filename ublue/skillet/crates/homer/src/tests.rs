use super::{render, Link};

#[test]
fn pihole_dashboard_uses_same_origin_api_proxy() {
    let config = render(
        "clamps",
        &[Link {
            group: super::HOME_INFRA,
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

#[test]
fn only_populated_groups_are_rendered_in_stable_order_without_losing_card_fields() {
    let link = Link {
        group: super::FILES,
        name: "syncthing".into(),
        hostname: "sync.example.invalid".into(),
        path: None,
        endpoint: None,
        service_type: None,
        icon: None,
        logo: None,
        api_version: None,
        api_key: None,
    };
    let mut home = link.clone();
    home.group = super::HOME_INFRA;
    home.name = "homeassistant".into();
    home.service_type = Some("HomeAssistant");
    home.api_key = Some("synthetic-widget-key".into());
    let mut media = link.clone();
    media.group = super::MEDIA;
    media.name = "jellyfin".into();
    let mut custom = link.clone();
    custom.group = "Shared tools";
    custom.name = "custom".into();
    let yaml: serde_yml::Value =
        serde_yml::from_str(&render("fixture", &[home, custom, link, media]).unwrap()).unwrap();
    let groups = yaml["services"].as_sequence().unwrap();
    assert_eq!(
        groups
            .iter()
            .map(|group| group["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Media", "Files", "Home & Infra", "Shared tools"]
    );
    assert_eq!(groups[2]["items"][0]["type"], "HomeAssistant");
    assert_eq!(groups[2]["items"][0]["apikey"], "synthetic-widget-key");
    assert_eq!(groups[1]["items"][0]["url"], "https://sync.example.invalid");
    let empty: serde_yml::Value = serde_yml::from_str(&render("fixture", &[]).unwrap()).unwrap();
    assert_eq!(empty["services"].as_sequence().unwrap().len(), 0);
}

#[test]
fn regrouping_retained_cards_preserves_urls_widgets_and_unrecognized_fields() {
    let input = "title: Existing\nservices:\n  - name: Services\n    items:\n      - name: pihole\n        url: https://existing.example.invalid/admin/\n        custom: {nested: value}\n        apikey: fixture\n";
    let output: serde_yml::Value =
        serde_yml::from_str(&super::regroup(input, |_| super::HOME_INFRA).unwrap()).unwrap();
    let original: serde_yml::Value = serde_yml::from_str(input).unwrap();
    assert_eq!(output["services"][0]["name"], "Home & Infra");
    assert_eq!(
        output["services"][0]["items"],
        original["services"][0]["items"]
    );
    assert_eq!(output["title"], original["title"]);
    assert!(super::regroup("services: invalid", |_| "Unused").is_err());
}
