use super::{CaddySites, UiEnvironment, UiService};
use skillet_core::{
    system::PodmanSecretResource,
    test_utils::{MockFiles, MockSystem},
};
use std::sync::atomic::Ordering;

fn syncthing_only() -> Vec<UiService> {
    vec![UiService {
        name: "syncthing".to_string(),
        upstream: "syncthing".to_string(),
        transport: crate::UpstreamTransport::Bridge,
        port: 8384,
        tailnet_identity: false,
        anonymous_token_refresh: false,
        aliases: Vec::new(),
    }]
}

fn identity_service(
    name: &str,
    upstream: &str,
    transport: crate::UpstreamTransport,
    port: u16,
) -> UiService {
    UiService {
        name: name.into(),
        upstream: upstream.into(),
        transport,
        port,
        aliases: Vec::new(),
        tailnet_identity: true,
        anonymous_token_refresh: false,
    }
}

#[test]
fn applying_unchanged_caddy_sites_preserves_the_container() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    system
        .ensure_podman_secret("cloudflare_acme_token", "dummy-token")
        .unwrap();
    let sites = CaddySites::from_host(
        "joey",
        &UiEnvironment {
            ui_domain: "test.example.invalid".to_string(),
            acme_staging: true,
        },
        &syncthing_only(),
    )
    .unwrap();
    super::apply(&system, &files, &sites, "joey").unwrap();
    let initial_restart_count = system.restart_count.load(Ordering::SeqCst);
    let initial_config = files
        .files
        .lock()
        .unwrap()
        .get("/etc/containers/systemd/caddy.container")
        .cloned()
        .unwrap();
    let initial_config_text = String::from_utf8_lossy(&initial_config);
    assert!(initial_config_text.contains("Volume=/etc/skillet/caddy:/etc/caddy:ro,Z"));
    assert!(files
        .files
        .lock()
        .unwrap()
        .contains_key("/etc/skillet/caddy/Caddyfile"));

    super::apply(&system, &files, &sites, "joey").unwrap();

    assert_eq!(
        system.restart_count.load(Ordering::SeqCst),
        initial_restart_count
    );
    assert_eq!(
        files
            .files
            .lock()
            .unwrap()
            .get("/etc/containers/systemd/caddy.container"),
        Some(&initial_config)
    );
}

#[test]
fn derives_a_single_non_clamps_service_hostname_for_either_environment() {
    let services = syncthing_only();
    for (domain, staging) in [
        ("test.example.invalid", true),
        ("prod.example.invalid", false),
    ] {
        let sites = CaddySites::from_host(
            "joey",
            &UiEnvironment {
                ui_domain: domain.to_string(),
                acme_staging: staging,
            },
            &services,
        )
        .unwrap();
        assert_eq!(sites.services.len(), 1);
        assert_eq!(
            sites.services[0].hostname,
            format!("syncthing.joey.{domain}")
        );
        assert_eq!(sites.acme_staging, staging);
    }
}

#[test]
fn renders_each_declared_service_with_tailnet_filter_and_container_dns() {
    let services = vec![
        UiService {
            name: "pihole".to_string(),
            upstream: "pihole".to_string(),
            transport: crate::UpstreamTransport::Bridge,
            port: 8088,
            tailnet_identity: false,
            anonymous_token_refresh: false,
            aliases: Vec::new(),
        },
        syncthing_only().remove(0),
    ];
    let sites = CaddySites::from_host(
        "clamps",
        &UiEnvironment {
            ui_domain: "private.example.invalid".to_string(),
            acme_staging: true,
        },
        &services,
    )
    .unwrap();
    let rendered = sites.render();
    assert!(rendered.contains("pihole.clamps.private.example.invalid"));
    assert!(rendered.contains("syncthing.clamps.private.example.invalid"));
    assert!(rendered.contains("reverse_proxy pihole:8088"));
    assert!(rendered.contains("reverse_proxy syncthing:8384"));
    assert!(rendered
        .contains("respond @outside_tailnet \"Access denied by Skillet tailnet policy\" 403"));
    assert!(rendered.contains(
        "respond @outside_tailnet \"Access denied by Skillet tailnet policy\" 403\n        reverse_proxy pihole:8088"
    ));
    assert_eq!(
        rendered
            .matches("respond @outside_tailnet \"Access denied by Skillet tailnet policy\" 403")
            .count(),
        2
    );
    assert!(rendered.contains("https://acme-staging-v02.api.letsencrypt.org/directory"));
}

