//! Ordered production application from the canonical host declaration.
use skillet_hosts::HostProfile;
use skillet_vm::transport::{GuestCommand, GuestTransport};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Stage(&'static str),
    Activate(&'static str),
}

/// Service preparation and activation are distinct effects. Preparation must
/// defer consumer activation; each related set is complete before its start.
pub trait Actions {
    type Error;
    fn stage(&mut self, service: &'static str) -> Result<(), Self::Error>;
    fn activate(&mut self, unit: &'static str) -> Result<(), Self::Error>;
}

pub fn steps(profile: &HostProfile) -> Vec<Step> {
    let mut result = vec![Step::Activate("skillet-apply.service")];
    for service in ["pihole", "tailscale"] {
        if profile.services.iter().any(|declaration| {
            declaration.name() == service
                && declaration
                    .credential_consumers()
                    .iter()
                    .any(|c| c.unit != skillet_pihole::dns::UNIT)
        }) {
            result.push(Step::Stage(service));
        }
    }
    if profile.supports_service("syncthing") {
        result.push(Step::Stage("syncthing"));
        for service in &profile.services {
            if let skillet_hosts::ServiceConfig::Syncthing { instance, .. } = service.config {
                result.push(Step::Activate(instance.initial_unit()));
            }
        }
    }
    result.push(Step::Activate("skillet-full-apply.service"));
    if profile.supports_service("pihole") {
        result.extend([
            Step::Stage("pihole-dns"),
            Step::Activate(skillet_pihole::dns::UNIT),
        ]);
    }
    // UI DNS needs the enrolled device addresses. Monitoring is last so all
    // selected workloads have been activated before the Agent discovers them.
    if !profile.ui_services().is_empty() {
        result.extend([
            Step::Stage("caddy"),
            Step::Activate("skillet-caddy-apply.service"),
        ]);
    }
    if profile.supports_service("gokapi") {
        result.extend([
            Step::Stage("gokapi"),
            Step::Activate("skillet-gokapi-apply.service"),
        ]);
    }
    if profile.supports_service("plex") {
        result.push(Step::Activate("skillet-plex-apply.service"));
    }
    if profile.supports_service("joplin") {
        result.extend([
            Step::Stage("joplin"),
            Step::Activate("skillet-joplin-apply.service"),
        ]);
    }
    if profile.supports_service("airtrail") {
        result.extend([
            Step::Stage("airtrail"),
            Step::Activate("skillet-airtrail-apply.service"),
        ]);
    }
    if profile.supports_service("media") {
        result.extend([
            Step::Stage("media"),
            Step::Activate("skillet-media-apply.service"),
        ]);
    }
    if profile.supports_service("homeassistant") {
        result.push(Step::Stage("homeassistant"));
    }
    for (service, unit) in [
        ("smtp", "skillet-smtp-apply.service"),
        ("ddns", "skillet-ddns-apply.service"),
        ("network", "skillet-network-prepare.service"),
        ("datadog", "skillet-datadog-apply.service"),
    ] {
        if profile.supports_production_service(service) {
            result.extend([Step::Stage(service), Step::Activate(unit)]);
        }
    }
    result
}

pub fn run<A: Actions>(profile: &HostProfile, actions: &mut A) -> Result<(), A::Error> {
    for step in steps(profile) {
        match step {
            Step::Stage(service) => actions.stage(service)?,
            Step::Activate(unit) => actions.activate(unit)?,
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
