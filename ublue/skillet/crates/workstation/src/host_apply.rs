//! Ordered production application from the canonical host declaration.
use skillet_hosts::HostProfile;
use skillet_vm::transport::{GuestCommand, GuestTransport};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Stage(&'static str),
    Activate(&'static str),
    TailnetReady,
}

/// Exact generic CLI arguments for every unit activated by remote apply. The
/// temporary `ExecStart` override uses these to run workstation code even when
/// the guest's persistent `/etc` unit still has an older executable path.
pub fn temporary_arguments(unit: &str) -> Option<&'static [&'static str]> {
    const HOST_FILE: &str = "/etc/skillet/host";
    match unit {
        "skillet-apply.service" => Some(&["apply", "--host-file", HOST_FILE, "--phase", "base"]),
        "skillet-syncthing-initialize.service" => {
            Some(&["syncthing", "initialize", "--host-file", HOST_FILE])
        }
        "skillet-syncthing-personal-initialize.service" => Some(&[
            "syncthing",
            "initialize",
            "--host-file",
            HOST_FILE,
            "--instance",
            "personal",
        ]),
        "skillet-full-apply.service" => {
            Some(&["apply", "--host-file", HOST_FILE, "--phase", "full"])
        }
        "skillet-pihole-dns-apply.service" => {
            Some(&["apply", "--host-file", HOST_FILE, "--phase", "pihole-dns"])
        }
        "skillet-caddy-apply.service" => {
            Some(&["apply", "--host-file", HOST_FILE, "--phase", "caddy"])
        }
        "skillet-gokapi-apply.service" => {
            Some(&["apply", "--host-file", HOST_FILE, "--phase", "gokapi"])
        }
        skillet_homeassistant::zigbee::UNIT => {
            Some(&["homeassistant", "zigbee", "--host-file", HOST_FILE])
        }
        "skillet-plex-apply.service" => {
            Some(&["apply", "--host-file", HOST_FILE, "--phase", "plex"])
        }
        "skillet-joplin-apply.service" => {
            Some(&["apply", "--host-file", HOST_FILE, "--phase", "joplin"])
        }
        "skillet-airtrail-apply.service" => {
            Some(&["apply", "--host-file", HOST_FILE, "--phase", "airtrail"])
        }
        "skillet-media-apply.service" => {
            Some(&["apply", "--host-file", HOST_FILE, "--phase", "media"])
        }
        "skillet-smtp-apply.service" => {
            Some(&["apply", "--host-file", HOST_FILE, "--phase", "smtp"])
        }
        "skillet-ddns-apply.service" => {
            Some(&["apply", "--host-file", HOST_FILE, "--phase", "ddns"])
        }
        "skillet-network-prepare.service" => {
            Some(&["apply", "--host-file", HOST_FILE, "--phase", "network"])
        }
        "skillet-datadog-apply.service" => {
            Some(&["apply", "--host-file", HOST_FILE, "--phase", "datadog"])
        }
        _ => None,
    }
}