#[test]
fn homer_origins_can_read_pihole_and_homeassistant_apis_and_shared_assets() {
    let services = vec![
        UiService {
            name: "pihole".into(),
            upstream: "pihole".into(),
            transport: crate::UpstreamTransport::Bridge,
            port: 8088,
            aliases: Vec::new(),
            tailnet_identity: true,
            anonymous_token_refresh: false,
        },
        UiService {
            name: "homeassistant".into(),
            upstream: "homeassistant".into(),
            transport: crate::UpstreamTransport::Bridge,
            port: 8123,
            aliases: Vec::new(),
            tailnet_identity: true,
            anonymous_token_refresh: false,
        },
        UiService {
            name: "homer".into(),
            upstream: "homer".into(),
            transport: crate::UpstreamTransport::Bridge,
            port: 8080,
            aliases: vec!["dashboard.{host}".into()],
            tailnet_identity: true,
            anonymous_token_refresh: false,
        },
    ];
    let sites = CaddySites::from_host(
        "clamps",
        &UiEnvironment {
            ui_domain: "private.example.invalid".into(),
            acme_staging: true,
        },
        &services,
    )
    .unwrap();
    let rendered = sites.render();

    for origin in [
        "https://homer.clamps.private.example.invalid",
        "https://dashboard.clamps.private.example.invalid",
    ] {
        assert!(rendered.contains(&format!("Origin \"{origin}\"")));
        assert!(rendered.contains(&format!("Access-Control-Allow-Origin \"{origin}\"")));
    }
    assert!(rendered.contains("header_up Host pi.hole"));
    assert!(rendered.contains("header_up Origin http://pi.hole"));
    assert!(rendered.contains("@homer_pihole_api {\n            method GET\n            path /pihole-api/api/stats/summary /pihole-api/api/dns/blocking\n        }"));
    assert!(rendered.contains("uri strip_prefix /pihole-api"));
    assert!(rendered.contains("reverse_proxy pihole:8088"));
    assert!(rendered.contains("Access-Control-Allow-Methods \"GET, POST, OPTIONS\""));
    assert!(rendered.contains("Access-Control-Allow-Methods \"GET, OPTIONS\""));
    assert!(
        rendered.contains("header_up Origin https://homeassistant.clamps.private.example.invalid")
    );
    assert!(rendered
        .contains("@homer_preflight method OPTIONS\n        respond @homer_preflight \"\" 204"));
    let source_gate = rendered.find("@outside_tailnet").unwrap();
    let origin_gate = rendered.find("respond @cross_origin 403").unwrap();
    let preflight = rendered.find("respond @homer_preflight").unwrap();
    let auth_gate = rendered
        .find("forward_auth unix//run/skillet-tailnet-ui/auth.sock")
        .unwrap();
    assert!(
        source_gate < origin_gate && origin_gate < preflight && preflight < auth_gate,
        "empty preflight must pass source/origin checks before auth, and real requests must still be authorized"
    );
    assert!(rendered.contains("respond @cross_origin 403"));
    assert!(rendered.contains("respond @cross_site 403"));
}

#[test]
fn guest_payload_must_match_the_callers_declared_services() {
    let services = syncthing_only();
    let expected = CaddySites::from_host(
        "joey",
        &UiEnvironment {
            ui_domain: "test.example.invalid".to_string(),
            acme_staging: true,
        },
        &services,
    )
    .unwrap();
    let payload = serde_json::to_string(&expected).unwrap();
    assert_eq!(
        CaddySites::parse(&payload, "joey", &services).unwrap(),
        expected
    );
    assert!(CaddySites::parse(&payload, "clamps", &services).is_err());
    assert!(CaddySites::parse(&payload, "joey", &[]).is_err());
}

#[test]
fn rejects_domain_injection_duplicates_and_empty_service_sets() {
    for domain in [
        "https://example.invalid",
        "example.invalid/path",
        "example.invalid:443",
        "bad..invalid",
        "bad\n.invalid",
    ] {
        assert!(super::validate_domain(domain).is_err());
    }
    let duplicate = vec![syncthing_only()[0].clone(), syncthing_only()[0].clone()];
    assert!(CaddySites::from_host(
        "host",
        &UiEnvironment {
            ui_domain: "example.invalid".to_string(),
            acme_staging: false
        },
        &duplicate
    )
    .is_err());
    assert!(CaddySites::from_host(
        "host",
        &UiEnvironment {
            ui_domain: "example.invalid".to_string(),
            acme_staging: false
        },
        &[]
    )
    .is_err());
    assert!(super::validate_domain_in_zone("ui.other.invalid", "example.invalid").is_err());
    assert!(super::validate_domain_in_zone("ui.example.invalid", "example.invalid").is_ok());
}

