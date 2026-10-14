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
            declaration.name() == service && !declaration.credential_consumers().is_empty()
        }) {
            result.push(Step::Stage(service));
        }
    }
    if profile.supports_service("syncthing") {
        result.extend([
            Step::Stage("syncthing"),
            Step::Activate("skillet-syncthing-initialize.service"),
        ]);
    }
    result.push(Step::Activate("skillet-full-apply.service"));
    // UI DNS needs the enrolled device addresses. Monitoring is last so all
    // selected workloads have been activated before the Agent discovers them.
    if !profile.ui_services().is_empty() {
        result.extend([
            Step::Stage("caddy"),
            Step::Activate("skillet-caddy-apply.service"),
        ]);
    }
    for (service, unit) in [
        ("smtp", "skillet-smtp-apply.service"),
        ("ddns", "skillet-ddns-apply.service"),
        ("network", "skillet-network-prepare.service"),
        ("datadog", "skillet-datadog-apply.service"),
    ] {
        if profile.supports_service(service) {
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

/// A skipped condition is not a completed oneshot. Verify actual execution.
pub fn activate(guest: &impl GuestTransport, unit: &str) -> skillet_vm::Result<()> {
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
    let text = std::str::from_utf8(&status.stdout)
        .map_err(|_| skillet_vm::Error::Invalid("invalid apply status".into()))?;
    if !status.status.success()
        || ![
            "LoadState=loaded",
            "ConditionResult=yes",
            "Result=success",
            "ExecMainStatus=0",
        ]
        .iter()
        .all(|required| text.lines().any(|line| line == *required))
        || text
            .lines()
            .find_map(|line| line.strip_prefix("ExecMainStartTimestampMonotonic="))
            .and_then(|value| value.parse::<u64>().ok())
            .is_none_or(|value| value == 0)
    {
        return Err(skillet_vm::Error::Invalid(format!(
            "{unit} did not execute successfully; inspect its journal"
        )));
    }
    Ok(())
}

#[cfg(test)]
#[path = "host_apply/tests.rs"]
mod tests;
