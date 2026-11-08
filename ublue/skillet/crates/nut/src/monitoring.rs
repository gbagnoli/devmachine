use crate::{Config, NutError, STATE};
use sha2::{Digest as _, Sha256};
use skillet_core::{
    activation::{activate, ActivationRequest, ConsumerKind},
    files::{FileMutationResource, FileReadResource, Ownership},
    system::ServiceResource,
};
use std::path::Path;

pub const METRICS_TIMER: &str = "skillet-nut-metrics.timer";

fn units(config: Config, socket: &str, binary: &str) -> Vec<skillet_core::vendor_units::Unit> {
    let service = format!("[Unit]\nDescription=Read-only NUT metrics for Datadog\nAfter=nut-server.service datadog-agent.service\nConditionPathExists=/dev/skillet-ups-{}\nConditionPathExists={socket}\n\n[Service]\nType=oneshot\nExecStart={binary} metrics nut --ups {} --dogstatsd-socket {socket}\nTimeoutStartSec=20\nNoNewPrivileges=true\nProtectSystem=strict\nProtectHome=true\nPrivateTmp=true\nProtectKernelTunables=true\nProtectKernelModules=true\nProtectControlGroups=true\nRestrictAddressFamilies=AF_UNIX AF_INET AF_INET6\nCapabilityBoundingSet=\nLockPersonality=true\nUMask=0077\n", config.name, config.name);
    let timer = "[Unit]\nDescription=Poll local UPS metrics\n\n[Timer]\nOnBootSec=1min\nOnCalendar=*-*-* *:*:00,30\nAccuracySec=1s\nUnit=skillet-nut-metrics.service\n\n[Install]\nWantedBy=timers.target\n";
    vec![
        skillet_core::vendor_units::Unit {
            name: "skillet-nut-metrics.service",
            content: service,
        },
        skillet_core::vendor_units::Unit {
            name: METRICS_TIMER,
            content: timer.into(),
        },
    ]
}

/// Static collector definitions; host values remain in a runtime environment file.
pub fn image_units() -> Vec<skillet_core::vendor_units::Unit> {
    vec![
        skillet_core::vendor_units::Unit {
            name: "skillet-nut-metrics.service",
            content: include_str!("nut-metrics.service").into(),
        },
        skillet_core::vendor_units::Unit {
            name: METRICS_TIMER,
            content: include_str!("nut-metrics.timer").into(),
        },
    ]
}

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
    let legacy = units(config, socket, "/var/usrlocal/bin/skillet");
    let vendor = image_units();
    let receipt = skillet_core::vendor_units::LegacyReceipt {
        path: Path::new(STATE).join("metrics.applied"),
        revisions: vec![Sha256::digest(
            legacy
                .iter()
                .map(|unit| unit.content.as_str())
                .collect::<String>(),
        )
        .to_vec()],
    };
    if skillet_core::vendor_units::reconcile(
        system,
        files,
        "nut-metrics",
        &vendor,
        &legacy,
        Some(&receipt),
    )? {
        files.ensure_directory(Path::new("/etc/skillet/nut"), Some(0o700), &root)?;
        let environment = format!(
            "SKILLET_UPS={}\nSKILLET_DOGSTATSD_SOCKET={socket}\n",
            config.name
        );
        let changed = files.ensure_file(
            Path::new("/etc/skillet/nut/metrics.env"),
            environment.as_bytes(),
            Some(0o600),
            &root,
        )?;
        system.service_enable(METRICS_TIMER)?;
        activate(
            system,
            files,
            &ActivationRequest {
                service: METRICS_TIMER.into(),
                state_path: Path::new(STATE).join("metrics.applied"),
                revision: Sha256::digest(format!(
                    "{}{environment}",
                    vendor
                        .iter()
                        .map(|unit| unit.content.as_str())
                        .collect::<String>()
                ))
                .to_vec(),
                definition_changed: changed,
                reload_daemon: false,
                consumer_kind: ConsumerKind::Persistent,
            },
        )?;
        return Ok(());
    }
    let service = &legacy[0].content;
    let timer = &legacy[1].content;
    let mut changed = false;
    for (unit, content) in [
        ("skillet-nut-metrics.service", service.as_str()),
        (METRICS_TIMER, timer.as_str()),
    ] {
        changed |= files.ensure_file(
            &Path::new("/etc/systemd/system").join(unit),
            content.as_bytes(),
            Some(0o644),
            &root,
        )?;
    }
    if changed {
        system.service_reenable(METRICS_TIMER)?;
    } else {
        system.service_enable(METRICS_TIMER)?;
    }
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
