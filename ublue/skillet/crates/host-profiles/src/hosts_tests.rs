use super::{
    profile::{
        AcceptanceOwner, HostId, HostProfile, HostService, NetworkPolicy, ServiceConfig,
        UiServiceDeclaration,
    },
    tailscale_config, ui_config_for_host, TAILSCALE_AUTH_KEY_CREDENTIAL,
};
use skillet_core::{
    credentials::CredentialInputs, files::LocalFileResource, test_utils::MockSystem,
};
use skillet_podman::SecretTarget;

#[test]
fn application_and_database_acceptance_mounts_match_rendered_quadlets() {
    use skillet_core::{system::AccountResource, test_utils::MockFiles};
    use std::{collections::BTreeSet, path::Path};
    let profile = super::profile_for_name("donbot").unwrap();
    let system = MockSystem::new();
    system
        .ensure_user("giacomo", Some(1000), Some(1000))
        .unwrap();
    system.ensure_group("giacomo", Some(1000)).unwrap();
    let files = MockFiles::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    files.record_btrfs_subvolume(Path::new("/var/lib/data/syncthing"));
    for service in &profile.services {
        match service.config {
            ServiceConfig::Joplin(config) => skillet_joplin::apply(
                &system,
                &files,
                "donbot",
                config,
                "https://joplin.example.invalid",
                &"a".repeat(32),
            )
            .unwrap(),
            ServiceConfig::Airtrail(config) => skillet_airtrail::apply(
                &system,
                &files,
                "donbot",
                config,
                "https://airtrail.example.invalid",
                &"b".repeat(32),
                &skillet_airtrail::Integrations::default(),
            )
            .unwrap(),
            _ => {}
        }
    }
    let quadlets = files.files.lock().unwrap();
    let plan = profile.acceptance_plan();
    let mut checked = 0;
    for service in &plan.services {
        let Some(name) = service.container.as_deref() else {
            continue;
        };
        if !matches!(
            name,
            "joplin-server" | "airtrail" | "postgresql-joplin" | "postgresql-airtrail"
        ) {
            continue;
        }
        let path = format!("/etc/containers/systemd/{name}.container");
        let unit = std::str::from_utf8(&quadlets[&path]).unwrap();
        let actual = unit
            .lines()
            .filter_map(|line| line.strip_prefix("Volume="))
            .map(|volume| volume.split(':').next().unwrap())
            .collect::<BTreeSet<_>>();
        let expected = service
            .bind_paths
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "acceptance mounts for {name}");
        checked += 1;
    }
    assert_eq!(checked, 4, "both app-dedicated databases must be checked");
    let metadata = files.directory_metadata.lock().unwrap();
    for path in [
        "/var/lib/data/syncthing/data/backups",
        "/var/lib/data/syncthing/data/backups/postgres",
        "/var/lib/data/syncthing/data/backups/airtrail",
    ] {
        assert_eq!(
            metadata[path].1,
            skillet_core::files::Ownership::named(Some("giacomo"), Some("giacomo"))
        );
    }
}

#[test]
fn datadog_is_optional_and_separately_credential_gated() {
    let files = skillet_core::test_utils::MockFiles::new();
    let system = MockSystem::new();
    assert!(super::apply_host_phase(
        "clamps",
        super::HostApplyPhase::Datadog,
        &system,
        &files,
        &CredentialInputs::default()
    )
    .is_err());
    assert!(files.files.lock().unwrap().is_empty());
    assert!(system.podman_secrets.lock().unwrap().is_empty());
    let clamps = super::profile_for_name("clamps").unwrap();
    assert!(clamps.supports_service("datadog"));
    assert!(!super::profile_for_name("agent")
        .unwrap()
        .supports_service("datadog"));
    assert_eq!(
        super::credentials_for_phase("clamps", super::HostApplyPhase::Datadog).unwrap(),
        ["datadog_config"]
    );
    assert!(
        !super::credentials_for_phase("clamps", super::HostApplyPhase::Full)
            .unwrap()
            .contains(&"datadog_config")
    );
    assert!(clamps
        .acceptance_plan()
        .services
        .iter()
        .all(|service| service.unit != "datadog-agent.service"));
}

