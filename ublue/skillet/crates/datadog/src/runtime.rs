use serde::{Deserialize, Serialize};
use serde_json::json;
use skillet_core::{
    files::{FileError, FileMutationResource, FileReadResource, Ownership, StorageResource},
    system::{
        AccountLookupResource, AccountResource, PodmanSecretResource, ServiceResource, SystemError,
    },
};
use skillet_podman::{
    MountDependency, NetworkAttachment, PodmanConfig, PodmanError, ProcessIdentity, QuadletSecret,
    SecretTarget, Volume,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
    path::Path,
};
use thiserror::Error;

pub const CREDENTIAL: &str = "datadog_config";
/// Official supported major channel; Quadlet auto-update follows Agent 7 fixes.
pub const IMAGE: &str = "registry.datadoghq.com/agent:7";
const CONFIG_DIR: &str = "/etc/skillet/datadog/conf.d";
pub const DOGSTATSD_DIR: &str = "/run/skillet-dogstatsd";
pub const DOGSTATSD_SOCKET: &str = "/run/skillet-dogstatsd/dsd.socket";

#[derive(Debug, Error)]
pub enum DatadogError {
    #[error("invalid Datadog configuration or credentials")]
    Invalid,
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    System(#[from] SystemError),
    #[error(transparent)]
    Podman(#[from] PodmanError),
}

// Deliberately no Debug: this contains an API key and private tags.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    version: u32,
    api_key: String,
    site: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default = "default_dns_servers")]
    dns_servers: Vec<IpAddr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    switches: Option<Vec<SwitchConfig>>,
}

/// Secret-bearing switch configuration delivered only inside the encrypted
/// Datadog credential payload. Deliberately has no `Debug` implementation.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SwitchConfig {
    pub name: String,
    pub endpoint: String,
    pub make: String,
    pub password: String,
    pub metrics_port: u16,
}

impl SwitchConfig {
    fn valid(&self) -> bool {
        valid_switch_name(&self.name)
            && self.make == "netgear"
            && !self.password.is_empty()
            && !self.password.contains('\0')
            && valid_switch_endpoint(&self.endpoint)
            && (9500..9532).contains(&self.metrics_port)
    }
}

fn valid_switch_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name.as_bytes()[0].is_ascii_lowercase()
        && name
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn valid_switch_endpoint(value: &str) -> bool {
    let Ok(url) = url::Url::parse(value) else {
        return false;
    };
    let private_host = match url.host() {
        Some(url::Host::Ipv4(address)) => address.is_private() || address.is_link_local(),
        Some(url::Host::Ipv6(address)) => {
            address.is_unique_local() || address.is_unicast_link_local()
        }
        Some(url::Host::Domain(_)) | None => false,
    };
    url.scheme() == "http"
        && private_host
        && url.username().is_empty()
        && url.password().is_none()
        && (url.path().is_empty() || url.path() == "/")
        && url.query().is_none()
        && url.fragment().is_none()
}

fn default_dns_servers() -> Vec<IpAddr> {
    vec![IpAddr::from([1, 1, 1, 1]), IpAddr::from([1, 0, 0, 1])]
}

impl Input {
    pub fn set_switches(
        &mut self,
        switches: Option<Vec<SwitchConfig>>,
    ) -> Result<(), DatadogError> {
        let mut names = BTreeSet::new();
        let mut ports = BTreeSet::new();
        if switches.as_ref().is_some_and(|switches| {
            switches.iter().any(|switch| {
                !switch.valid()
                    || !names.insert(switch.name.as_str())
                    || !ports.insert(switch.metrics_port)
            })
        }) {
            return Err(DatadogError::Invalid);
        }
        self.switches = switches;
        Ok(())
    }

    pub fn switches(&self) -> Option<&[SwitchConfig]> {
        self.switches.as_deref()
    }