/// Native helper started inside an apply transaction, rather than by its SSH caller.
pub fn temporary_dependency(unit: &str) -> Option<(&'static str, &'static [&'static str])> {
    match unit {
        "skillet-gokapi-apply.service" => Some((
            "skillet-gokapi-initialize.service",
            &["gokapi", "initialize", "--host-file", "/etc/skillet/host"],
        )),
        "skillet-media-apply.service" => {
            Some(("skillet-media-reconcile.service", &["media", "reconcile"]))
        }
        "skillet-smtp-apply.service" => {
            Some(("skillet-smtp-prepare.service", &["smtp", "prepare"]))
        }
        _ => None,
    }
}

/// Service preparation and activation are distinct effects. Preparation must
/// defer consumer activation; each related set is complete before its start.
pub trait Actions {
    type Error;
    fn stage(&mut self, services: &[&'static str]) -> Result<(), Self::Error>;
    fn activate(&mut self, unit: &'static str) -> Result<(), Self::Error>;
    fn wait_for_tailnet(&mut self) -> Result<(), Self::Error>;
}

const BASE: Step = Step::Activate("skillet-apply.service");
const FULL: Step = Step::Activate("skillet-full-apply.service");
const CADDY: Step = Step::Activate("skillet-caddy-apply.service");

struct Node {
    step: Step,
    requires: Vec<Step>,
}

#[derive(Debug, thiserror::Error)]
#[error("apply dependencies cannot be satisfied for {0:?}")]
pub struct PlanError(Step);

fn schedule(mut pending: Vec<Node>) -> Result<Vec<Step>, PlanError> {
    let mut ready = Vec::with_capacity(pending.len());
    while let Some(first) = pending.first() {
        let next = pending
            .iter()
            .enumerate()
            .filter(|(_, node)| node.requires.iter().all(|step| ready.contains(step)))
            .min_by_key(|(_, node)| !matches!(node.step, Step::Stage(_)))
            .map(|(index, _)| index)
            .ok_or(PlanError(first.step))?;
        ready.push(pending.remove(next).step);
    }
    Ok(ready)
}

pub fn steps(profile: &HostProfile) -> Result<Vec<Step>, PlanError> {
    let mut nodes = vec![Node {
        step: BASE,
        requires: vec![],
    }];
    let mut full_requires = vec![BASE];
    for service in ["pihole", "tailscale"] {
        if profile.services.iter().any(|declaration| {
            declaration.name() == service
                && declaration
                    .credential_consumers()
                    .iter()
                    .any(|consumer| consumer.unit != skillet_pihole::dns::UNIT)
        }) {
            let step = Step::Stage(service);
            nodes.push(Node {
                step,
                requires: vec![BASE],
            });
            full_requires.push(step);
        }
    }
    if profile.supports_service("syncthing") {
        let stage = Step::Stage("syncthing");
        nodes.push(Node {
            step: stage,
            requires: vec![BASE],
        });
        for service in &profile.services {
            if let skillet_hosts::ServiceConfig::Syncthing { instance, .. } = service.config {
                let step = Step::Activate(instance.initial_unit());
                nodes.push(Node {
                    step,
                    requires: vec![stage],
                });
                full_requires.push(step);
            }
        }
    }
    nodes.push(Node {
        step: FULL,
        requires: full_requires,
    });
    let has_caddy = !profile.ui_services().is_empty();
    if has_caddy {
        let readiness = if profile.supports_service("tailscale") {
            nodes.push(Node {
                step: Step::TailnetReady,
                requires: vec![FULL],
            });
            Step::TailnetReady
        } else {
            FULL
        };
        let stage = Step::Stage("caddy");
        nodes.push(Node {
            step: stage,
            requires: vec![readiness],
        });
        nodes.push(Node {
            step: CADDY,
            requires: vec![stage],
        });
    }

    add_service_nodes(profile, &mut nodes, has_caddy);
    schedule(nodes)
}

fn add_service_nodes(profile: &HostProfile, nodes: &mut Vec<Node>, has_caddy: bool) {
    for (service, unit, stages, uses_caddy, production_only) in [
        ("pihole", skillet_pihole::dns::UNIT, true, false, false),
        ("gokapi", "skillet-gokapi-apply.service", true, true, false),
        ("plex", "skillet-plex-apply.service", false, true, false),
        ("joplin", "skillet-joplin-apply.service", true, true, false),
        (
            "airtrail",
            "skillet-airtrail-apply.service",
            true,
            true,
            false,
        ),
        ("media", "skillet-media-apply.service", true, true, false),
        (
            "homeassistant",
            skillet_homeassistant::zigbee::UNIT,
            true,
            false,
            false,
        ),
        ("smtp", "skillet-smtp-apply.service", true, false, true),
        ("ddns", "skillet-ddns-apply.service", true, false, true),
        (
            "network",
            "skillet-network-prepare.service",
            true,
            false,
            true,
        ),
        (
            "datadog",
            "skillet-datadog-apply.service",
            true,
            false,
            true,
        ),
    ] {
        let selected = if production_only {
            profile.supports_production_service(service)
        } else {
            profile.supports_service(service)
        };
        if !selected {
            continue;
        }
        let mut requires = vec![FULL];
        if uses_caddy && has_caddy {
            requires.push(CADDY);
        }
        if service == "datadog" {
            requires.extend(nodes.iter().filter_map(|node| match node.step {
                Step::Activate(_) => Some(node.step),
                _ => None,
            }));
        }
        if stages {
            let stage = Step::Stage(if service == "pihole" {
                "pihole-dns"
            } else {
                service
            });
            nodes.push(Node {
                step: stage,
                requires: vec![BASE],
            });
            requires.push(stage);
        }
        nodes.push(Node {
            step: Step::Activate(unit),
            requires,
        });
    }
}

pub fn run<A: Actions>(plan: &[Step], actions: &mut A) -> Result<(), A::Error> {
    let mut steps = plan.iter().copied().peekable();
    while let Some(step) = steps.next() {
        match step {
            Step::Stage(service) => {
                let mut batch = vec![service];
                while let Some(Step::Stage(service)) = steps.peek().copied() {
                    steps.next();
                    batch.push(service);
                }
                actions.stage(&batch)?;
            }
            Step::Activate(unit) => actions.activate(unit)?,
            Step::TailnetReady => actions.wait_for_tailnet()?,
        }
    }
    Ok(())
}

/// A skipped condition is not a completed oneshot. Some systemd versions unload
/// inactive oneshots before a follow-up SSH query, so retain a journal cursor and
/// use the manager's completion event as execution evidence when runtime fields vanish.
pub fn activate(guest: &impl GuestTransport, unit: &str) -> skillet_vm::Result<()> {
    let cursor = journal_cursor(guest)?;
    let started = guest.execute(
        &GuestCommand {
            program: "/usr/bin/sudo",
            arguments: &["-n", "systemctl", "start", unit],
        },
        None,
    )?;
    if !started.status.success() {
        return Err(skillet_vm::Error::Guest {
            operation: format!("starting {unit}"),
            code: started.status.code(),
        });
    }
    let status = guest.execute(
        &GuestCommand {
            program: "systemctl",
            arguments: &[
                "show",
                unit,
                "-p",
                "LoadState",
                "-p",
                "ConditionResult",
                "-p",
                "Result",
                "-p",
                "ExecMainStatus",
                "-p",
                "ExecMainStartTimestampMonotonic",
            ],
        },
        None,
    )?;
    if runtime_state_confirms_execution(&status) {
        return Ok(());
    }
    if journal_confirms_start(guest, unit, &cursor)? {
        return Ok(());
    }
    Err(skillet_vm::Error::Invalid(format!(
        "{unit} did not execute successfully; inspect its journal"
    )))
}

fn journal_cursor(guest: &impl GuestTransport) -> skillet_vm::Result<String> {
    let output = guest.execute(
        &GuestCommand {
            program: "/usr/bin/sudo",
            arguments: &["-n", "journalctl", "--no-pager", "-n", "0", "--show-cursor"],
        },
        None,
    )?;
    if !output.status.success() {
        return Err(skillet_vm::Error::Invalid(
            "could not capture the systemd journal cursor before apply".into(),
        ));
    }
    std::str::from_utf8(&output.stdout)
        .map_err(|_| skillet_vm::Error::Invalid("invalid systemd journal cursor".into()))?
        .lines()
        .find_map(|line| line.strip_prefix("-- cursor: "))
        .map(str::to_owned)
        .ok_or_else(|| skillet_vm::Error::Invalid("missing systemd journal cursor".into()))
}

fn runtime_state_confirms_execution(status: &std::process::Output) -> bool {
    let Ok(text) = std::str::from_utf8(&status.stdout) else {
        return false;
    };
    status.status.success()
        && [
            "LoadState=loaded",
            "ConditionResult=yes",
            "Result=success",
            "ExecMainStatus=0",
        ]
        .iter()
        .all(|required| text.lines().any(|line| line == *required))
        && text
            .lines()
            .find_map(|line| line.strip_prefix("ExecMainStartTimestampMonotonic="))
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|value| value != 0)
}

fn journal_confirms_start(
    guest: &impl GuestTransport,
    unit: &str,
    cursor: &str,
) -> skillet_vm::Result<bool> {
    let output = guest.execute(
        &GuestCommand {
            program: "/usr/bin/sudo",
            arguments: &[
                "-n",
                "journalctl",
                "-u",
                unit,
                "--no-pager",
                "--output=json",
                "--after-cursor",
                cursor,
            ],
        },
        None,
    )?;
    if !output.status.success() {
        return Ok(false);
    }
    let entries = std::str::from_utf8(&output.stdout)
        .map_err(|_| skillet_vm::Error::Invalid("invalid systemd apply journal".into()))?;
    Ok(journal_has_successful_start(entries, unit))
}

fn journal_has_successful_start(entries: &str, unit: &str) -> bool {
    const FINISHED_MESSAGE_ID: &str = "39f53479d3a045ac8e11786248231fbf";
    entries
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .any(|entry| {
            entry.get("MESSAGE_ID").and_then(serde_json::Value::as_str) == Some(FINISHED_MESSAGE_ID)
                && entry.get("UNIT").and_then(serde_json::Value::as_str) == Some(unit)
                && entry.get("JOB_TYPE").and_then(serde_json::Value::as_str) == Some("start")
                && entry.get("JOB_RESULT").and_then(serde_json::Value::as_str) == Some("done")
                && entry
                    .get("INVOCATION_ID")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| !value.is_empty())
        })
}

#[cfg(test)]
#[path = "host_apply/tests.rs"]
mod tests;
