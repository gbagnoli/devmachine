use super::{
    profile::{
        HostId, HostProfile, HostService, NetworkPolicy, ServiceConfig, UiServiceDeclaration,
    },
    tailscale_config, ui_config_for_host, TAILSCALE_AUTH_KEY_CREDENTIAL,
};
use skillet_core::{
    credentials::CredentialInputs, files::LocalFileResource, test_utils::MockSystem,
};
use skillet_podman::SecretTarget;

#[test]
fn boot_expectations_are_profile_inputs_and_unknown_profiles_are_refused() {
    let clamps = super::boot_policy_for_host("clamps").unwrap();
    assert_eq!(clamps.signed_image, "ghcr.io/gbagnoli/ucore-clamps");
    assert_eq!(clamps.masked_units, ["systemd-resolved.service"]);
    let other = super::boot_policy_for_host("beezelbot").unwrap();
    assert!(other.masked_units.is_empty());
    assert!(super::boot_policy_for_host("clamps-test-smoke").is_none());
    assert!(super::boot_policy_for_host("unknown").is_none());
}

#[test]
fn clamps_tailscale_uses_host_network_and_persistent_state() {
    let config = tailscale_config(
        "clamps-test-smoke",
        "test-auth-key".to_string(),
        "/var/lib/data/tailscale",
    );

    assert_eq!(config.name, "tailscale");
    assert_eq!(config.image, "docker.io/tailscale/tailscale:stable");
    assert!(config.networks.is_empty());
    assert_eq!(config.volumes.len(), 1);
    assert_eq!(config.volumes[0].host_path, "/var/lib/data/tailscale");
    assert_eq!(config.volumes[0].container_path, "/var/lib/tailscale");
    let container = &config.extra_config["Container"];
    assert!(container.contains(&"ContainerName=tailscale".to_string()));
    assert!(container.contains(&"Network=host".to_string()));
    assert!(container.contains(&"AddCapability=NET_ADMIN".to_string()));
    assert!(container.contains(&"AddCapability=NET_RAW".to_string()));
    assert!(container.contains(&"AddDevice=/dev/net/tun:/dev/net/tun".to_string()));
    assert!(container.contains(&"Environment=TS_AUTH_ONCE=true".to_string()));
    assert!(container.contains(&"Environment=TS_ACCEPT_DNS=false".to_string()));
    assert!(container.contains(&"Environment=TS_HOSTNAME=clamps-test-smoke".to_string()));
    assert!(config.secrets.iter().any(|secret| {
        secret.secret_name == TAILSCALE_AUTH_KEY_CREDENTIAL
            && matches!(
                &secret.target,
                SecretTarget::Environment { env_var_name } if env_var_name == "TS_AUTHKEY"
            )
    }));
}

#[test]
fn profile_capabilities_are_the_authority_for_services_credentials_network_and_snapshots() {
    let clamps = super::profile_for_name("clamps").unwrap();
    assert_eq!(
        clamps
            .services
            .iter()
            .map(HostService::name)
            .collect::<Vec<_>>(),
        ["pihole", "tailscale", "syncthing", "unifi", "btrbk"]
    );
    let credentials = clamps.credential_consumers();
    assert!(credentials.iter().any(|use_| {
        use_.credential == super::PIHOLE_WEB_PASSWORD_CREDENTIAL && use_.unit == "pihole.service"
    }));
    assert!(credentials.iter().any(|use_| {
        use_.credential == super::TAILSCALE_AUTH_KEY_CREDENTIAL && use_.unit == "tailscale.service"
    }));
    assert!(credentials.iter().any(|use_| {
        use_.credential == super::CLOUDFLARE_ACME_TOKEN_CREDENTIAL && use_.unit == "caddy.service"
    }));
    assert_eq!(
        clamps.btrbk_config().unwrap().snapshot_subvolumes,
        [std::path::PathBuf::from("syncthing")]
    );
    assert_eq!(clamps.signed_image, Some("ghcr.io/gbagnoli/ucore-clamps"));
    assert_eq!(clamps.masked_units, ["systemd-resolved.service"]);
    assert!(clamps.requires_data_mount);
    assert!(clamps.requires_pihole_dns_listener_policy());
    let network = clamps.service_network();
    assert!(network.options.contains(&"IPv6=true".to_string()));
    assert!(network
        .options
        .contains(&"Subnet=172.26.26.0/24".to_string()));
    assert!(network
        .options
        .contains(&"Subnet=fd59:4e23:2950:11f5::/64".to_string()));

    let beezelbot = super::profile_for_name("beezelbot").unwrap();
    assert_eq!(beezelbot.services.len(), 1);
    assert!(beezelbot.supports_service("syncthing"));
    assert!(!beezelbot.supports_service("pihole"));
    assert!(beezelbot.btrbk_config().is_none());
    assert_eq!(
        beezelbot.signed_image,
        Some("ghcr.io/gbagnoli/ucore-beezelbot")
    );
    assert!(beezelbot.masked_units.is_empty());
    assert!(beezelbot.requires_data_mount);
    assert!(!beezelbot.requires_pihole_dns_listener_policy());
    assert_eq!(
        beezelbot.credential_consumers(),
        [
            super::CredentialConsumer {
                credential: super::CADDY_SITES_CREDENTIAL,
                unit: "caddy.service",
            },
            super::CredentialConsumer {
                credential: super::CLOUDFLARE_ACME_TOKEN_CREDENTIAL,
                unit: "caddy.service",
            }
        ]
    );
}