#[test]
fn boot_expectations_are_profile_inputs_and_unknown_profiles_are_refused() {
    let clamps = super::boot_policy_for_host("clamps").unwrap();
    assert_eq!(clamps.signed_image, "ghcr.io/gbagnoli/ucore-clamps");
    assert_eq!(clamps.masked_units, ["systemd-resolved.service"]);
    let other = super::boot_policy_for_host("joey").unwrap();
    assert_eq!(other.masked_units, ["systemd-resolved.service"]);
    assert_eq!(other.signed_image, "ghcr.io/gbagnoli/ucore-joey");
    assert!(super::boot_policy_for_host("clamps-test-smoke").is_none());
    assert!(super::boot_policy_for_host("unknown").is_none());
}

#[test]
fn clamps_tailscale_uses_host_network_and_persistent_state() {
    let config = tailscale_config(
        "clamps-test-smoke",
        "test-auth-key".to_string(),
        "/var/lib/data/tailscale",
        false,
    );

    assert_eq!(config.name, "tailscale");
    assert_eq!(config.image, "docker.io/tailscale/tailscale:stable");
    assert_eq!(
        config.network_attachments,
        [skillet_podman::NetworkAttachment::Host]
    );
    assert_eq!(config.volumes.len(), 2);
    assert_eq!(config.volumes[0].host_path, "/var/lib/data/tailscale");
    assert_eq!(config.volumes[0].container_path, "/var/lib/tailscale");
    let container = &config.extra_config["Container"];
    assert!(container.contains(&"ContainerName=tailscale".to_string()));
    assert_eq!(
        config.network_attachments,
        [skillet_podman::NetworkAttachment::Host]
    );
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
        [
            "network",
            "pihole",
            "tailscale",
            "syncthing",
            "nut",
            "homeassistant",
            "unifi",
            "ddns",
            "datadog",
            "btrbk",
            "mikochi",
            "homer",
            "smtp"
        ]
    );
    let credentials = clamps.credential_consumers();
    assert!(!credentials
        .iter()
        .any(|use_| { use_.credential == super::PIHOLE_WEB_PASSWORD_CREDENTIAL }));
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

    let joey = super::profile_for_name("joey").unwrap();
    assert_eq!(
        joey.services
            .iter()
            .map(HostService::name)
            .collect::<Vec<_>>(),
        [
            "pihole",
            "tailscale",
            "syncthing",
            "ddns",
            "datadog",
            "btrbk",
            "mikochi",
            "homer",
            "smtp"
        ]
    );
    assert!(!joey.supports_service("unifi"));
    assert_eq!(
        joey.btrbk_config().unwrap().snapshot_subvolumes,
        [std::path::PathBuf::from("syncthing")]
    );
    assert_eq!(joey.signed_image, Some("ghcr.io/gbagnoli/ucore-joey"));
    assert_eq!(joey.masked_units, ["systemd-resolved.service"]);
    assert!(joey.requires_data_mount);
    assert!(joey.requires_pihole_dns_listener_policy());
    assert!(joey.tailscale_exit_node());
    assert!(joey
        .services
        .iter()
        .any(|service| matches!(service.config, ServiceConfig::Pihole)));
}

