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
pub struct UiServiceDeclaration {
    pub name: &'static str,
    pub upstream: &'static str,
    pub port: u16,
    pub aliases: &'static [&'static str],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CredentialConsumer {
    pub credential: &'static str,
    pub unit: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceConfig {
    Pihole {
        custom_dns: &'static [(&'static str, &'static str)],
    },
    Syncthing {
        data_path: &'static str,
        data_owner: &'static str,
        data_group: &'static str,
        uid: u32,
        gid: u32,
    },
    Unifi,
    Tailscale {
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
    pub fn name(&self) -> &'static str {
        match self.config {
            ServiceConfig::Pihole { .. } => "pihole",
            ServiceConfig::Syncthing { .. } => "syncthing",
            ServiceConfig::Unifi => "unifi",
            ServiceConfig::Tailscale { .. } => "tailscale",
            ServiceConfig::Btrbk { .. } => "btrbk",
        }
    }

    pub fn credential_consumers(&self) -> Vec<CredentialConsumer> {
        match self.config {
            ServiceConfig::Pihole { .. } => vec![CredentialConsumer {
                credential: super::PIHOLE_WEB_PASSWORD_CREDENTIAL,
                unit: "pihole.service",
            }],
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

impl HostProfile {
    pub fn supports_service(&self, name: &str) -> bool {
        self.services.iter().any(|service| service.name() == name)
    }

    pub fn requires_pihole_dns_listener_policy(&self) -> bool {
        self.services
            .iter()
            .any(|service| matches!(service.config, ServiceConfig::Pihole { .. }))
    }

    pub fn requires_full_apply_credentials(&self) -> bool {
        self.services.iter().any(|service| {
            matches!(
                service.config,
                ServiceConfig::Pihole { .. } | ServiceConfig::Tailscale { .. }
            )
        })
    }

    pub fn ui_services(&self) -> Vec<skillet_caddy::UiService> {
        self.services
            .iter()
            .filter_map(|service| service.ui.as_ref())
            .map(|ui| skillet_caddy::UiService {
                name: ui.name.to_string(),
                upstream: ui.upstream.to_string(),
                port: ui.port,
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

const CLAMPS_DNS: &[(&str, &str)] = &[("192.168.1.100", "my.custom.domain")];
const CLAMPS_SYNCTHING_UI: UiServiceDeclaration = UiServiceDeclaration {
    name: "syncthing",
    upstream: "syncthing",
    port: 8384,
    aliases: &["sync.{host}"],
};
const BEEZELBOT_SYNCTHING_UI: UiServiceDeclaration = CLAMPS_SYNCTHING_UI;

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
                config: ServiceConfig::Pihole {
                    custom_dns: CLAMPS_DNS,
                },
                ui: Some(UiServiceDeclaration {
                    name: "pihole",
                    upstream: "pihole",
                    port: 8088,
                    aliases: &[],
                }),
            },
            HostService {
                config: ServiceConfig::Tailscale {
                    state_path: "/var/lib/data/tailscale",
                },
                ui: None,
            },
            HostService {
                config: ServiceConfig::Syncthing {
                    data_path: "/var/lib/data/syncthing",
                    data_owner: "giacomo",
                    data_group: "giacomo",
                    uid: 1000,
                    gid: 1000,
                },
                ui: Some(CLAMPS_SYNCTHING_UI),
            },
            HostService {
                config: ServiceConfig::Unifi,
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

fn beezelbot() -> HostProfile {
    HostProfile {
        id: HostId::known("beezelbot"),
        signed_image: Some("ghcr.io/gbagnoli/ucore-beezelbot"),
        masked_units: &[],
        network: NetworkPolicy {
            ipv4_subnet: "172.26.26.0/24",
            ipv4_gateway: "172.26.26.1",
            ipv6_subnet: "fd59:4e23:2950:11f5::/64",
            ipv6_gateway: "fd59:4e23:2950:11f5::1",
        },
        requires_data_mount: true,
        services: vec![HostService {
            config: ServiceConfig::Syncthing {
                data_path: "/var/lib/data/syncthing",
                data_owner: "giacomo",
                data_group: "giacomo",
                uid: 1000,
                gid: 1000,
            },
            ui: Some(BEEZELBOT_SYNCTHING_UI),
        }],
    }
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
        "clamps" => Some(clamps()),
        "beezelbot" => Some(beezelbot()),
        "agent" => Some(agent_baseline()),
        _ => None,
    }
}

pub fn profile_for_name(host: &str) -> Option<HostProfile> {
    HostId::parse(host).and_then(|id| profile_for_host(&id))
}

pub fn declared_profiles() -> Vec<HostProfile> {
    vec![clamps(), beezelbot(), agent_baseline()]
}
