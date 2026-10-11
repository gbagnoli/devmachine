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
        port: 8384,
        tailnet_identity: false,
        aliases: Vec::new(),
    }]
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
            port: 8088,
            tailnet_identity: false,
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
    assert_eq!(
        rendered
            .matches("respond @outside_tailnet \"Access denied by Skillet tailnet policy\" 403")
            .count(),
        2
    );
    assert!(rendered.contains("https://acme-staging-v02.api.letsencrypt.org/directory"));
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
        port: 8384,
        tailnet_identity: false,
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
        port: 8080,
        aliases: vec!["browse.{host}".into()],
        tailnet_identity: true,
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
    assert!(config.contains("request_header -X-Skillet-User"));
    assert!(config.contains("respond @cross_origin 403"));
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
            port: 8080,
            aliases: Vec::new(),
            tailnet_identity: true,
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
        &[UiService {
            name: "files".into(),
            upstream: "mikochi".into(),
            port: 8080,
            aliases: Vec::new(),
            tailnet_identity: true,
        }],
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
}
