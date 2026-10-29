//! Validated host identity and the single source of host capabilities.
use std::{borrow::Cow, path::PathBuf};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct HostId(Cow<'static, str>);

impl HostId {
    pub fn parse(value: &str) -> Option<Self> {
        let valid = !value.is_empty()
            && value.len() <= 63
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            && value.as_bytes()[0].is_ascii_lowercase()
            && value.as_bytes()[value.len() - 1].is_ascii_alphanumeric();
        valid.then(|| Self(Cow::Owned(value.to_string())))
    }

    fn known(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SharingDeclaration {
    pub names: &'static [&'static str],
    pub policy: skillet_caddy::sharing::Policy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UiServiceDeclaration {
    pub sharing: Option<SharingDeclaration>,
    /// Override the service's shared dashboard group on this host.
    pub dashboard_group: Option<&'static str>,
    pub name: &'static str,
    pub upstream: &'static str,
    pub transport: skillet_caddy::UpstreamTransport,
    pub port: u16,
    pub tailnet_identity: bool,
    pub aliases: &'static [&'static str],
    pub public_names: &'static [&'static str],
    pub cache: Option<skillet_caddy::cache::Policy>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialConsumer {
    pub credential: &'static str,
    pub unit: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceConfig {
    Pihole,
    Syncthing {
        files_path: &'static str,
        data_path: &'static str,
        data_owner: &'static str,
        data_group: &'static str,
        container_uid: u32,
        container_gid: u32,
    },
    Mikochi {
        files_path: &'static str,
        data_owner: &'static str,
        data_group: &'static str,
        container_uid: u32,
        container_gid: u32,
    },
    Homer,
    Joplin(skillet_joplin::Config),
    Jellyfin(skillet_jellyfin::Config),
    Plex(skillet_plex::Config),
    Gokapi(skillet_gokapi::Config),
    Airtrail(skillet_airtrail::Config),
    Media {
        app: skillet_media::App,
        config: skillet_media::Config,
    },
    Nut(skillet_nut::Config),
    Unifi,
    HomeAssistant(skillet_homeassistant::Config),
    Smtp,
    Network(skillet_network::Selection),
    Ddns {
        production_enabled: bool,
    },
    Datadog {
        production_region: &'static str,
        network_monitoring: bool,
    },
    Tailscale {
        advertise_exit_node: bool,
        state_path: &'static str,
    },
    Btrbk {
        snapshot_subvolumes: &'static [&'static str],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostService {
    pub config: ServiceConfig,
    pub ui: Option<UiServiceDeclaration>,
}

impl HostService {
    pub fn dashboard_group(&self) -> &'static str {
        self.ui
            .and_then(|ui| ui.dashboard_group)
            .unwrap_or(match self.config {
                ServiceConfig::Media { .. }
                | ServiceConfig::Jellyfin(_)
                | ServiceConfig::Plex(_) => skillet_homer::MEDIA,
                ServiceConfig::Joplin(_) | ServiceConfig::Airtrail(_) => {
                    skillet_homer::APPLICATIONS
                }
                ServiceConfig::Gokapi(_)
                | ServiceConfig::Syncthing { .. }
                | ServiceConfig::Mikochi { .. } => skillet_homer::FILES,
                _ => skillet_homer::HOME_INFRA,
            })
    }

    fn supports(&self, name: &str) -> bool {
        self.name() == name
            || (name == "media" && matches!(self.config, ServiceConfig::Media { .. }))
    }
    pub fn name(&self) -> &'static str {
        match self.config {
            ServiceConfig::Pihole => "pihole",
            ServiceConfig::Syncthing { .. } => "syncthing",
            ServiceConfig::Mikochi { .. } => "mikochi",
            ServiceConfig::Homer => "homer",
            ServiceConfig::Joplin(_) => "joplin",
            ServiceConfig::Jellyfin(_) => "jellyfin",
            ServiceConfig::Plex(_) => "plex",
            ServiceConfig::Gokapi(_) => "gokapi",
            ServiceConfig::Airtrail(_) => "airtrail",
            ServiceConfig::Media { app, .. } => app.name(),
            ServiceConfig::Nut(_) => "nut",
            ServiceConfig::Unifi => "unifi",
            ServiceConfig::HomeAssistant(_) => "homeassistant",
            ServiceConfig::Ddns { .. } => "ddns",
            ServiceConfig::Datadog { .. } => "datadog",
            ServiceConfig::Smtp => "smtp",
            ServiceConfig::Network(_) => "network",
            ServiceConfig::Tailscale { .. } => "tailscale",
            ServiceConfig::Btrbk { .. } => "btrbk",
        }
    }

    fn backend_network(&self, host: &str) -> String {
        self.ui.filter(|ui| ui.tailnet_identity).map_or_else(
            || host.into(),
            |ui| skillet_caddy::private_network(host, ui.name).unit_name,
        )
    }

    pub fn credential_consumers(&self) -> Vec<CredentialConsumer> {
        match self.config {
            ServiceConfig::Media {
                app: skillet_media::App::Prowlarr,
                ..
            } => vec![CredentialConsumer {
                credential: skillet_media::CREDENTIAL,
                unit: skillet_media::APPLY_UNIT,
            }],
            ServiceConfig::HomeAssistant(_) => vec![CredentialConsumer {
                credential: skillet_homeassistant::backup::CREDENTIAL,
                unit: skillet_homeassistant::backup::UNIT,
            }],
            ServiceConfig::Syncthing { .. } => vec![CredentialConsumer {
                credential: skillet_syncthing::initial::CREDENTIAL,
                unit: skillet_syncthing::initial::UNIT,
            }],
            ServiceConfig::Gokapi(_) => vec![CredentialConsumer {
                credential: skillet_gokapi::ADMIN_CREDENTIAL,
                unit: skillet_gokapi::APPLY_UNIT,
            }],
            ServiceConfig::Joplin(_) => vec![CredentialConsumer {
                credential: skillet_joplin::DATABASE_PASSWORD_CREDENTIAL,
                unit: "skillet-joplin-apply.service",
            }],
            ServiceConfig::Airtrail(_) => vec![
                CredentialConsumer {
                    credential: skillet_airtrail::DATABASE_PASSWORD_CREDENTIAL,
                    unit: "skillet-airtrail-apply.service",
                },
                CredentialConsumer {
                    credential: skillet_airtrail::INTEGRATIONS_CREDENTIAL,
                    unit: "skillet-airtrail-apply.service",
                },
            ],
            ServiceConfig::Network(_) => vec![CredentialConsumer {
                credential: skillet_network::CREDENTIAL,
                unit: skillet_network::PREPARE_UNIT,
            }],
            ServiceConfig::Smtp => vec![CredentialConsumer {
                credential: skillet_smtp::CREDENTIAL,
                unit: "postfix.service",
            }],
            ServiceConfig::Datadog { .. } => vec![CredentialConsumer {
                credential: skillet_datadog::CREDENTIAL,
                unit: "datadog-agent.service",
            }],
            ServiceConfig::Ddns { .. } => vec![CredentialConsumer {
                credential: super::CLOUDFLARE_DDNS_CONFIG_CREDENTIAL,
                unit: "cloudflare-ddns.service",
            }],
            ServiceConfig::Pihole => {
                let mut consumers = vec![CredentialConsumer {
                    credential: skillet_pihole::dns::CREDENTIAL,
                    unit: skillet_pihole::dns::UNIT,
                }];
                if !self.ui.is_some_and(|ui| ui.tailnet_identity) {
                    consumers.push(CredentialConsumer {
                        credential: super::PIHOLE_WEB_PASSWORD_CREDENTIAL,
                        unit: "pihole.service",
                    });
                }
                consumers
            }
            ServiceConfig::Tailscale { .. } => vec![CredentialConsumer {
                credential: super::TAILSCALE_AUTH_KEY_CREDENTIAL,
                unit: "tailscale.service",
            }],
            _ => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkPolicy {
    pub ipv4_subnet: &'static str,
    pub ipv4_gateway: &'static str,
    pub ipv6_subnet: &'static str,
    pub ipv6_gateway: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostProfile {
    pub id: HostId,
    pub signed_image: Option<&'static str>,
    pub masked_units: &'static [&'static str],
    pub network: NetworkPolicy,
    pub requires_data_mount: bool,
    pub services: Vec<HostService>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAcceptancePlan {
    pub requires_data_mount: bool,
    pub services: Vec<AcceptanceService>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptanceService {
    pub unit: String,
    pub container: Option<String>,
    pub network_mode: Option<String>,
    pub bind_paths: Vec<String>,
    pub owner: Option<AcceptanceOwner>,
    pub listeners: Vec<AcceptanceListener>,
    pub health_probe: Option<HealthProbe>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceptanceOwner {
    Named { user: String, group: String },
    Numeric { uid: u32, gid: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptanceListener {
    pub port: u16,
    pub protocol: ListenerProtocol,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListenerProtocol {
    Tcp,
    Udp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HealthProbe {
    Gokapi,
    Plex,
    Jellyfin,
    Media,
    Homeassistant,
    Mikochi,
    Pihole,
    Tailscale,
}

impl HostProfile {
    pub fn dashboard_group(&self, ui_name: &str) -> &'static str {
        self.services
            .iter()
            .find(|service| {
                service.ui.is_some_and(|ui| {
                    ui.name == ui_name
                        || (ui.sharing.is_some()
                            && skillet_caddy::sharing::site_name(ui.name) == ui_name)
                })
            })
            .map_or("Services", HostService::dashboard_group)
    }

    pub fn tailscale_exit_node(&self) -> bool {
        self.services.iter().any(|service| {
            matches!(
                service.config,
                ServiceConfig::Tailscale {
                    advertise_exit_node: true,
                    ..
                }
            )
        })
    }

    /// Native interfaces selected by this caller; independent of the Podman network.
    pub fn host_network_selection(&self) -> Option<skillet_network::Selection> {
        self.services
            .iter()
            .find_map(|service| match service.config {
                ServiceConfig::Network(selection) => Some(selection),
                _ => None,
            })
    }

    /// Runtime checks follow the services and UI capabilities declared by
    /// this profile, so fixture acceptance cannot stand in for application
    /// acceptance.
    pub fn acceptance_plan(&self) -> HostAcceptancePlan {
        self.acceptance_plan_with_ddns(false)
    }

    /// Public-address updates require a separate opt-in acceptance scenario;
    /// ordinary application checks must not start an external DNS writer.
    pub fn acceptance_plan_with_ddns(&self, with_ddns: bool) -> HostAcceptancePlan {
        let mut services = self
            .services
            .iter()
            .filter(|service| {
                !matches!(
                    service.config,
                    ServiceConfig::Datadog { .. }
                        | ServiceConfig::Smtp
                        | ServiceConfig::Nut(_)
                        | ServiceConfig::Network(_)
                ) && (with_ddns || !matches!(service.config, ServiceConfig::Ddns { .. }))
            })
            .flat_map(|service| {
                let mut declared = vec![acceptance_service(self.id.as_str(), service)];
                let database = match service.config {
                    ServiceConfig::Joplin(config) => Some(config.postgres()),
                    ServiceConfig::Airtrail(config) => Some(config.postgres()),
                    _ => None,
                };
                if let Some(database) = database {
                    declared.push(AcceptanceService {
                        unit: database.unit_name(),
                        container: Some(database.container_name()),
                        network_mode: Some(database.network_name.into()),
                        bind_paths: vec![database.data_path.into()],
                        owner: None,
                        listeners: Vec::new(),
                        health_probe: None,
                    });
                }
                declared
            })
            .collect::<Vec<_>>();
        if self.services.iter().any(|service| service.ui.is_some()) {
            services.push(caddy_acceptance(
                self.id.as_str(),
                self.supports_service("homer"),
            ));
            if self.ui_services().iter().any(|ui| ui.tailnet_identity) {
                services.push(AcceptanceService {
                    unit: "skillet-tailnet-ui.service".into(),
                    container: None,
                    network_mode: None,
                    bind_paths: Vec::new(),
                    owner: None,
                    listeners: Vec::new(),
                    health_probe: None,
                });
            }
        }
        HostAcceptancePlan {
            requires_data_mount: self.requires_data_mount,
            services,
        }
    }

    pub fn supports_service(&self, name: &str) -> bool {
        self.services.iter().any(|service| service.supports(name))
    }

    pub fn media_config(&self) -> Option<skillet_media::Config> {
        self.services
            .iter()
            .find_map(|service| match service.config {
                ServiceConfig::Media { config, .. } => Some(config),
                _ => None,
            })
    }

    /// A declared service can remain available to disposable test scenarios
    /// while its production writer is held until an explicit cutover.
    pub fn supports_production_service(&self, name: &str) -> bool {
        self.services.iter().any(|service| {
            service.supports(name)
                && !matches!(
                    service.config,
                    ServiceConfig::Ddns {
                        production_enabled: false
                    }
                )
        })
    }

    pub fn requires_pihole_dns_listener_policy(&self) -> bool {
        self.services
            .iter()
            .any(|service| matches!(service.config, ServiceConfig::Pihole))
    }

    pub fn requires_service_network(&self) -> bool {
        self.services.iter().any(|service| {
            matches!(
                service.config,
                ServiceConfig::Pihole
                    | ServiceConfig::Syncthing { .. }
                    | ServiceConfig::Ddns { .. }
            ) || service.ui.is_some()
        })
    }

    pub fn requires_full_apply_credentials(&self) -> bool {
        self.services.iter().any(|service| {
            service.credential_consumers().iter().any(|consumer| {
                matches!(
                    consumer.credential,
                    super::PIHOLE_WEB_PASSWORD_CREDENTIAL | super::TAILSCALE_AUTH_KEY_CREDENTIAL
                )
            })
        })
    }

    pub fn ui_services(&self) -> Vec<skillet_caddy::UiService> {
        self.services
            .iter()
            .filter_map(|service| service.ui.as_ref().map(|ui| (service, ui)))
            .map(|(service, ui)| skillet_caddy::UiService {
                name: ui.name.to_string(),
                cache: ui.cache,
                upstream: ui.upstream.to_string(),
                transport: ui.transport,
                port: ui.port,
                tailnet_identity: ui.tailnet_identity,
                anonymous_token_refresh: matches!(service.config, ServiceConfig::Mikochi { .. }),
                sharing: ui
                    .sharing
                    .map(|sharing| skillet_caddy::sharing::Declaration {
                        names: sharing.names.iter().map(|name| (*name).into()).collect(),
                        policy: sharing.policy,
                    }),
                public_names: ui
                    .public_names
                    .iter()
                    .map(|name| (*name).to_string())
                    .collect(),
                aliases: ui
                    .aliases
                    .iter()
                    .map(|alias| (*alias).to_string())
                    .collect(),
            })
            .collect()
    }

    pub fn credential_consumers(&self) -> Vec<CredentialConsumer> {
        let mut consumers = self
            .services
            .iter()
            .flat_map(HostService::credential_consumers)
            .collect::<Vec<_>>();
        if self.services.iter().any(|service| service.ui.is_some()) {
            consumers.extend([
                CredentialConsumer {
                    credential: super::CADDY_SITES_CREDENTIAL,
                    unit: "caddy.service",
                },
                CredentialConsumer {
                    credential: super::CLOUDFLARE_ACME_TOKEN_CREDENTIAL,
                    unit: "caddy.service",
                },
            ]);
        }
        consumers
    }

    pub fn service_network(&self) -> skillet_podman::PodmanNetwork {
        let host = self.id.as_str();
        skillet_podman::PodmanNetwork {
            unit_name: host.to_string(),
            options: vec![
                "DisableDNS=false".to_string(),
                "Driver=bridge".to_string(),
                format!("Gateway={}", self.network.ipv4_gateway),
                format!("Gateway={}", self.network.ipv6_gateway),
                "IPv6=true".to_string(),
                format!("NetworkName={host}"),
                format!("Subnet={}", self.network.ipv4_subnet),
                format!("Subnet={}", self.network.ipv6_subnet),
            ],
        }
    }

    pub fn btrbk_config(&self) -> Option<skillet_btrbk::BtrbkConfig> {
        self.services
            .iter()
            .find_map(|service| match service.config {
                ServiceConfig::Btrbk {
                    snapshot_subvolumes,
                } => Some(skillet_btrbk::BtrbkConfig {
                    snapshot_subvolumes: snapshot_subvolumes
                        .iter()
                        .map(|subvolume| PathBuf::from(*subvolume))
                        .collect(),
                }),
                _ => None,
            })
    }
}

fn acceptance_service(profile: &str, service: &HostService) -> AcceptanceService {
    match service.config {
        ServiceConfig::HomeAssistant(_) => homeassistant_acceptance(),
        ServiceConfig::Joplin(config) => joplin_acceptance(config),
        ServiceConfig::Jellyfin(config) => {
            jellyfin_acceptance(config, service.backend_network(profile))
        }
        ServiceConfig::Gokapi(config) => {
            gokapi_acceptance(config, service.backend_network(profile))
        }
        ServiceConfig::Plex(config) => plex_acceptance(config, service.backend_network(profile)),
        ServiceConfig::Airtrail(config) => airtrail_acceptance(config),
        ServiceConfig::Media { app, config } => media_acceptance(app, config),
        ServiceConfig::Nut(_) => nut_acceptance(),
        ServiceConfig::Network(_) => native_network_acceptance(),
        ServiceConfig::Smtp => smtp_acceptance(),
        ServiceConfig::Datadog { .. } => datadog_acceptance(),
        ServiceConfig::Ddns { .. } => AcceptanceService {
            unit: "cloudflare-ddns.service".into(),
            container: Some("cloudflare-ddns".into()),
            network_mode: Some(profile.into()),
            bind_paths: Vec::new(),
            owner: None,
            listeners: Vec::new(),
            health_probe: None,
        },
        ServiceConfig::Pihole => AcceptanceService {
            unit: "pihole.service".into(),
            container: Some("pihole".into()),
            network_mode: Some(service.backend_network(profile)),
            bind_paths: vec![
                "/var/lib/data/pihole/etc".into(),
                "/var/lib/data/pihole/log".into(),
            ],
            // Pi-hole may change ownership of its writable bind directories
            // during startup. The host profile does not declare numeric
            // ownership for these paths, so acceptance validates their mounts
            // and runtime without inventing an owner contract.
            owner: None,
            listeners: [ListenerProtocol::Tcp, ListenerProtocol::Udp]
                .map(|protocol| AcceptanceListener { port: 53, protocol })
                .into(),
            health_probe: Some(HealthProbe::Pihole),
        },
        ServiceConfig::Syncthing {
            data_path,
            data_owner,
            data_group,
            ..
        } => AcceptanceService {
            unit: "syncthing.service".into(),
            container: Some("syncthing".into()),
            network_mode: Some(service.backend_network(profile)),
            bind_paths: vec![data_path.into()],
            owner: Some(AcceptanceOwner::Named {
                user: data_owner.into(),
                group: data_group.into(),
            }),
            listeners: [ListenerProtocol::Tcp, ListenerProtocol::Udp]
                .map(|protocol| AcceptanceListener {
                    port: 22000,
                    protocol,
                })
                .into(),
            health_probe: None,
        },
        ServiceConfig::Mikochi {
            files_path,
            data_owner,
            data_group,
            ..
        } => AcceptanceService {
            unit: "mikochi.service".into(),
            container: Some("mikochi".into()),
            network_mode: Some(format!("{profile}-files")),
            bind_paths: vec![files_path.into()],
            owner: Some(AcceptanceOwner::Named {
                user: data_owner.into(),
                group: data_group.into(),
            }),
            listeners: Vec::new(),
            health_probe: Some(HealthProbe::Mikochi),
        },
        ServiceConfig::Homer => homer_acceptance(profile),
        ServiceConfig::Unifi => unifi_acceptance(),
        ServiceConfig::Tailscale { state_path, .. } => AcceptanceService {
            unit: "tailscale.service".into(),
            container: Some("tailscale".into()),
            network_mode: Some("host".into()),
            bind_paths: vec![state_path.into()],
            owner: None,
            listeners: Vec::new(),
            health_probe: Some(HealthProbe::Tailscale),
        },
        ServiceConfig::Btrbk { .. } => AcceptanceService {
            unit: "skillet-btrbk.timer".into(),
            container: None,
            network_mode: None,
            bind_paths: Vec::new(),
            owner: None,
            listeners: Vec::new(),
            health_probe: None,
        },
    }
}

fn media_acceptance(app: skillet_media::App, config: skillet_media::Config) -> AcceptanceService {
    let mut bind_paths = Vec::new();
    if app != skillet_media::App::Putioarr {
        bind_paths.push(config.config_path(app).to_string_lossy().into_owned());
    }
    if app != skillet_media::App::Prowlarr {
        bind_paths.push(config.files_path.into());
    }
    if app == skillet_media::App::Lidarr {
        bind_paths.push(config.music_path.into());
    }
    AcceptanceService {
        unit: format!("{}.service", app.name()),
        container: Some(app.name().into()),
        network_mode: Some(config.network.into()),
        bind_paths,
        owner: Some(AcceptanceOwner::Named {
            user: config.data_owner.into(),
            group: config.data_group.into(),
        }),
        listeners: Vec::new(),
        health_probe: (app == skillet_media::App::Prowlarr).then_some(HealthProbe::Media),
    }
}

fn airtrail_acceptance(config: skillet_airtrail::Config) -> AcceptanceService {
    AcceptanceService {
        unit: "airtrail.service".into(),
        container: Some("airtrail".into()),
        network_mode: Some(config.network_name.into()),
        bind_paths: vec![config.uploads_path.into()],
        owner: None,
        listeners: Vec::new(),
        health_probe: None,
    }
}

fn joplin_acceptance(config: skillet_joplin::Config) -> AcceptanceService {
    AcceptanceService {
        unit: "joplin-server.service".into(),
        container: Some("joplin-server".into()),
        network_mode: Some(config.network_name.into()),
        bind_paths: Vec::new(),
        owner: None,
        listeners: Vec::new(),
        health_probe: None,
    }
}

fn homer_acceptance(profile: &str) -> AcceptanceService {
    AcceptanceService {
        unit: "homer.service".into(),
        container: Some("homer".into()),
        network_mode: Some(format!("{profile}-homer")),
        bind_paths: Vec::new(),
        owner: None,
        listeners: Vec::new(),
        health_probe: None,
    }
}

fn native_network_acceptance() -> AcceptanceService {
    AcceptanceService {
        unit: skillet_network::PREPARE_UNIT.into(),
        container: None,
        network_mode: None,
        bind_paths: Vec::new(),
        owner: None,
        listeners: Vec::new(),
        health_probe: None,
    }
}

fn caddy_acceptance(profile: &str, has_homer: bool) -> AcceptanceService {
    let mut bind_paths = vec!["/var/lib/data/caddy/data".into()];
    if has_homer {
        bind_paths.push("/etc/skillet/homer".into());
    }
    AcceptanceService {
        unit: "caddy.service".into(),
        container: Some("caddy".into()),
        network_mode: Some(profile.into()),
        bind_paths,
        owner: None,
        listeners: vec![AcceptanceListener {
            port: 443,
            protocol: ListenerProtocol::Tcp,
        }],
        health_probe: None,
    }
}

const PIHOLE_UI: UiServiceDeclaration = UiServiceDeclaration {
    dashboard_group: None,
    sharing: None,
    public_names: &[],
    cache: None,
    name: "pihole",
    upstream: "pihole",
    transport: skillet_caddy::UpstreamTransport::Bridge,
    port: 8088,
    tailnet_identity: true,
    aliases: &[],
};

const SYNCTHING_UI: UiServiceDeclaration = UiServiceDeclaration {
    dashboard_group: None,
    sharing: None,
    public_names: &[],
    cache: None,
    name: "syncthing",
    upstream: "syncthing",
    transport: skillet_caddy::UpstreamTransport::Bridge,
    port: 8384,
    tailnet_identity: true,
    aliases: &["sync.{host}"],
};

fn clamps() -> HostProfile {
    HostProfile {
        id: HostId::known("clamps"),
        signed_image: Some("ghcr.io/gbagnoli/ucore-clamps"),
        masked_units: &["systemd-resolved.service"],
        network: NetworkPolicy {
            ipv4_subnet: "172.26.26.0/24",
            ipv4_gateway: "172.26.26.1",
            ipv6_subnet: "fd59:4e23:2950:11f5::/64",
            ipv6_gateway: "fd59:4e23:2950:11f5::1",
        },
        requires_data_mount: true,
        services: vec![
            HostService {
                config: ServiceConfig::Network(skillet_network::Selection {
                    wired: true,
                    wifi: true,
                }),
                ui: None,
            },
            HostService {
                config: ServiceConfig::Pihole,
                ui: Some(PIHOLE_UI),
            },
            HostService {
                config: ServiceConfig::Tailscale {
                    advertise_exit_node: true,
                    state_path: "/var/lib/data/tailscale",
                },
                ui: None,
            },
            HostService {
                config: ServiceConfig::Syncthing {
                    files_path: "/var/lib/data/syncthing/data",
                    data_path: "/var/lib/data/syncthing",
                    data_owner: "giacomo",
                    data_group: "giacomo",
                    container_uid: 1000,
                    container_gid: 1000,
                },
                ui: Some(SYNCTHING_UI),
            },
            HostService {
                config: ServiceConfig::Nut(skillet_nut::Config {
                    name: "cyberpower",
                    vendor: 0x0764,
                    product: 0x0501,
                    description: "CyberPower CP1500 AVR",
                }),
                ui: None,
            },
            HostService {
                config: ServiceConfig::HomeAssistant(skillet_homeassistant::Config {
                    bluetooth: true,
                    backup_subvolume: "/var/lib/data/syncthing",
                    files_root: "/var/lib/data/syncthing/data",
                    backup_files: "/var/lib/data/syncthing/data/backups/homeassistant",
                    owner: "giacomo",
                    group: "giacomo",
                }),
                ui: Some(UiServiceDeclaration {
                    dashboard_group: None,
                    sharing: None,
                    public_names: &[],
                    cache: None,
                    name: "homeassistant",
                    upstream: "homeassistant",
                    transport: skillet_caddy::UpstreamTransport::HostLoopback,
                    port: 8123,
                    tailnet_identity: true,
                    aliases: &[],
                }),
            },
            HostService {
                config: ServiceConfig::Unifi,
                ui: None,
            },
            HostService {
                // Rupik remains the production DDNS writer until cutover.
                config: ServiceConfig::Ddns {
                    production_enabled: false,
                },
                ui: None,
            },
            HostService {
                config: ServiceConfig::Datadog {
                    production_region: "ftwo",
                    network_monitoring: true,
                },
                ui: None,
            },
            HostService {
                config: ServiceConfig::Btrbk {
                    snapshot_subvolumes: &["syncthing"],
                },
                ui: None,
            },
        ],
    }
}

/// Boxy's replacement. Private DNS and routing inputs are delivered separately.
fn joey() -> HostProfile {
    HostProfile {
        id: HostId::known("joey"),
        signed_image: Some("ghcr.io/gbagnoli/ucore-joey"),
        masked_units: &["systemd-resolved.service"],
        network: NetworkPolicy {
            ipv4_subnet: "172.26.26.0/24",
            ipv4_gateway: "172.26.26.1",
            ipv6_subnet: "fd59:4e23:2950:11f5::/64",
            ipv6_gateway: "fd59:4e23:2950:11f5::1",
        },
        requires_data_mount: true,
        services: vec![
            HostService {
                config: ServiceConfig::Pihole,
                ui: Some(PIHOLE_UI),
            },
            HostService {
                config: ServiceConfig::Tailscale {
                    advertise_exit_node: true,
                    state_path: "/var/lib/data/tailscale",
                },
                ui: None,
            },
            HostService {
                config: ServiceConfig::Syncthing {
                    files_path: "/var/lib/data/syncthing/data",
                    data_path: "/var/lib/data/syncthing",
                    data_owner: "giacomo",
                    data_group: "giacomo",
                    container_uid: 1000,
                    container_gid: 1000,
                },
                ui: Some(SYNCTHING_UI),
            },
            HostService {
                config: ServiceConfig::Ddns {
                    production_enabled: true,
                },
                ui: None,
            },
            HostService {
                config: ServiceConfig::Datadog {
                    production_region: "ftwo",
                    network_monitoring: true,
                },
                ui: None,
            },
            HostService {
                config: ServiceConfig::Btrbk {
                    snapshot_subvolumes: &["syncthing"],
                },
                ui: None,
            },
        ],
    }
}

/// Donbot is a fresh installation replacing Calculon, with the shared fleet
/// baseline and private application services.
fn donbot() -> HostProfile {
    let mut profile = HostProfile {
        id: HostId::known("donbot"),
        signed_image: Some("ghcr.io/gbagnoli/ucore-donbot"),
        masked_units: &["systemd-resolved.service"],
        network: NetworkPolicy {
            ipv4_subnet: "172.27.27.0/24",
            ipv4_gateway: "172.27.27.1",
            ipv6_subnet: "fd59:4e23:2950:12f5::/64",
            ipv6_gateway: "fd59:4e23:2950:12f5::1",
        },
        requires_data_mount: true,
        services: vec![
            HostService {
                config: ServiceConfig::Pihole,
                ui: Some(PIHOLE_UI),
            },
            HostService {
                config: ServiceConfig::Tailscale {
                    advertise_exit_node: true,
                    state_path: "/var/lib/data/tailscale",
                },
                ui: None,
            },
            HostService {
                config: ServiceConfig::Syncthing {
                    files_path: "/var/lib/data/syncthing/data",
                    data_path: "/var/lib/data/syncthing",
                    data_owner: "giacomo",
                    data_group: "giacomo",
                    container_uid: 1000,
                    container_gid: 1000,
                },
                ui: Some(SYNCTHING_UI),
            },
            HostService {
                config: ServiceConfig::Joplin(skillet_joplin::Config {
                    image: skillet_joplin::DEFAULT_IMAGE,
                    network_name: "donbot-joplin",
                    database_path: "/var/lib/data/joplin/postgres",
                    postgres_data_container_path: "/var/lib/postgresql",
                    backup_path: "/var/lib/data/syncthing/data/backups/postgres/joplin",
                    backup_subvolume: "/var/lib/data/syncthing",
                    backup_owner: "giacomo",
                    backup_group: "giacomo",
                }),
                ui: Some(UiServiceDeclaration {
                    dashboard_group: None,
                    sharing: None,
                    public_names: &["notes"],
                    cache: None,
                    name: "joplin",
                    upstream: "joplin-server",
                    transport: skillet_caddy::UpstreamTransport::Bridge,
                    port: skillet_joplin::PORT,
                    tailnet_identity: false,
                    aliases: &[],
                }),
            },
            HostService {
                config: ServiceConfig::Airtrail(skillet_airtrail::Config {
                    image: skillet_airtrail::DEFAULT_IMAGE,
                    network_name: "donbot-airtrail",
                    database_path: "/var/lib/data/airtrail/postgres",
                    postgres_data_container_path: "/var/lib/postgresql",
                    uploads_path: "/var/lib/data/airtrail/uploads",
                    backup_path: "/var/lib/data/syncthing/data/backups/airtrail",
                    backup_subvolume: "/var/lib/data/syncthing",
                    backup_owner: "giacomo",
                    backup_group: "giacomo",
                }),
                ui: Some(UiServiceDeclaration {
                    dashboard_group: None,
                    sharing: None,
                    public_names: &[],
                    cache: None,
                    name: "airtrail",
                    upstream: "airtrail",
                    transport: skillet_caddy::UpstreamTransport::Bridge,
                    port: skillet_airtrail::PORT,
                    tailnet_identity: true,
                    aliases: &[],
                }),
            },
            HostService {
                config: ServiceConfig::Datadog {
                    production_region: "oneprovider",
                    network_monitoring: true,
                },
                ui: None,
            },
            HostService {
                config: ServiceConfig::Btrbk {
                    snapshot_subvolumes: &["syncthing"],
                },
                ui: None,
            },
        ],
    };
    profile.services.extend(donbot_media_services());
    profile
}

fn donbot_media_services() -> Vec<HostService> {
    let config = skillet_media::Config {
        config_root: "/var/lib/data/media/config",
        files_path: "/var/lib/data/media/files",
        music_path: "/var/lib/data/syncthing/data/music",
        network: "donbot-media",
        data_owner: "giacomo",
        data_group: "giacomo",
    };
    let mut services = vec![HostService {
        config: ServiceConfig::Jellyfin(skillet_jellyfin::Config {
            state_path: "/var/lib/data/jellyfin/config",
            cache_path: "/var/lib/data/jellyfin/cache",
            files_path: config.files_path,
            music_path: config.music_path,
            data_owner: config.data_owner,
            data_group: config.data_group,
        }),
        ui: Some(UiServiceDeclaration {
            dashboard_group: None,
            sharing: None,
            name: "jellyfin",
            upstream: "jellyfin",
            port: skillet_jellyfin::PORT,
            transport: skillet_caddy::UpstreamTransport::Bridge,
            public_names: &["video", "media"],
            aliases: &[],
            tailnet_identity: false,
            cache: Some(skillet_caddy::cache::Policy::JellyfinStatic),
        }),
    }];
    services.push(HostService {
        config: ServiceConfig::Plex(skillet_plex::Config {
            state_path: "/var/lib/data/plex/config",
            transcode_path: "/var/lib/data/plex/transcode",
            files_path: config.files_path,
            music_path: config.music_path,
            data_owner: config.data_owner,
            data_group: config.data_group,
        }),
        ui: Some(UiServiceDeclaration {
            dashboard_group: None,
            sharing: None,
            name: "plex",
            upstream: "plex",
            port: skillet_plex::PORT,
            transport: skillet_caddy::UpstreamTransport::Bridge,
            public_names: &["plex"],
            aliases: &[],
            tailnet_identity: false,
            cache: None,
        }),
    });
    services.push(HostService {
        config: ServiceConfig::Gokapi(skillet_gokapi::Config {
            state_path: "/var/lib/data/gokapi",
            backup_path: "/var/lib/data/syncthing/data/backups/gokapi",
            backup_subvolume: "/var/lib/data/syncthing",
            owner: config.data_owner,
            group: config.data_group,
        }),
        ui: Some(UiServiceDeclaration {
            dashboard_group: None,
            sharing: Some(SharingDeclaration {
                names: &["send"],
                policy: skillet_caddy::sharing::Policy::Gokapi224,
            }),
            name: "gokapi",
            upstream: "gokapi",
            port: skillet_gokapi::PORT,
            transport: skillet_caddy::UpstreamTransport::Bridge,
            public_names: &[],
            aliases: &[],
            tailnet_identity: true,
            cache: None,
        }),
    });
    services.extend(skillet_media::App::ALL.into_iter().map(|app| HostService {
        config: ServiceConfig::Media { app, config },
        ui: (app != skillet_media::App::Putioarr).then_some(UiServiceDeclaration {
            dashboard_group: None,
            sharing: None,
            public_names: &[],
            cache: None,
            name: app.name(),
            upstream: app.name(),
            transport: skillet_caddy::UpstreamTransport::Bridge,
            port: app.port(),
            tailnet_identity: true,
            aliases: &[],
        }),
    }));
    services
}

fn agent_baseline() -> HostProfile {
    HostProfile {
        id: HostId::known("agent"),
        signed_image: None,
        masked_units: &[],
        network: NetworkPolicy {
            ipv4_subnet: "172.26.26.0/24",
            ipv4_gateway: "172.26.26.1",
            ipv6_subnet: "fd59:4e23:2950:11f5::/64",
            ipv6_gateway: "fd59:4e23:2950:11f5::1",
        },
        requires_data_mount: false,
        services: Vec::new(),
    }
}

/// Resolve a validated host identity to its declared configuration.
pub fn profile_for_host(host: &HostId) -> Option<HostProfile> {
    match host.as_str() {
        "clamps" => Some(with_fleet_baseline(clamps())),
        "joey" => Some(with_fleet_baseline(joey())),
        "donbot" => Some(with_fleet_baseline(donbot())),
        "agent" => Some(agent_baseline()),
        _ => None,
    }
}

pub fn profile_for_name(host: &str) -> Option<HostProfile> {
    HostId::parse(host).and_then(|id| profile_for_host(&id))
}

pub fn declared_profiles() -> Vec<HostProfile> {
    vec![
        with_fleet_baseline(clamps()),
        with_fleet_baseline(joey()),
        with_fleet_baseline(donbot()),
        agent_baseline(),
    ]
}

fn with_fleet_baseline(mut profile: HostProfile) -> HostProfile {
    // File browsing is part of the shared Syncthing composition, not a second
    // independently maintained per-host inventory.
    let file_browsers: Vec<_> = profile
        .services
        .iter()
        .filter_map(|service| {
            let ServiceConfig::Syncthing {
                files_path,
                data_owner,
                data_group,
                container_uid,
                container_gid,
                ..
            } = service.config
            else {
                return None;
            };
            Some(HostService {
                config: ServiceConfig::Mikochi {
                    files_path,
                    data_owner,
                    data_group,
                    container_uid,
                    container_gid,
                },
                ui: Some(UiServiceDeclaration {
                    dashboard_group: None,
                    sharing: None,
                    public_names: &[],
                    cache: None,
                    name: "files",
                    upstream: "mikochi",
                    transport: skillet_caddy::UpstreamTransport::Bridge,
                    port: 8080,
                    aliases: &[],
                    tailnet_identity: true,
                }),
            })
        })
        .collect();
    profile.services.extend(file_browsers);
    profile.services.push(HostService {
        config: ServiceConfig::Homer,
        ui: Some(UiServiceDeclaration {
            dashboard_group: None,
            sharing: None,
            public_names: &[],
            cache: None,
            name: "homer",
            upstream: "homer",
            transport: skillet_caddy::UpstreamTransport::Bridge,
            port: 8080,
            aliases: &["{host}", "ui.{host}.{zone}"],
            tailnet_identity: true,
        }),
    });
    profile.services.push(HostService {
        config: ServiceConfig::Smtp,
        ui: None,
    });
    profile
}

fn smtp_acceptance() -> AcceptanceService {
    AcceptanceService {
        unit: "postfix.service".into(),
        container: None,
        network_mode: None,
        bind_paths: Vec::new(),
        owner: None,
        listeners: Vec::new(),
        health_probe: None,
    }
}

fn datadog_acceptance() -> AcceptanceService {
    AcceptanceService {
        unit: "datadog-agent.service".into(),
        container: Some("datadog-agent".into()),
        network_mode: Some("host".into()),
        bind_paths: vec!["/etc/skillet/datadog/conf.d".into()],
        owner: Some(AcceptanceOwner::Numeric { uid: 0, gid: 0 }),
        listeners: Vec::new(),
        health_probe: None,
    }
}

fn unifi_acceptance() -> AcceptanceService {
    AcceptanceService {
        unit: "unifi.service".into(),
        container: Some("unifi".into()),
        network_mode: Some("host".into()),
        bind_paths: vec!["/var/lib/data/unifi".into()],
        owner: Some(AcceptanceOwner::Numeric { uid: 999, gid: 999 }),
        listeners: vec![
            AcceptanceListener {
                port: 8080,
                protocol: ListenerProtocol::Tcp,
            },
            AcceptanceListener {
                port: 8443,
                protocol: ListenerProtocol::Tcp,
            },
            AcceptanceListener {
                port: 3478,
                protocol: ListenerProtocol::Udp,
            },
            AcceptanceListener {
                port: 10001,
                protocol: ListenerProtocol::Udp,
            },
        ],
        health_probe: None,
    }
}

fn homeassistant_acceptance() -> AcceptanceService {
    AcceptanceService {
        unit: "homeassistant.service".into(),
        container: Some("homeassistant".into()),
        network_mode: Some("host".into()),
        bind_paths: vec!["/var/lib/data/homeassistant/config".into()],
        owner: None,
        listeners: vec![],
        health_probe: Some(HealthProbe::Homeassistant),
    }
}

fn jellyfin_acceptance(config: skillet_jellyfin::Config, network: String) -> AcceptanceService {
    AcceptanceService {
        unit: "jellyfin.service".into(),
        container: Some("jellyfin".into()),
        network_mode: Some(network),
        bind_paths: vec![
            config.state_path.into(),
            config.cache_path.into(),
            config.libraries()[0].0.to_string_lossy().into_owned(),
            config.libraries()[1].0.to_string_lossy().into_owned(),
            config.music_path.into(),
        ],
        owner: Some(AcceptanceOwner::Named {
            user: config.data_owner.into(),
            group: config.data_group.into(),
        }),
        listeners: Vec::new(),
        health_probe: Some(HealthProbe::Jellyfin),
    }
}

fn nut_acceptance() -> AcceptanceService {
    AcceptanceService {
        unit: "nut-monitor.service".into(),
        container: None,
        network_mode: None,
        bind_paths: Vec::new(),
        owner: None,
        listeners: Vec::new(),
        health_probe: None,
    }
}

fn plex_acceptance(config: skillet_plex::Config, network: String) -> AcceptanceService {
    AcceptanceService {
        unit: "plex.service".into(),
        container: Some("plex".into()),
        network_mode: Some(network),
        bind_paths: std::iter::once(config.state_path.to_string())
            .chain(std::iter::once(config.transcode_path.to_string()))
            .chain(
                config
                    .libraries()
                    .into_iter()
                    .map(|(path, _)| path.to_string_lossy().into_owned()),
            )
            .collect(),
        owner: Some(AcceptanceOwner::Named {
            user: config.data_owner.into(),
            group: config.data_group.into(),
        }),
        listeners: vec![AcceptanceListener {
            port: skillet_plex::PORT,
            protocol: ListenerProtocol::Tcp,
        }],
        health_probe: Some(HealthProbe::Plex),
    }
}

fn gokapi_acceptance(config: skillet_gokapi::Config, network: String) -> AcceptanceService {
    AcceptanceService {
        unit: "gokapi.service".into(),
        container: Some("gokapi".into()),
        network_mode: Some(network),
        bind_paths: vec![
            format!("{}/config", config.state_path),
            format!("{}/data", config.state_path),
        ],
        owner: Some(AcceptanceOwner::Named {
            user: config.owner.into(),
            group: config.group.into(),
        }),
        listeners: vec![],
        health_probe: Some(HealthProbe::Gokapi),
    }
}