#[test]
fn donbot_declares_joplin_and_airtrail_uis_that_register_with_homer_and_caddy() {
    let donbot = super::profile_for_name("donbot").expect("Donbot profile");
    assert!(donbot.supports_service("homer"));
    assert!(donbot.supports_service("joplin"));
    assert!(donbot.supports_service("airtrail"));
    assert!(donbot.supports_service("media"));
    assert!(donbot.supports_production_service("media"));
    assert!(
        super::credentials_for_phase("donbot", super::HostApplyPhase::Full)
            .unwrap()
            .iter()
            .all(|name| *name != skillet_media::CREDENTIAL)
    );
    assert_eq!(
        super::credentials_for_phase("donbot", super::HostApplyPhase::Media).unwrap(),
        [skillet_media::CREDENTIAL, super::CADDY_SITES_CREDENTIAL]
    );
    assert_eq!(
        donbot
            .ui_services()
            .iter()
            .map(|service| service.name.as_str())
            .collect::<Vec<_>>(),
        [
            "pihole",
            "syncthing",
            "joplin",
            "airtrail",
            "prowlarr",
            "sonarr",
            "radarr",
            "lidarr",
            "files",
            "homer"
        ]
    );
    assert!(
        super::credentials_for_phase("donbot", super::HostApplyPhase::Full)
            .unwrap()
            .iter()
            .all(|name| *name != skillet_joplin::DATABASE_PASSWORD_CREDENTIAL)
    );
    assert!(
        super::credentials_for_phase("donbot", super::HostApplyPhase::Full)
            .unwrap()
            .iter()
            .all(|name| {
                *name != skillet_airtrail::DATABASE_PASSWORD_CREDENTIAL
                    && *name != skillet_airtrail::INTEGRATIONS_CREDENTIAL
            })
    );
    assert_eq!(
        super::credentials_for_phase("donbot", super::HostApplyPhase::Joplin).unwrap(),
        [
            skillet_joplin::DATABASE_PASSWORD_CREDENTIAL,
            super::CADDY_SITES_CREDENTIAL
        ]
    );
    assert!(donbot
        .services
        .iter()
        .any(|service| matches!(service.config, ServiceConfig::Joplin(_))));
    assert!(donbot
        .services
        .iter()
        .any(|service| matches!(service.config, ServiceConfig::Airtrail(_))));
    assert_eq!(
        super::credentials_for_phase("donbot", super::HostApplyPhase::Airtrail).unwrap(),
        [
            skillet_airtrail::DATABASE_PASSWORD_CREDENTIAL,
            skillet_airtrail::INTEGRATIONS_CREDENTIAL,
            super::CADDY_SITES_CREDENTIAL
        ]
    );
}

#[test]
fn agent_baseline_declares_no_ui_or_credentials_and_unknown_full_profiles_fail() {
    let baseline = super::profile_for_name("agent").unwrap();
    assert_eq!(baseline.services, Vec::<HostService>::new());
    assert_eq!(
        baseline.ui_services(),
        Vec::<skillet_caddy::UiService>::new()
    );
    assert_eq!(
        baseline.credential_consumers(),
        Vec::<super::CredentialConsumer>::new()
    );
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
    assert_eq!(
        super::credentials_for_phase("clamps", super::HostApplyPhase::Ddns).unwrap(),
        [super::CLOUDFLARE_DDNS_CONFIG_CREDENTIAL]
    );
    assert_eq!(
        super::credentials_for_phase("clamps", super::HostApplyPhase::Base).unwrap(),
        Vec::<&str>::new()
    );
    assert_eq!(
        super::credentials_for_phase("clamps", super::HostApplyPhase::Full).unwrap(),
        [super::TAILSCALE_AUTH_KEY_CREDENTIAL]
    );
    assert_eq!(
        super::credentials_for_phase("clamps", super::HostApplyPhase::Caddy).unwrap(),
        [
            super::CADDY_SITES_CREDENTIAL,
            super::CLOUDFLARE_ACME_TOKEN_CREDENTIAL
        ]
    );
    assert_eq!(
        super::credentials_for_phase("joey", super::HostApplyPhase::Full).unwrap(),
        [super::TAILSCALE_AUTH_KEY_CREDENTIAL]
    );
    assert!(super::credentials_for_phase("unknown", super::HostApplyPhase::Base).is_err());
}