#[test]
fn renders_declared_aliases_and_rejects_conflicting_names() {
    let services = vec![UiService {
        name: "syncthing".to_string(),
        upstream: "syncthing".to_string(),
        transport: crate::UpstreamTransport::Bridge,
        port: 8384,
        tailnet_identity: false,
        anonymous_token_refresh: false,
        aliases: vec!["sync".to_string(), "sync.{host}".to_string()],
    }];
    let sites = CaddySites::from_host(
        "clamps",
        &UiEnvironment {
            ui_domain: "test.example.invalid".to_string(),
            acme_staging: true,
        },
        &services,
    )
    .unwrap();
    assert_eq!(sites.machine_hostname, "clamps.test.example.invalid");
    assert_eq!(
        sites.services[0].aliases,
        vec![
            "sync.test.example.invalid",
            "sync.clamps.test.example.invalid",
        ]
    );
    let rendered = sites.render();
    assert!(rendered.contains("sync.test.example.invalid {"));
    assert!(rendered.contains("sync.clamps.test.example.invalid {"));
    assert_eq!(rendered.matches("reverse_proxy syncthing:8384").count(), 3);
    assert_eq!(rendered.matches("resolvers 1.1.1.1 1.0.0.1").count(), 3);

    for alias in ["", ".sync", "sync.", "*.sync", "bad{host}", "clamps"] {
        let invalid = vec![UiService {
            tailnet_identity: false,
            anonymous_token_refresh: false,
            aliases: vec![alias.to_string()],
            ..services[0].clone()
        }];
        assert!(CaddySites::from_host(
            "clamps",
            &UiEnvironment {
                ui_domain: "test.example.invalid".to_string(),
                acme_staging: true,
            },
            &invalid,
        )
        .is_err());
    }
}
#[test]
fn relative_ui_domain_defaults_and_always_appends_zone() {
    assert_eq!(
        super::resolve_ui_domain("example.com", None).expect("default"),
        "ui.example.com"
    );
    assert_eq!(
        super::resolve_ui_domain("example.com", Some("ui.whatever")).expect("relative prefix"),
        "ui.whatever.example.com"
    );
    assert_eq!(
        super::resolve_ui_domain("example.com", Some(" ui ")).expect("trimmed prefix"),
        "ui.example.com"
    );
    assert_eq!(
        super::resolve_ui_domain("example.com", Some("other.example.com"))
            .expect("all values are relative"),
        "other.example.com.example.com"
    );
    assert_eq!(
        super::resolve_ui_domain("example.test", None).expect("other environment"),
        "ui.example.test"
    );
}

#[test]
fn relative_ui_domain_rejects_invalid_prefixes_and_total_length() {
    for prefix in [
        "",
        " ",
        ".ui",
        "ui.",
        "ui..private",
        "https://ui",
        "ui:443",
        "ui/path",
        "ui space",
        "*",
        "{host}",
        "-ui",
        "ui-",
    ] {
        assert!(super::resolve_ui_domain("example.com", Some(prefix)).is_err());
    }
    assert!(super::resolve_ui_domain("invalid", None).is_err());
    let label = "a".repeat(63);
    let too_long = format!("{label}.{label}.{label}.{label}");
    assert!(super::resolve_ui_domain("example.com", Some(&too_long)).is_err());
}

