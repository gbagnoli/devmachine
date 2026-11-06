use crate::{cache, CaddySites, SiteAccess, UiEnvironment};
use skillet_core::{
    system::PodmanSecretResource,
    test_utils::{MockFiles, MockSystem},
};

pub(super) fn sites() -> CaddySites {
    let mut service = crate::public_tests::public_service();
    service.name = "jellyfin".into();
    service.upstream = "jellyfin".into();
    service.port = 8096;
    service.public_names = vec!["video".into(), "media".into()];
    service.cache = Some(cache::Policy::JellyfinStatic);
    CaddySites::from_host_with_zone(
        "server",
        &UiEnvironment {
            ui_domain: "ui.example.invalid".into(),
            acme_staging: true,
        },
        &[service],
        Some("example.invalid"),
    )
    .unwrap()
}

#[test]
fn cache_is_caller_bound_public_only_and_has_a_bounded_disposable_mount() {
    let sites = sites();
    assert_eq!(sites.version, 7);
    assert_eq!(sites.services[0].access, SiteAccess::Public);
    let rendered = sites.render();
    assert_eq!(rendered.matches("handle @static_cache").count(), 2);
    assert!(rendered.contains("max_cacheable_body_bytes 2097152"));
    assert!(rendered.contains("default_cache_control no-store"));
    assert!(!rendered.contains("/Items/"));
    let mut service = crate::public_tests::public_service();
    service.cache = Some(cache::Policy::JellyfinStatic);
    service.public_names.clear();
    assert!(CaddySites::from_host(
        "server",
        &UiEnvironment {
            ui_domain: "ui.example.invalid".into(),
            acme_staging: true
        },
        &[service]
    )
    .is_err());
    let system = MockSystem::new();
    system
        .ensure_podman_secret("cloudflare_acme_token", "fixture")
        .unwrap();
    let files = MockFiles::new();
    crate::apply(&system, &files, &sites, "server").unwrap();
    let state = files.files.lock().unwrap();
    let quadlet = String::from_utf8_lossy(&state["/etc/containers/systemd/caddy.container"]);
    assert!(quadlet.contains(cache::IMAGE));
    assert!(quadlet.contains("size=128m"));
    assert!(quadlet.contains("--memory=512m"));
    assert!(!quadlet.contains("/syncthing/"));
}

#[test]
fn cache_policy_cannot_be_injected_or_downgraded() {
    let mut service = crate::public_tests::public_service();
    let plain = CaddySites::from_host_with_zone(
        "server",
        &UiEnvironment {
            ui_domain: "ui.example.invalid".into(),
            acme_staging: true,
        },
        &[service.clone()],
        Some("example.invalid"),
    )
    .unwrap();
    service.cache = Some(cache::Policy::JellyfinStatic);
    // Retained v6 continues uncached; cache activation needs new delivery.
    assert_eq!(
        CaddySites::parse(
            &serde_json::to_string(&plain).unwrap(),
            "server",
            &[service.clone()]
        )
        .unwrap()
        .services[0]
            .cache,
        None
    );
    let mut changed = plain;
    changed.services[0].cache = Some(cache::Policy::JellyfinStatic);
    assert!(CaddySites::parse(
        &serde_json::to_string(&changed).unwrap(),
        "server",
        &[service]
    )
    .is_err());
}