    /// Host capability tags are authoritative, including on retained payloads.
    pub fn render_with_kexec(mut self, enabled: bool) -> Result<String, DatadogError> {
        self.tags
            .retain(|tag| tag != "ucore-kexec" && !tag.starts_with("ucore-kexec:"));
        if enabled {
            self.tags.push("ucore-kexec".into());
        }
        serde_json::to_string(&self).map_err(|_| DatadogError::Invalid)
    }

    pub fn render_for_environment(
        mut self,
        environment: &str,
        production_region: &str,
    ) -> Result<String, DatadogError> {
        if !matches!(environment, "prod" | "test")
            || production_region.is_empty()
            || !production_region
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(DatadogError::Invalid);
        }
        let expected = format!("env:{environment}");
        if self
            .tags
            .iter()
            .any(|tag| tag.starts_with("env:") && tag != &expected)
        {
            return Err(DatadogError::Invalid);
        }
        if !self.tags.contains(&expected) {
            self.tags.push(expected);
        }
        let region = if environment == "test" {
            "lab"
        } else {
            production_region
        };
        // Region is deployment policy: templates cannot move test telemetry
        // into a production location or override the host caller's location.
        self.tags.retain(|tag| !tag.starts_with("region:"));
        self.tags.push(format!("region:{region}"));
        if environment == "test" {
            self.switches = None;
        }
        serde_json::to_string(&self).map_err(|_| DatadogError::Invalid)
    }

    pub fn parse(input: &str) -> Result<Self, DatadogError> {
        let value: Self = serde_json::from_str(input).map_err(|_| DatadogError::Invalid)?;
        if value.version != 1
            || value.dns_servers.is_empty()
            || value.dns_servers.iter().any(|ip| {
                ip.is_unspecified()
                    || ip.is_multicast()
                    || matches!(ip, IpAddr::V4(ip) if ip.is_broadcast())
            })
            || value.dns_servers.iter().collect::<BTreeSet<_>>().len() != value.dns_servers.len()
            || value.api_key.len() != 32
            || !value.api_key.bytes().all(|byte| byte.is_ascii_hexdigit())
            || !matches!(
                value.site.as_str(),
                "datadoghq.com"
                    | "datadoghq.eu"
                    | "us3.datadoghq.com"
                    | "us5.datadoghq.com"
                    | "ap1.datadoghq.com"
                    | "ap2.datadoghq.com"
                    | "ddog-gov.com"
                    | "us2.ddog-gov.com"
            )
            || value.tags.iter().any(|tag| {
                tag.is_empty()
                    || tag.len() > 200
                    || tag
                        .chars()
                        .any(|character| character.is_whitespace() || character.is_control())
            })
            || value.switches.as_ref().is_some_and(|switches| {
                switches.iter().any(|switch| !switch.valid())
                    || switches
                        .iter()
                        .map(|switch| switch.name.as_str())
                        .collect::<BTreeSet<_>>()
                        .len()
                        != switches.len()
                    || switches
                        .iter()
                        .map(|switch| switch.metrics_port)
                        .collect::<BTreeSet<_>>()
                        .len()
                        != switches.len()
            })
            || (value.tags.iter().any(|tag| tag == "env:test")
                && value
                    .switches
                    .as_ref()
                    .is_some_and(|switches| !switches.is_empty()))
        {
            return Err(DatadogError::Invalid);
        }
        Ok(value)
    }
}

pub struct RuntimeConfig<'a> {
    pub hostname: &'a str,
    pub network_monitoring: bool,
    pub monitored_units: &'a [String],
}