#[test]
fn file_ui_is_gated_before_proxy_and_cannot_disable_declared_identity() {
    let services = vec![UiService {
        name: "files".into(),
        upstream: "mikochi".into(),
        transport: crate::UpstreamTransport::Bridge,
        port: 8080,
        aliases: vec!["browse.{host}".into()],
        tailnet_identity: true,
        anonymous_token_refresh: true,
    }];
    let mut sites = CaddySites::from_host(
        "joey",
        &UiEnvironment {
            ui_domain: "ui.example.invalid".into(),
            acme_staging: true,
        },
        &services,
    )
    .unwrap();
    sites.tailnet_users = vec!["alice@example.invalid".into()];
    let config = sites.render();
    assert!(config.contains("files.joey.ui.example.invalid"));
    assert_eq!(
        config
            .matches("forward_auth unix//run/skillet-tailnet-ui/auth.sock")
            .count(),
        2
    );
    assert!(config.find("forward_auth").unwrap() < config.find("reverse_proxy mikochi").unwrap());
    assert!(config.contains("header_up X-Skillet-Peer {http.request.remote}"));
    assert_eq!(config.matches("uri /auth?\n").count(), 2);
    assert!(!config.contains("uri /auth\n"));
    assert!(config.contains("request_header -X-Skillet-User"));
    assert!(config.contains("respond @cross_origin 403"));
    assert!(
        config.find("forward_auth").unwrap() < config.find("respond @anonymous_refresh").unwrap()
    );
    assert!(config.contains("method GET\n            path /api/refresh"));
    assert!(config.contains("Cache-Control no-store"));
    let mut unsafe_services = services.clone();
    unsafe_services[0].tailnet_identity = false;
    assert!(CaddySites::from_host(
        "joey",
        &UiEnvironment {
            ui_domain: "ui.example.invalid".into(),
            acme_staging: true
        },
        &unsafe_services
    )
    .is_err());
    let encoded = serde_json::to_string(&sites).unwrap();
    assert_eq!(
        CaddySites::parse(&encoded, "joey", &services).unwrap(),
        sites
    );
    sites.services[0].tailnet_identity = false;
    assert!(CaddySites::parse(&serde_json::to_string(&sites).unwrap(), "joey", &services).is_err());
}

#[test]
fn identity_caddy_has_only_the_authorization_socket_and_a_private_backend() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    system
        .ensure_podman_secret("cloudflare_acme_token", "dummy-token")
        .unwrap();
    let mut sites = CaddySites::from_host(
        "joey",
        &UiEnvironment {
            ui_domain: "ui.example.invalid".into(),
            acme_staging: true,
        },
        &[UiService {
            name: "files".into(),
            upstream: "mikochi".into(),
            transport: crate::UpstreamTransport::Bridge,
            port: 8080,
            aliases: Vec::new(),
            tailnet_identity: true,
            anonymous_token_refresh: false,
        }],
    )
    .unwrap();
    sites.tailnet_users = vec!["alice@example.invalid".into()];
    super::apply(&system, &files, &sites, "joey").unwrap();
    let records = files.files.lock().unwrap();
    let quadlet = String::from_utf8_lossy(&records["/etc/containers/systemd/caddy.container"]);
    assert!(quadlet.contains("Network=joey-files.network"));
    assert!(quadlet.contains("Volume=/run/skillet-tailnet-ui:/run/skillet-tailnet-ui:ro,z"));
    assert!(!quadlet.contains("skillet-tailscale"));
    let network = String::from_utf8_lossy(&records["/etc/containers/systemd/joey-files.network"]);
    assert!(network.contains("Internal=false"));
    assert!(network.contains("Options=isolate=strict"));
    let restarts = system.restart_count.load(Ordering::SeqCst);
    drop(records);
    super::apply(&system, &files, &sites, "joey").unwrap();
    assert_eq!(system.restart_count.load(Ordering::SeqCst), restarts);
}

