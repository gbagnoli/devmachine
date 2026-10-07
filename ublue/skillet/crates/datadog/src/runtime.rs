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
use std::{collections::BTreeMap, path::Path};
use thiserror::Error;

pub const CREDENTIAL: &str = "datadog_config";
/// Official Agent 7 index selected on 2026-10-06.
pub const IMAGE: &str = "registry.datadoghq.com/agent@sha256:161a43ad2b290f7527a70e70c8880c1b3d65b39411552a8cb41398049546daf5";
const CONFIG_DIR: &str = "/etc/skillet/datadog/conf.d";

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
}

impl Input {
    pub fn render_for_environment(mut self, environment: &str) -> Result<String, DatadogError> {
        if !matches!(environment, "prod" | "test") {
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
        serde_json::to_string(&self).map_err(|_| DatadogError::Invalid)
    }

    pub fn parse(input: &str) -> Result<Self, DatadogError> {
        let value: Self = serde_json::from_str(input).map_err(|_| DatadogError::Invalid)?;
        if value.version != 1
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
            !matches!(unit.rsplit_once('.'), Some((_, "service" | "timer")))
                || !unit
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
    {
        return Err(DatadogError::Invalid);
    }
    files.require_btrfs_subvolume_mount(Path::new("/var/lib/data"), Path::new("/var"), "/data")?;
    let mut revisions = Vec::new();
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
    for (name, value) in [
        ("datadog_api_key", input.api_key),
        ("datadog_site", input.site),
        ("datadog_tags", tags),
    ] {
        system.ensure_podman_secret(name, &value)?;
        revisions.push(value.into_bytes());
    }
    system.service_enable("podman.socket")?;
    if !system.service_is_active("podman.socket")? {
        system.service_start("podman.socket")?;
    }
    skillet_podman::container(system, files, container_config(runtime, revisions))?;
    Ok(())
}

fn container_config(runtime: &RuntimeConfig<'_>, revisions: Vec<Vec<u8>>) -> PodmanConfig {
    let mut settings = vec![
        "ContainerName=datadog-agent".into(),
        "PodmanArgs=--pid=host --cgroupns=host --security-opt=label=disable".into(),
        "Environment=DOCKER_HOST=unix:///run/podman/podman.sock".into(),
        format!("Environment=DD_HOSTNAME={}", runtime.hostname),
        "Environment=DD_PROCESS_AGENT_ENABLED=true".into(),
        "Environment=DD_APM_ENABLED=false".into(),
        "Environment=DD_LOGS_ENABLED=false".into(),
        "Environment=DD_USE_DOGSTATSD=false".into(),
    ];
    if runtime.network_monitoring {
        settings.extend([
            "Environment=DD_SYSTEM_PROBE_NETWORK_ENABLED=true".into(),
            "Environment=DD_SYSTEM_PROBE_SERVICE_MONITORING_ENABLED=true".into(),
            "AddCapability=SYS_ADMIN SYS_RESOURCE SYS_PTRACE NET_ADMIN NET_BROADCAST NET_RAW IPC_LOCK CHOWN".into(),
        ]);
    } else {
        settings.push("Environment=DD_SYSTEM_PROBE_ENABLED=false".into());
    }
    let mut volumes = [
        ("/run/podman/podman.sock", "/run/podman/podman.sock"),
        ("/proc", "/host/proc"),
        ("/sys/fs/cgroup", "/host/sys/fs/cgroup"),
        ("/run/systemd", "/host/run/systemd"),
        ("/", "/host/root"),
        (CONFIG_DIR, "/etc/datadog-agent/conf.d"),
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
            ("datadog_site", "DD_SITE"),
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
                    "After=podman.socket network-online.target".into(),
                ],
            ),
            ("Service".into(), vec!["Restart=always".into()]),
            ("Install".into(), vec!["WantedBy=multi-user.target".into()]),
        ]),
    }
}

#[cfg(test)]
#[path = "runtime/tests.rs"]
mod tests;
