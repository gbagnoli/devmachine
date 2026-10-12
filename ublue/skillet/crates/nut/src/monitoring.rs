use crate::{Config, NutError, STATE};
use sha2::{Digest as _, Sha256};
use skillet_core::{
    activation::{activate, ActivationRequest, ConsumerKind},
    files::{FileMutationResource, FileReadResource, Ownership},
    system::ServiceResource,
};
use std::path::Path;

pub const METRICS_TIMER: &str = "skillet-nut-metrics.timer";

/// Called only after successful credential-gated Datadog activation.
pub fn apply_metrics<S, F>(
    system: &S,
    files: &F,
    config: Config,
    socket: &Path,
) -> Result<(), NutError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    config.validate()?;
    let Some(socket) = socket.to_str() else {
        return Err(NutError::InvalidConfig);
    };
    if !socket.starts_with('/')
        || !socket
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'))
    {
        return Err(NutError::InvalidConfig);
    }
    let root = Ownership::named(Some("root"), Some("root"));
    files.ensure_directory(Path::new(STATE), Some(0o700), &root)?;
    files.ensure_directory(Path::new("/etc/systemd/system"), Some(0o755), &root)?;
    let service = format!("[Unit]\nDescription=Read-only NUT metrics for Datadog\nAfter=nut-server.service datadog-agent.service\nConditionPathExists=/dev/skillet-ups-{}\nConditionPathExists={socket}\n\n[Service]\nType=oneshot\nExecStart=/usr/bin/skillet metrics nut --ups {} --dogstatsd-socket {socket}\nTimeoutStartSec=20\nNoNewPrivileges=true\nProtectSystem=strict\nProtectHome=true\nPrivateTmp=true\nProtectKernelTunables=true\nProtectKernelModules=true\nProtectControlGroups=true\nRestrictAddressFamilies=AF_UNIX AF_INET AF_INET6\nCapabilityBoundingSet=\nLockPersonality=true\nUMask=0077\n", config.name, config.name);
    let timer = "[Unit]\nDescription=Poll local UPS metrics\n\n[Timer]\nOnBootSec=1min\nOnCalendar=*-*-* *:*:00,30\nAccuracySec=1s\nUnit=skillet-nut-metrics.service\n\n[Install]\nWantedBy=timers.target\n";
    let mut changed = false;
    for (unit, content) in [
        ("skillet-nut-metrics.service", service.as_str()),
        (METRICS_TIMER, timer),
    ] {
        changed |= files.ensure_file(
            &Path::new("/etc/systemd/system").join(unit),
            content.as_bytes(),
            Some(0o644),
            &root,
        )?;
    }
    system.service_enable(METRICS_TIMER)?;
    activate(
        system,
        files,
        &ActivationRequest {
            service: METRICS_TIMER.into(),
            state_path: Path::new(STATE).join("metrics.applied"),
            revision: Sha256::digest(format!("{service}{timer}")).to_vec(),
            definition_changed: changed,
            reload_daemon: true,
            consumer_kind: ConsumerKind::Persistent,
        },
    )?;
    Ok(())
}