#[test]
#[ignore = "requires Podman and the Caddy container image"]
fn generated_identity_caddyfile_validates_with_the_real_consumer() {
    let temp = tempfile::tempdir().unwrap();
    let sites = CaddySites::from_host(
        "clamps",
        &UiEnvironment {
            ui_domain: "ui.example.invalid".into(),
            acme_staging: true,
        },
        &[
            identity_service("pihole", "pihole", crate::UpstreamTransport::Bridge, 8088),
            identity_service(
                "homeassistant",
                "homeassistant",
                crate::UpstreamTransport::HostLoopback,
                8123,
            ),
            identity_service("homer", "homer", crate::UpstreamTransport::Bridge, 8080),
            UiService {
                name: "files".into(),
                upstream: "mikochi".into(),
                transport: crate::UpstreamTransport::Bridge,
                port: 8080,
                aliases: Vec::new(),
                tailnet_identity: true,
                anonymous_token_refresh: true,
            },
            UiService {
                name: "syncthing".into(),
                upstream: "syncthing".into(),
                transport: crate::UpstreamTransport::Bridge,
                port: 8384,
                aliases: vec!["sync.{host}".into()],
                tailnet_identity: true,
                anonymous_token_refresh: false,
            },
        ],
    )
    .unwrap();
    let path = temp.path().join("Caddyfile");
    std::fs::write(&path, sites.render()).unwrap();
    let volume = format!("{}:/etc/caddy/Caddyfile:ro,z", path.display());
    let output = std::process::Command::new("podman")
        .args([
            "run",
            "--rm",
            "--network",
            "none",
            "--volume",
            &volume,
            "--env",
            "CF_API_TOKEN=0000000000000000000000000000000000000000",
            "ghcr.io/caddybuilds/caddy-cloudflare:2",
            "caddy",
            "validate",
            "--config",
            "/etc/caddy/Caddyfile",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let adapted = std::process::Command::new("podman")
        .args([
            "run",
            "--rm",
            "--network",
            "none",
            "--volume",
            &volume,
            "ghcr.io/caddybuilds/caddy-cloudflare:2",
            "caddy",
            "adapt",
            "--config",
            "/etc/caddy/Caddyfile",
            "--adapter",
            "caddyfile",
        ])
        .output()
        .unwrap();
    assert!(
        adapted.status.success(),
        "{}",
        String::from_utf8_lossy(&adapted.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&adapted.stdout).unwrap();
    assert_identity_query_rewrites(&json, 6);
}

fn assert_identity_query_rewrites(json: &serde_json::Value, route_count: usize) {
    let mut nodes = vec![json];
    let mut auth_count = 0;
    let mut application_count = 0;
    while let Some(node) = nodes.pop() {
        match node {
            serde_json::Value::Object(fields) => {
                if fields.get("handler").and_then(serde_json::Value::as_str)
                    == Some("reverse_proxy")
                {
                    if let Some(rewrite) = fields.get("rewrite") {
                        assert_eq!(rewrite["method"], "GET");
                        assert_eq!(rewrite["uri"], "/auth?");
                        auth_count += 1;
                    } else {
                        application_count += 1;
                    }
                }
                nodes.extend(fields.values());
            }
            serde_json::Value::Array(items) => nodes.extend(items),
            _ => {}
        }
    }
    assert_eq!(
        auth_count, route_count,
        "every identity route must clear its auth query"
    );
    assert_eq!(
        application_count, route_count,
        "application requests must retain their original URI"
    );
}

#[test]
fn host_loopback_routes_are_versioned_private_and_do_not_join_backend_bridges() {
    let system = MockSystem::new();
    let files = MockFiles::new();
    system
        .ensure_podman_secret("cloudflare_acme_token", "dummy")
        .unwrap();
    let mut services = syncthing_only();
    services[0].name = "homeassistant".into();
    services[0].port = 8123;
    services[0].tailnet_identity = true;
    services[0].transport = crate::UpstreamTransport::HostLoopback;
    let sites = CaddySites::from_host(
        "joey",
        &UiEnvironment {
            ui_domain: "ui.example.invalid".into(),
            acme_staging: true,
        },
        &services,
    )
    .unwrap();
    assert_eq!(sites.version, 4);
    let rendered = sites.render();
    assert!(rendered.contains("reverse_proxy unix//run/skillet-ui-host/homeassistant.sock"));
    assert!(rendered.find("forward_auth").unwrap() < rendered.find("reverse_proxy").unwrap());
    let mut old = sites.clone();
    old.version = 3;
    assert!(CaddySites::parse(&serde_json::to_string(&old).unwrap(), "joey", &services).is_err());
    super::apply(&system, &files, &sites, "joey").unwrap();
    let data = files.files.lock().unwrap();
    let quadlet = String::from_utf8_lossy(&data["/etc/containers/systemd/caddy.container"]);
    assert!(quadlet.contains("Requires=skillet-ui-host-homeassistant.service"));
    assert!(quadlet.contains("Volume=/run/skillet-ui-host:/run/skillet-ui-host:ro,z"));
    assert!(!quadlet.contains("Network=joey-homeassistant.network"));
    let policy =
        String::from_utf8_lossy(&data["/etc/systemd/system/skillet-ui-host-policy.service"]);
    assert!(policy.contains("After=skillet-tailnet-ui-policy.service\n"));
    assert!(!policy.contains("Requires=skillet-tailnet-ui-policy.service"));
    drop(data);
    services[0].tailnet_identity = false;
    assert!(CaddySites::from_host(
        "joey",
        &UiEnvironment {
            ui_domain: "ui.example.invalid".into(),
            acme_staging: true
        },
        &services
    )
    .is_err());
}

#[test]
fn retained_bridge_payload_without_transport_remains_compatible() {
    let services = syncthing_only();
    let sites = CaddySites::from_host(
        "joey",
        &UiEnvironment {
            ui_domain: "ui.example.invalid".into(),
            acme_staging: true,
        },
        &services,
    )
    .unwrap();
    let mut payload = serde_json::to_value(&sites).unwrap();
    payload["services"][0]
        .as_object_mut()
        .unwrap()
        .remove("transport");
    assert!(CaddySites::parse(&payload.to_string(), "joey", &services).is_ok());
}