#[test]
fn ddns_is_caller_selected_and_independently_credential_gated() {
    let system = MockSystem::new();
    let files = skillet_core::test_utils::MockFiles::new();
    let credentials = CredentialInputs::default();
    assert!(super::apply_host_phase(
        "clamps",
        super::HostApplyPhase::Ddns,
        &system,
        &files,
        &credentials
    )
    .is_err());
    assert!(super::apply_host_phase(
        "agent",
        super::HostApplyPhase::Ddns,
        &system,
        &files,
        &credentials
    )
    .is_err());
    assert!(system.podman_secrets.lock().unwrap().is_empty());
    assert!(files.files.lock().unwrap().is_empty());
    let profile = super::profile_for_name("clamps").unwrap();
    assert!(profile.supports_service("ddns"));
    assert!(!profile.supports_production_service("ddns"));
    assert!(!profile
        .acceptance_plan()
        .services
        .iter()
        .any(|service| service.unit == "cloudflare-ddns.service"));
    assert!(profile
        .acceptance_plan_with_ddns(true)
        .services
        .iter()
        .any(|service| service.unit == "cloudflare-ddns.service"));
    assert!(!super::profile_for_name("agent")
        .unwrap()
        .supports_service("ddns"));
    assert!(super::profile_for_name("joey")
        .unwrap()
        .supports_production_service("ddns"));
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
        ["pihole", "syncthing", "homeassistant", "files", "homer"]
    );
    assert_eq!(clamps.network_name, "clamps");
    assert_eq!(clamps.services[0].upstream, "pihole");
    assert_eq!(clamps.services[0].port, 8088);
    assert!(clamps.services[0].tailnet_identity);
    assert!(clamps.services[3].anonymous_token_refresh);
    assert_eq!(
        clamps.services[2].transport,
        skillet_caddy::UpstreamTransport::HostLoopback
    );
    assert_eq!(clamps.services[0].aliases, Vec::<String>::new());
    assert_eq!(clamps.services[1].upstream, "syncthing");
    assert_eq!(clamps.services[1].port, 8384);
    assert!(clamps.services[1].tailnet_identity);
    assert_eq!(clamps.services[1].aliases, ["sync.{host}"]);

    let joey = ui_config_for_host("joey").expect("joey UI declaration");
    assert_eq!(joey.services.len(), 4);
    assert_eq!(joey.services[0].name, "pihole");
    assert!(joey.services[0].tailnet_identity);
    assert!(joey.services[2].anonymous_token_refresh);
    assert_eq!(joey.services[1].name, "syncthing");
    assert!(joey.services[1].tailnet_identity);
    assert_eq!(joey.services[1].aliases, ["sync.{host}"]);
    assert_eq!(joey.network_name, "joey");
    assert!(ui_config_for_host("unknown-host").is_none());
}

#[test]
fn application_acceptance_tracks_each_profiles_declared_services() {
    use super::{profile_for_name, HealthProbe, ListenerProtocol};

    let clamps = profile_for_name("clamps").unwrap().acceptance_plan();
    let clamps_names = clamps
        .services
        .iter()
        .map(|service| service.unit.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        clamps_names,
        [
            "pihole.service",
            "tailscale.service",
            "syncthing.service",
            "homeassistant.service",
            "unifi.service",
            "skillet-btrbk.timer",
            "mikochi.service",
            "homer.service",
            "caddy.service",
            "skillet-tailnet-ui.service"
        ]
    );
    let pihole = clamps
        .services
        .iter()
        .find(|service| service.unit == "pihole.service")
        .unwrap();
    assert_eq!(pihole.health_probe, Some(HealthProbe::Pihole));
    assert_eq!(pihole.owner, None);
    assert!(pihole
        .listeners
        .iter()
        .any(|listener| listener.port == 53 && listener.protocol == ListenerProtocol::Udp));
    let syncthing = clamps
        .services
        .iter()
        .find(|service| service.unit == "syncthing.service")
        .unwrap();
    assert_eq!(
        syncthing.owner,
        Some(AcceptanceOwner::Named {
            user: "giacomo".into(),
            group: "giacomo".into()
        })
    );
    let unifi = clamps
        .services
        .iter()
        .find(|service| service.unit == "unifi.service")
        .unwrap();
    assert_eq!(
        unifi.owner,
        Some(AcceptanceOwner::Numeric { uid: 999, gid: 999 })
    );

    let joey = profile_for_name("joey").unwrap().acceptance_plan();
    assert_eq!(
        joey.services
            .iter()
            .map(|service| service.unit.as_str())
            .collect::<Vec<_>>(),
        [
            "pihole.service",
            "tailscale.service",
            "syncthing.service",
            "skillet-btrbk.timer",
            "mikochi.service",
            "homer.service",
            "caddy.service",
            "skillet-tailnet-ui.service"
        ]
    );
    assert!(joey
        .services
        .iter()
        .filter(|service| matches!(service.container.as_deref(), Some("caddy")))
        .all(|service| service.network_mode.as_deref() == Some("joey")));
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
                files_path: "/var/lib/data/syncthing/data",
                data_path: "/srv/sync",
                data_owner: "sync",
                data_group: "sync",
                container_uid: 1000,
                container_gid: 1000,
            },
            ui: Some(UiServiceDeclaration {
                name: "sync",
                upstream: "sync",
                transport: skillet_caddy::UpstreamTransport::Bridge,
                port: 8384,
                tailnet_identity: false,
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
            config: ServiceConfig::Pihole,
            ui: None,
        }],
    };
    assert_eq!(
        profile.ui_services(),
        Vec::<skillet_caddy::UiService>::new()
    );
    assert_eq!(
        profile.credential_consumers(),
        [
            super::CredentialConsumer {
                credential: skillet_pihole::dns::CREDENTIAL,
                unit: skillet_pihole::dns::UNIT,
            },
            super::CredentialConsumer {
                credential: super::PIHOLE_WEB_PASSWORD_CREDENTIAL,
                unit: "pihole.service",
            }
        ]
    );
}