pub fn apply<S, F>(
    system: &S,
    files: &F,
    input: &str,
    runtime: &RuntimeConfig<'_>,
) -> Result<(), DatadogError>
where
    S: AccountLookupResource + AccountResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    let input = Input::parse(input)?;
    if runtime.hostname.is_empty()
        || runtime.hostname.len() > 63
        || !runtime
            .hostname
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || runtime.monitored_units.is_empty()
        || runtime.monitored_units.iter().any(|unit| {
            unit == "datadog-agent.service"
                || !matches!(unit.rsplit_once('.'), Some((_, "service" | "timer")))
                || !unit.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'@')
                })
        })
    {
        return Err(DatadogError::Invalid);
    }
    files.require_btrfs_subvolume_mount(Path::new("/var/lib/data"), Path::new("/var"), "/data")?;
    let root = Ownership::named(Some("root"), Some("root"));
    // The Agent creates a write-only 0722 socket. Restrict traversal of
    // its directory instead of relying on an unsupported socket-mode setting.
    files.ensure_directory(Path::new(DOGSTATSD_DIR), Some(0o700), &root)?;
    files.ensure_directory(Path::new("/etc/tmpfiles.d"), Some(0o755), &root)?;
    files.ensure_file(
        Path::new("/etc/tmpfiles.d/skillet-dogstatsd.conf"),
        format!("d {DOGSTATSD_DIR} 0700 root root -\n").as_bytes(),
        Some(0o644),
        &root,
    )?;
    files.ensure_directory(
        &Path::new(CONFIG_DIR).join("openmetrics.d"),
        Some(0o755),
        &root,
    )?;
    let mut revisions = Vec::new();
    if let Some(switches) = input.switches() {
        revisions.push(apply_switches(system, files, switches)?);
    }
    for (name, instance) in [
        ("btrfs", json!({})),
        ("network", json!({})),
        (
            "tcp_check",
            json!({"name": "host-ssh", "host": "127.0.0.1", "port": 22}),
        ),
        (
            "systemd",
            json!({"unit_names": runtime.monitored_units, "private_socket": "/host/run/systemd/private"}),
        ),
    ] {
        let directory = Path::new(CONFIG_DIR).join(format!("{name}.d"));
        files.ensure_directory(
            &directory,
            Some(0o755),
            &Ownership::named(Some("root"), Some("root")),
        )?;
        let config = json!({"init_config": {}, "instances": [instance]}).to_string();
        files.ensure_file(
            &directory.join("conf.yaml"),
            config.as_bytes(),
            Some(0o644),
            &Ownership::named(Some("root"), Some("root")),
        )?;
        revisions.push(config.into_bytes());
    }
    let tags = input
        .tags
        .into_iter()
        .chain(["service:host".into()])
        .collect::<Vec<_>>()
        .join(" ");
    for (name, value) in [("datadog_api_key", input.api_key), ("datadog_tags", tags)] {
        system.ensure_podman_secret(name, &value)?;
        revisions.push(value.into_bytes());
    }
    system.service_enable("podman.socket")?;
    if !system.service_is_active("podman.socket")? {
        system.service_start("podman.socket")?;
    }
    skillet_podman::container(
        system,
        files,
        container_config(runtime, revisions, &input.site, &input.dns_servers),
    )?;
    Ok(())
}

