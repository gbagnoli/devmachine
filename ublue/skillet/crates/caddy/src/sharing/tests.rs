use super::*;
#[test]
fn declaration_adds_separate_public_site_and_preserves_private_authentication() {
    let mut service = crate::UiService {
        name: "gokapi".into(),
        upstream: "gokapi".into(),
        port: 53842,
        transport: UpstreamTransport::Bridge,
        aliases: vec![],
        public_names: vec![],
        cache: None,
        tailnet_identity: true,
        anonymous_token_refresh: false,
        sharing: Some(Declaration {
            names: vec!["send".into()],
            policy: Policy::Gokapi224,
        }),
    };
    let environment = crate::UiEnvironment {
        ui_domain: "ui.example.invalid".into(),
        acme_staging: true,
    };
    let sites = crate::CaddySites::from_host_with_zone(
        "fixture",
        &environment,
        &[service.clone()],
        Some("example.invalid"),
    )
    .unwrap();
    assert_eq!(sites.version, 8);
    assert_eq!(sites.services[0].access, SiteAccess::Tailnet);
    assert!(sites.services[0].tailnet_identity);
    assert_eq!(sites.services[1].hostname, "send.example.invalid");
    assert_eq!(sites.services[1].access, SiteAccess::Public);
    assert!(!sites.services[1].tailnet_identity);
    assert_eq!(sites.services[1].upstream, sites.services[0].upstream);
    let payload = serde_json::to_string(&sites).unwrap();
    crate::CaddySites::parse(&payload, "fixture", &[service.clone()]).unwrap();
    let mut forged = sites.clone();
    forged.services[1].public_routes = None;
    assert!(crate::CaddySites::parse(
        &serde_json::to_string(&forged).unwrap(),
        "fixture",
        &[service.clone()]
    )
    .is_err());
    service.tailnet_identity = false;
    assert!(crate::CaddySites::from_host_with_zone(
        "fixture",
        &environment,
        &[service],
        Some("example.invalid")
    )
    .is_err());
}

#[test]
fn retained_private_payload_does_not_implicitly_enable_new_public_sharing() {
    let declaration = UiService {
        name: "gokapi".into(),
        upstream: "gokapi".into(),
        port: 53842,
        transport: UpstreamTransport::Bridge,
        aliases: vec![],
        public_names: vec![],
        cache: None,
        tailnet_identity: true,
        anonymous_token_refresh: false,
        sharing: Some(Declaration {
            names: vec!["send".into()],
            policy: Policy::Gokapi224,
        }),
    };
    let environment = crate::UiEnvironment {
        ui_domain: "ui.example.invalid".into(),
        acme_staging: true,
    };
    let mut retained = crate::CaddySites::from_host_with_zone(
        "fixture",
        &environment,
        std::slice::from_ref(&declaration),
        Some("example.invalid"),
    )
    .unwrap();
    retained.version = 7;
    retained.services.pop();
    let parsed = crate::CaddySites::parse(
        &serde_json::to_string(&retained).unwrap(),
        "fixture",
        &[declaration],
    )
    .unwrap();
    assert_eq!(parsed.services.len(), 1);
    assert!(parsed.services[0].tailnet_identity);
    assert_eq!(parsed.services[0].access, SiteAccess::Tailnet);
}