#[test]
fn smtp_is_shared_and_has_an_independent_credential_phase() {
    for host in ["clamps", "joey"] {
        let profile = super::profile_for_name(host).unwrap();
        assert_eq!(
            profile
                .services
                .iter()
                .filter(|service| service.name() == "smtp")
                .count(),
            1
        );
        assert_eq!(
            super::credentials_for_phase(host, super::HostApplyPhase::Smtp).unwrap(),
            [skillet_smtp::CREDENTIAL]
        );
        assert!(
            !super::credentials_for_phase(host, super::HostApplyPhase::Full)
                .unwrap()
                .contains(&skillet_smtp::CREDENTIAL)
        );
    }
}

#[test]
fn tailscale_exit_node_is_caller_selected_and_preferences_follow_readiness() {
    assert!(super::profile_for_name("clamps")
        .unwrap()
        .tailscale_exit_node());
    assert!(!super::profile_for_name("agent")
        .unwrap()
        .tailscale_exit_node());
    for enabled in [false, true] {
        let config = tailscale_config(
            "fixture-host",
            "fixture-key".into(),
            "/var/lib/data/tailscale",
            enabled,
        );
        let container = &config.extra_config["Container"];
        assert!(container.contains(&format!(
            "Environment=TS_EXTRA_ARGS=--advertise-exit-node={enabled}"
        )));
        assert!(container.contains(&"Environment=TS_AUTH_ONCE=true".into()));
        assert!(container.contains(&"Notify=healthy".into()));
        assert!(container.contains(&"HealthCmd=tailscale status --peers=false".into()));
        assert!(config.extra_config["Service"].contains(&format!("ExecStartPost=/usr/bin/podman exec tailscale tailscale set --advertise-exit-node={enabled}")));
    }
}

#[test]
fn joey_boot_template_and_credential_unit_match_the_profile() {
    let template = include_str!("../../../../butane/joey.bu");
    let image = include_str!("../../../../butane/includes/ucore-joey.bu");
    let unit = include_str!("../../../../butane/includes/skillet-full.bu");
    let profile = super::profile_for_name("joey").unwrap();
    assert!(template.contains("device: /dev/vda"));
    for include in [
        "data-storage",
        "resolver",
        "skillet",
        "skillet-full",
        "skillet-joey",
        "ucore-bootstrap",
        "ucore-joey",
    ] {
        assert!(template.contains(&format!("local: includes/{include}.bu")));
    }
    assert!(image.contains(profile.signed_image.unwrap()));
    assert!(image.contains("name: systemd-resolved.service\n      mask: true"));
    for credential in super::credentials_for_phase("joey", super::HostApplyPhase::Full).unwrap() {
        assert!(unit.contains(&format!("LoadCredentialEncrypted={credential}:/etc/credstore.encrypted/skillet/{credential}.cred")));
    }
    assert!(unit.contains("apply --host-file /etc/skillet/host --phase full"));
    assert!(super::profile_for_name("beezelbot").is_none());
}