fn container_config(
    runtime: &RuntimeConfig<'_>,
    revisions: Vec<Vec<u8>>,
    site: &str,
    dns_servers: &[IpAddr],
) -> PodmanConfig {
    let mut settings = vec![
        "ContainerName=datadog-agent".into(),
        "AutoUpdate=registry".into(),
        "PodmanArgs=--pid=host --cgroupns=host --security-opt=label=disable".into(),
        "Environment=DOCKER_HOST=unix:///run/podman/podman.sock".into(),
        format!("Environment=DD_HOSTNAME={}", runtime.hostname),
        format!("Environment=DD_SITE={site}"),
        "Environment=DD_PROCESS_AGENT_ENABLED=true".into(),
        "Environment=DD_APM_ENABLED=false".into(),
        "Environment=DD_LOGS_ENABLED=false".into(),
        "Environment=DD_USE_DOGSTATSD=true".into(),
        "Environment=DD_DOGSTATSD_PORT=0".into(),
        format!("Environment=DD_DOGSTATSD_SOCKET={DOGSTATSD_SOCKET}"),
    ];
    // Monitoring must not depend on a filtered/unavailable resolver it observes.
    settings.extend(dns_servers.iter().map(|ip| format!("DNS={ip}")));
    if runtime.network_monitoring {
        // Podman's default seccomp profile allows perf_event_open only when
        // PERFMON is explicitly selected. SYS_ADMIN alone leaves the syscall
        // at the profile's ENOSYS default even though the kernel supports it.
        settings.extend([
            "Environment=DD_SYSTEM_PROBE_NETWORK_ENABLED=true".into(),
            "Environment=DD_SYSTEM_PROBE_SERVICE_MONITORING_ENABLED=true".into(),
            "AddCapability=SYS_ADMIN SYS_RESOURCE SYS_PTRACE NET_ADMIN NET_BROADCAST NET_RAW IPC_LOCK CHOWN PERFMON".into(),
        ]);
    } else {
        settings.push("Environment=DD_SYSTEM_PROBE_ENABLED=false".into());
    }
    let mut volumes = [
        ("/run/podman", "/run/podman"),
        ("/proc", "/host/proc"),
        ("/sys/fs/cgroup", "/host/sys/fs/cgroup"),
        ("/run/systemd", "/host/run/systemd"),
        ("/", "/host/root"),
    ]
    .into_iter()
    .map(|(host, container)| Volume {
        host_path: host.into(),
        container_path: container.into(),
        options: Some("ro".into()),
        host_mode: None,
        host_ownership: None,
    })
    .collect::<Vec<_>>();
    volumes.push(dogstatsd_volume());
    // Preserve the image's built-in integrations and auto-configuration.
    for name in ["btrfs", "network", "tcp_check", "systemd", "openmetrics"] {
        volumes.push(Volume {
            host_path: format!("{CONFIG_DIR}/{name}.d"),
            container_path: format!("/etc/datadog-agent/conf.d/{name}.d"),
            options: Some("ro".into()),
            host_mode: None,
            host_ownership: None,
        });
    }
    if runtime.network_monitoring {
        volumes.push(Volume {
            host_path: "/sys/kernel/debug".into(),
            container_path: "/sys/kernel/debug".into(),
            options: Some("ro".into()),
            host_mode: None,
            host_ownership: None,
        });
    }
    PodmanConfig {
        name: "datadog-agent".into(),
        image: IMAGE.into(),
        network_attachments: vec![NetworkAttachment::Host],
        port_publications: Vec::new(),
        storage_dependency: Some(MountDependency::shared_service_data()),
        process_identity: ProcessIdentity::Numeric { uid: 0, gid: 0 },
        namespace_mapping: None,
        volumes,
        secrets: [
            ("datadog_api_key", "DD_API_KEY"),
            ("datadog_tags", "DD_TAGS"),
        ]
        .into_iter()
        .map(|(name, target)| QuadletSecret {
            secret_name: name.into(),
            target: SecretTarget::Environment {
                env_var_name: target.into(),
            },
        })
        .collect(),
        config_revisions: revisions,
        extra_config: BTreeMap::from([
            ("Container".into(), settings),
            (
                "Unit".into(),
                vec![
                    "Requires=podman.socket".into(),
                    // The Agent's container metadata collector takes an initial
                    // inventory and then consumes runtime events. Order that
                    // inventory after concurrent application startup, without
                    // pulling in services whose credentials are not delivered.
                    format!(
                        "After=podman.socket network-online.target {}",
                        runtime.monitored_units.join(" ")
                    ),
                ],
            ),
            ("Service".into(), vec!["Restart=always".into()]),
            ("Install".into(), vec!["WantedBy=multi-user.target".into()]),
        ]),
    }
}

