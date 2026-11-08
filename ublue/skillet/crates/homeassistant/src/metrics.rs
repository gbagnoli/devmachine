//! Module-owned backup freshness, independent of the monitored application.
use crate::Error;
use serde::{Deserialize, Serialize};
use skillet_core::{
    files::{FileMutationResource, FileReadResource, Ownership},
    system::ServiceResource,
};
use std::path::Path;
pub const TIMER: &str = "skillet-homeassistant-metrics.timer";
pub const STATE: &str = "/var/lib/skillet/homeassistant/backup-state.json";
#[derive(Serialize, Deserialize)]
pub struct State {
    pub completed: u64,
    pub exported: u64,
}
pub fn render(now: u64, configured: bool, state: Option<&State>) -> Vec<String> {
    let mut packets = vec![format!(
        "homeassistant.backup.configured:{}|g|#service:homeassistant",
        u8::from(configured)
    )];
    if !configured {
        return packets;
    }
    let valid = state.filter(|s| s.completed <= now && s.exported <= now);
    let healthy =
        valid.is_some_and(|s| now - s.completed <= 36 * 3600 && now - s.exported <= 36 * 3600);
    packets.push(format!(
        "_sc|homeassistant.backup.fresh|{}|#service:homeassistant",
        if healthy { 0 } else { 2 }
    ));
    if let Some(s) = valid {
        packets.extend([
            format!(
                "homeassistant.backup.completed_age_seconds:{}|g|#service:homeassistant",
                now - s.completed
            ),
            format!(
                "homeassistant.backup.export_age_seconds:{}|g|#service:homeassistant",
                now - s.exported
            ),
        ]);
    }
    packets
}
pub fn image_units() -> Vec<skillet_core::vendor_units::Unit> {
    let mut definitions = units("/usr/bin/skillet", Path::new("${SKILLET_DOGSTATSD_SOCKET}"));
    definitions[0].content = definitions[0].content.replace(
        "Type=oneshot\n",
        "Type=oneshot\nEnvironmentFile=/etc/skillet/homeassistant-metrics.env\n",
    );
    definitions
}

fn units(binary: &str, socket: &Path) -> Vec<skillet_core::vendor_units::Unit> {
    let unit=format!("[Unit]\nDescription=Home Assistant backup freshness metrics\nAfter=datadog-agent.service\nRequiresMountsFor=/var/lib/data\n[Service]\nType=oneshot\nExecStart={binary} metrics homeassistant --dogstatsd-socket {}\nTimeoutStartSec=15\nNoNewPrivileges=yes\nProtectSystem=strict\nProtectHome=yes\nPrivateTmp=yes\n",socket.display());
    let timer="[Unit]\nDescription=Collect Home Assistant backup freshness\n[Timer]\nOnBootSec=2min\nOnUnitActiveSec=5min\n[Install]\nWantedBy=timers.target\n";
    vec![
        skillet_core::vendor_units::Unit {
            name: "skillet-homeassistant-metrics.service",
            content: unit,
        },
        skillet_core::vendor_units::Unit {
            name: TIMER,
            content: timer.into(),
        },
    ]
}

pub fn apply<S, F>(system: &S, files: &F, socket: &Path) -> Result<(), Error>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let root = Ownership::named(Some("root"), Some("root"));
    let socket_text = socket
        .to_str()
        .filter(|value| {
            value.starts_with('/')
                && value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
        })
        .ok_or(Error::Invalid)?;
    let definitions = units("/var/usrlocal/bin/skillet", socket);
    let vendor = image_units();
    if skillet_core::vendor_units::reconcile(
        system,
        files,
        "homeassistant-metrics",
        &vendor,
        &definitions,
        None,
    )? {
        files.ensure_directory(Path::new("/etc/skillet"), Some(0o755), &root)?;
        files.ensure_file(
            Path::new("/etc/skillet/homeassistant-metrics.env"),
            format!("SKILLET_DOGSTATSD_SOCKET={socket_text}\n").as_bytes(),
            Some(0o600),
            &root,
        )?;
        system.service_enable(TIMER)?;
        system.service_start(TIMER)?;
        return Ok(());
    }
    let unit = &definitions[0].content;
    let timer = &definitions[1].content;
    let a = files.ensure_file(
        Path::new("/etc/systemd/system/skillet-homeassistant-metrics.service"),
        unit.as_bytes(),
        Some(0o644),
        &root,
    )?;
    let b = files.ensure_file(
        &Path::new("/etc/systemd/system").join(TIMER),
        timer.as_bytes(),
        Some(0o644),
        &root,
    )?;
    if a || b {
        system.daemon_reload()?;
        system.service_reenable(TIMER)?;
    } else {
        system.service_enable(TIMER)?;
    }
    system.service_start(TIMER)?;
    Ok(())
}
#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;