#[test]
fn joey_full_apply_uses_shared_services_and_is_repeatable() {
    use skillet_core::{
        system::{AccountResource, GroupIdentity, UserIdentity},
        test_utils::MockFiles,
    };
    use std::{path::Path, sync::atomic::Ordering};
    let system = MockSystem::new();
    system.user_identities.lock().unwrap().insert(
        "giacomo".into(),
        UserIdentity {
            name: "giacomo".into(),
            uid: 1042,
            primary_gid: 2047,
        },
    );
    system.group_identities.lock().unwrap().insert(
        "giacomo".into(),
        GroupIdentity {
            name: "giacomo".into(),
            gid: 2047,
        },
    );
    system.ensure_group("pihole", Some(2000)).unwrap();
    system
        .ensure_user("pihole", Some(2000), Some(2000))
        .unwrap();
    let files = MockFiles::new();
    files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
    files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
    for path in ["/etc/subuid", "/etc/subgid"] {
        files
            .files
            .lock()
            .unwrap()
            .insert(path.into(), b"giacomo:100000:65536\n".to_vec());
    }
    let mut credentials = CredentialInputs::default();
    credentials.insert(
        super::TAILSCALE_AUTH_KEY_CREDENTIAL,
        super::tailscale::EnrollmentInput::new("fixture-auth-key".into(), false)
            .payload()
            .unwrap(),
    );
    super::apply_host("joey", &system, &files, &credentials).unwrap();
    let restarts = system.restart_count.load(Ordering::SeqCst);
    let starts = system.start_count.load(Ordering::SeqCst);
    let before = files.files.lock().unwrap().clone();
    super::apply_host("joey", &system, &files, &credentials).unwrap();
    assert_eq!(*files.files.lock().unwrap(), before);
    assert_eq!(system.restart_count.load(Ordering::SeqCst), restarts);
    assert_eq!(system.start_count.load(Ordering::SeqCst), starts);
    let pihole =
        String::from_utf8(before["/etc/containers/systemd/pihole.container"].clone()).unwrap();
    assert!(pihole.contains("Network=joey-pihole.network"));
    assert!(pihole.contains("Environment=FTLCONF_webserver_api_password=\n"));
    assert!(!pihole.contains("WEBPASSWORD_FILE"));
    assert!(!pihole.contains("Secret="));
    assert!(!pihole.lines().any(|line| line == "Network=joey.network"));
    let syncthing = String::from_utf8_lossy(&before["/etc/containers/systemd/syncthing.container"]);
    assert!(syncthing.contains("Network=joey-syncthing.network"));
    assert!(!syncthing.lines().any(|line| line == "Network=joey.network"));
    assert!(!syncthing
        .lines()
        .any(|line| line.starts_with("PublishPort=") && line.contains("8384")));
    let network =
        String::from_utf8_lossy(&before["/etc/containers/systemd/joey-syncthing.network"]);
    assert!(network.contains("Options=isolate=strict"));
    assert!(network.contains("Internal=false"));
    for protocol in ["tcp", "udp"] {
        assert!(pihole.contains(&format!("PublishPort=53:53/{protocol}")));
    }
    let tailscale =
        String::from_utf8(before["/etc/containers/systemd/tailscale.container"].clone()).unwrap();
    assert!(tailscale.contains("Network=host"));
    assert!(tailscale.contains("--advertise-exit-node=false"));
    assert!(!before.contains_key("/etc/containers/systemd/unifi.container"));
    assert!(!before.contains_key("/etc/containers/systemd/datadog-agent.container"));
    assert!(!before.contains_key("/etc/containers/systemd/cloudflare-ddns.container"));
}