#[test]
fn agent_baseline_declares_no_ui_or_credentials_and_unknown_full_profiles_fail() {
    let baseline = super::profile_for_name("agent").unwrap();
    assert!(baseline.services.is_empty());
    assert!(baseline.ui_services().is_empty());
    assert!(baseline.credential_consumers().is_empty());
    assert!(super::ui_config_for_host("agent").is_none());
    let system = MockSystem::new();
    let files = LocalFileResource::new();
    assert!(matches!(
        super::apply_host(
            "unknown-host",
            &system,
            &files,
            &CredentialInputs::default()
        ),
        Err(super::ApplyError::UnknownHost(_))
    ));
    assert!(system.podman_secrets.lock().unwrap().is_empty());
}

#[test]
fn credential_values_are_selected_by_host_and_apply_phase() {
    assert!(
        super::credentials_for_phase("clamps", super::HostApplyPhase::Base)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        super::credentials_for_phase("clamps", super::HostApplyPhase::Full).unwrap(),
        [
            super::PIHOLE_WEB_PASSWORD_CREDENTIAL,
            super::TAILSCALE_AUTH_KEY_CREDENTIAL
        ]
    );
    assert_eq!(
        super::credentials_for_phase("clamps", super::HostApplyPhase::Caddy).unwrap(),
        [
            super::CADDY_SITES_CREDENTIAL,
            super::CLOUDFLARE_ACME_TOKEN_CREDENTIAL
        ]
    );
    assert!(
        super::credentials_for_phase("beezelbot", super::HostApplyPhase::Full)
            .unwrap()
            .is_empty()
    );
    assert!(super::credentials_for_phase("unknown", super::HostApplyPhase::Base).is_err());
}

#[test]
fn host_ui_declarations_include_only_the_services_each_host_runs() {
    let clamps = ui_config_for_host("clamps").expect("clamps UI declaration");
    assert_eq!(
        clamps
            .services
            .iter()
            .map(|service| service.name.as_str())
            .collect::<Vec<_>>(),
        ["pihole", "syncthing"]
    );
    assert_eq!(clamps.network.unit_name, "clamps");
    assert_eq!(clamps.services[0].upstream, "pihole");
    assert_eq!(clamps.services[0].port, 8088);
    assert!(clamps.services[0].aliases.is_empty());
    assert_eq!(clamps.services[1].upstream, "syncthing");
    assert_eq!(clamps.services[1].port, 8384);
    assert_eq!(clamps.services[1].aliases, ["sync.{host}"]);

    let beezelbot = ui_config_for_host("beezelbot").expect("beezelbot UI declaration");
    assert_eq!(beezelbot.services.len(), 1);
    assert_eq!(beezelbot.services[0].name, "syncthing");
    assert_eq!(beezelbot.services[0].aliases, ["sync.{host}"]);
    assert_eq!(beezelbot.network.unit_name, "beezelbot");
    assert!(ui_config_for_host("unknown-host").is_none());
}

#[test]
fn host_ids_validate_ascii_dns_label_syntax() {
    for valid in ["a", "clamps", "remote-node-2"] {
        assert_eq!(
            HostId::parse(valid).as_ref().map(HostId::as_str),
            Some(valid)
        );
    }
    let max_length = "a".repeat(63);
    assert_eq!(
        HostId::parse(&max_length).as_ref().map(HostId::as_str),
        Some(max_length.as_str())
    );
    for invalid in [
        "",
        "Upper",
        "-leading",
        "trailing-",
        "has space",
        "a.b",
        "é",
    ] {
        assert!(HostId::parse(invalid).is_none(), "accepted {invalid:?}");
    }
}

#[test]
fn synthetic_profile_derives_network_and_ui_from_its_identity() {
    let profile = HostProfile {
        id: HostId::parse("remote-node").unwrap(),
        signed_image: None,
        masked_units: &[],
        network: NetworkPolicy {
            ipv4_subnet: "192.0.2.0/24",
            ipv4_gateway: "192.0.2.1",
            ipv6_subnet: "2001:db8::/64",
            ipv6_gateway: "2001:db8::1",
        },
        requires_data_mount: false,
        services: vec![HostService {
            config: ServiceConfig::Syncthing {
                data_path: "/srv/sync",
                data_owner: "sync",
                data_group: "sync",
                uid: 1234,
                gid: 1234,
            },
            ui: Some(UiServiceDeclaration {
                name: "sync",
                upstream: "sync",
                port: 8384,
                aliases: &["files.{host}"],
            }),
        }],
    };
    assert_eq!(profile.service_network().unit_name, "remote-node");
    assert_eq!(profile.ui_services()[0].name, "sync");
    assert_eq!(profile.ui_services()[0].aliases, ["files.{host}"]);
    assert!(profile.credential_consumers().iter().any(|use_| {
        use_.credential == super::CADDY_SITES_CREDENTIAL && use_.unit == "caddy.service"
    }));
}

#[test]
fn a_credential_service_remains_eligible_when_its_ui_is_not_exposed() {
    let profile = HostProfile {
        id: HostId::parse("pihole-node").unwrap(),
        signed_image: None,
        masked_units: &[],
        network: NetworkPolicy {
            ipv4_subnet: "192.0.2.0/24",
            ipv4_gateway: "192.0.2.1",
            ipv6_subnet: "2001:db8::/64",
            ipv6_gateway: "2001:db8::1",
        },
        requires_data_mount: false,
        services: vec![HostService {
            config: ServiceConfig::Pihole { custom_dns: &[] },
            ui: None,
        }],
    };
    assert!(profile.ui_services().is_empty());
    assert_eq!(
        profile.credential_consumers(),
        [super::CredentialConsumer {
            credential: super::PIHOLE_WEB_PASSWORD_CREDENTIAL,
            unit: "pihole.service",
        }]
    );
}