fn apply_switches<S, F>(
    system: &S,
    files: &F,
    switches: &[SwitchConfig],
) -> Result<Vec<u8>, DatadogError>
where
    S: AccountLookupResource + AccountResource + PodmanSecretResource + ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + StorageResource + ?Sized,
{
    let root = Ownership::named(Some("root"), Some("root"));
    let state_dir = Path::new("/var/lib/skillet/switches");
    files.ensure_directory(state_dir, Some(0o700), &root)?;
    let journal = state_dir.join("inventory.json");
    let previous: BTreeSet<String> = files
        .read_file(&journal)?
        .map(|bytes| serde_json::from_slice(&bytes).map_err(|_| DatadogError::Invalid))
        .transpose()?
        .unwrap_or_default();
    if previous.iter().any(|name| !valid_switch_name(name)) {
        return Err(DatadogError::Invalid);
    }
    let desired: BTreeSet<String> = switches.iter().map(|switch| switch.name.clone()).collect();
    let pending = previous.union(&desired).cloned().collect::<BTreeSet<_>>();
    let pending_bytes = serde_json::to_vec(&pending).map_err(|_| DatadogError::Invalid)?;
    files.ensure_file(&journal, &pending_bytes, Some(0o600), &root)?;
    for obsolete in previous.difference(&desired) {
        let name = format!("skillet-switch-{obsolete}");
        let quadlet = Path::new("/etc/containers/systemd").join(format!("{name}.container"));
        if files.read_file(&quadlet)?.is_some() {
            system.service_stop(&format!("{name}.service"))?;
            files.delete_file(&quadlet)?;
            system.daemon_reload()?;
        }
        system.delete_podman_secret(&name)?;
    }
    let mut instances = Vec::new();
    for switch in switches {
        let name = format!("skillet-switch-{}", switch.name);
        let payload = serde_json::to_string(switch).map_err(|_| DatadogError::Invalid)?;
        system.ensure_podman_secret(&name, &payload)?;
        skillet_podman::container(
            system,
            files,
            PodmanConfig {
                name: name.clone(),
                image: "ghcr.io/gbagnoli/skillet-switch-collector:v1".into(),
                network_attachments: vec![NetworkAttachment::Host],
                port_publications: Vec::new(),
                storage_dependency: None,
                process_identity: ProcessIdentity::Numeric {
                    uid: 65534,
                    gid: 65534,
                },
                namespace_mapping: None,
                volumes: Vec::new(),
                secrets: vec![QuadletSecret {
                    secret_name: name.clone(),
                    target: SecretTarget::File {
                        target_path: "/run/secrets/switch_config".into(),
                        mode: Some("0400".into()),
                        uid: Some(65534),
                        gid: Some(65534),
                    },
                }],
                config_revisions: vec![payload.into_bytes()],
                extra_config: BTreeMap::from([
                    (
                        "Container".into(),
                        vec![
                            format!("ContainerName={name}"),
                            "AutoUpdate=registry".into(),
                            "NoNewPrivileges=true".into(),
                            "DropCapability=all".into(),
                            "ReadOnly=true".into(),
                        ],
                    ),
                    ("Service".into(), vec!["Restart=always".into()]),
                    ("Install".into(), vec!["WantedBy=multi-user.target".into()]),
                ]),
            },
        )?;
        instances.push(json!({"openmetrics_endpoint":format!("http://127.0.0.1:{}/metrics", switch.metrics_port), "namespace":"netgear", "metrics":["netgear_collection_success", "netgear_last_success_timestamp_seconds", "netgear_port_link_up", "netgear_port_connection_speed_megabits_per_second"], "tags":[format!("switch:{}", switch.name)]}));
    }
    let root = Ownership::named(Some("root"), Some("root"));
    let directory = Path::new(CONFIG_DIR).join("openmetrics.d");
    files.ensure_directory(&directory, Some(0o755), &root)?;
    let config = json!({"init_config":{},"instances":instances})
        .to_string()
        .into_bytes();
    files.ensure_file(&directory.join("conf.yaml"), &config, Some(0o644), &root)?;
    let desired_bytes = serde_json::to_vec(&desired).map_err(|_| DatadogError::Invalid)?;
    files.ensure_file(&journal, &desired_bytes, Some(0o600), &root)?;
    Ok(config)
}

fn dogstatsd_volume() -> Volume {
    Volume {
        host_path: DOGSTATSD_DIR.into(),
        container_path: DOGSTATSD_DIR.into(),
        options: Some("rw".into()),
        host_mode: Some(0o700),
        host_ownership: Some(Ownership::named(Some("root"), Some("root"))),
    }
}

#[cfg(test)]
#[path = "runtime/tests.rs"]
mod tests;