#[test]
fn every_syncthing_caller_has_matching_private_file_browsing() {
    for profile in super::declared_profiles() {
        let syncthing = profile
            .services
            .iter()
            .find(|service| matches!(service.config, ServiceConfig::Syncthing { .. }));
        let files = profile
            .services
            .iter()
            .find(|service| matches!(service.config, ServiceConfig::Mikochi { .. }));
        assert_eq!(syncthing.is_some(), files.is_some());
        if let Some(files) = files {
            let ui = files.ui.unwrap();
            assert_eq!(ui.name, "files");
            assert!(ui.tailnet_identity);
            assert_eq!(ui.upstream, "mikochi");
            let plan = profile.acceptance_plan();
            let acceptance = plan
                .services
                .iter()
                .find(|service| service.unit == "mikochi.service")
                .unwrap();
            assert_eq!(acceptance.health_probe, Some(crate::HealthProbe::Mikochi));
        }
    }
}

#[test]
fn ups_opt_in_is_hardware_scoped_without_vm_or_ui_requirements() {
    let clamps = super::profile_for_name("clamps").unwrap();
    assert!(clamps.supports_service("nut"));
    assert!(!super::profile_for_name("joey")
        .unwrap()
        .supports_service("nut"));
    let config = clamps
        .services
        .iter()
        .find_map(|service| {
            if let ServiceConfig::Nut(config) = service.config {
                Some(config)
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(
        (config.name, config.vendor, config.product),
        ("cyberpower", 0x0764, 0x0501)
    );
    assert!(!clamps.ui_services().iter().any(|ui| ui.name == "nut"));
    assert!(!clamps
        .acceptance_plan()
        .services
        .iter()
        .any(|service| service.unit.starts_with("nut-")));
    assert!(!clamps
        .credential_consumers()
        .iter()
        .any(|consumer| consumer.unit.starts_with("nut-")));
}

#[test]
fn nut_metrics_follow_the_datadog_phase_and_same_host_declaration() {
    use skillet_core::{files::FileReadResource as _, test_utils::MockFiles};
    use std::{path::Path, sync::atomic::Ordering};
    for attached in [false, true] {
        let files = MockFiles::new();
        files.record_btrfs_mount(Path::new("/var"), "/dev/test", "/", "btrfs");
        files.record_btrfs_mount(Path::new("/var/lib/data"), "/dev/test", "/data", "btrfs");
        let system = MockSystem::new();
        if attached {
            system.usb_devices.lock().unwrap().insert((0x0764, 0x0501));
        }
        let mut credentials = CredentialInputs::default();
        credentials.insert("datadog_config", r#"{"version":1,"api_key":"00000000000000000000000000000000","site":"datadoghq.com","tags":["env:test"]}"#.into());
        super::apply_host_phase(
            "clamps",
            super::HostApplyPhase::Datadog,
            &system,
            &files,
            &credentials,
        )
        .unwrap();
        assert!(files
            .read_file(Path::new("/etc/systemd/system/skillet-nut-metrics.timer"))
            .unwrap()
            .is_some());
        let config = files
            .read_file(Path::new("/etc/skillet/datadog/conf.d/systemd.d/conf.yaml"))
            .unwrap()
            .unwrap();
        let config = String::from_utf8(config).unwrap();
        assert!(config.contains("skillet-nut-metrics.timer"));
        assert_eq!(config.contains("nut-driver@cyberpower.service"), attached);
        let before = system.restart_count.load(Ordering::SeqCst);
        super::apply_host_phase(
            "clamps",
            super::HostApplyPhase::Datadog,
            &system,
            &files,
            &credentials,
        )
        .unwrap();
        assert_eq!(system.restart_count.load(Ordering::SeqCst), before);
    }
}

#[test]
fn native_networking_is_separate_from_full_apply_and_container_acceptance() {
    use super::{credentials_for_phase, HostApplyPhase};
    assert_eq!(
        credentials_for_phase("clamps", HostApplyPhase::Network).unwrap(),
        ["network_config"]
    );
    assert!(!credentials_for_phase("clamps", HostApplyPhase::Full)
        .unwrap()
        .contains(&"network_config"));
    assert!(super::profile_for_name("joey")
        .unwrap()
        .host_network_selection()
        .is_none());
    assert!(super::profile_for_name("clamps")
        .unwrap()
        .acceptance_plan()
        .services
        .iter()
        .all(|service| service.unit != "skillet-network-prepare.service"));
    let files = skillet_core::test_utils::MockFiles::new();
    assert!(super::apply_host_phase(
        "clamps",
        HostApplyPhase::Network,
        &MockSystem::new(),
        &files,
        &CredentialInputs::default()
    )
    .is_err());
    assert!(files.files.lock().unwrap().is_empty());
}

#[test]
fn syncthing_canonical_and_alias_use_the_identity_gate_and_private_backend() {
    for profile in super::declared_profiles() {
        let Some(service) = profile
            .services
            .iter()
            .find(|service| matches!(service.config, ServiceConfig::Syncthing { .. }))
        else {
            continue;
        };
        let ui = service.ui.unwrap();
        assert!(ui.tailnet_identity);
        let upstreams = profile.ui_services();
        let mut sites = skillet_caddy::CaddySites::from_host_with_zone(
            profile.id.as_str(),
            &skillet_caddy::UiEnvironment {
                ui_domain: "ui.example.invalid".into(),
                acme_staging: true,
            },
            &upstreams,
            Some("example.invalid"),
        )
        .unwrap();
        sites.tailnet_users = vec!["alice@example.invalid".into()];
        let site = sites
            .services
            .iter()
            .find(|site| site.name == "syncthing")
            .unwrap();
        for hostname in std::iter::once(&site.hostname).chain(&site.aliases) {
            let section = sites
                .render()
                .split(&format!("\n{hostname} {{"))
                .nth(1)
                .unwrap()
                .split("\n}\n")
                .next()
                .unwrap()
                .to_owned();
            assert!(section.contains("forward_auth unix//run/skillet-tailnet-ui/auth.sock"));
            assert!(section.find("forward_auth").unwrap() < section.find("reverse_proxy").unwrap());
            assert!(section.contains("request_header -X-Skillet-User"));
        }
        let expected = skillet_caddy::private_network(profile.id.as_str(), ui.name).unit_name;
        let plan = profile.acceptance_plan();
        assert_eq!(
            plan.services
                .iter()
                .find(|service| service.unit == "syncthing.service")
                .unwrap()
                .network_mode
                .as_deref(),
            Some(expected.as_str())
        );
        let mut stale = sites.clone();
        stale
            .services
            .iter_mut()
            .find(|site| site.name == "syncthing")
            .unwrap()
            .tailnet_identity = false;
        assert!(skillet_caddy::CaddySites::parse(
            &serde_json::to_string(&stale).unwrap(),
            profile.id.as_str(),
            &upstreams
        )
        .is_err());
    }
}

#[test]
fn homeassistant_exports_share_the_declared_syncthing_identity_and_file_scope() {
    let profile = super::profile_for_name("clamps").unwrap();
    let backup = profile
        .services
        .iter()
        .find_map(|service| match service.config {
            ServiceConfig::HomeAssistant(config) => Some(config),
            _ => None,
        })
        .unwrap();
    let syncthing = profile
        .services
        .iter()
        .find_map(|service| match service.config {
            ServiceConfig::Syncthing {
                files_path,
                data_path,
                data_owner,
                data_group,
                ..
            } => Some((files_path, data_path, data_owner, data_group)),
            _ => None,
        })
        .unwrap();
    assert_eq!(backup.files_root, syncthing.0);
    assert_eq!(backup.backup_subvolume, syncthing.1);
    assert_eq!(backup.owner, syncthing.2);
    assert_eq!(backup.group, syncthing.3);
    let other = super::profile_for_name("joey").unwrap();
    assert!(!other.supports_service("homeassistant"));
}
