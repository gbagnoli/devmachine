//! Negotiated physical Ethernet link state, independent of traffic throughput.
use crate::DatadogError;
use skillet_core::{
    activation::{activate, ActivationRequest, ConsumerKind},
    files::{FileMutationResource, FileReadResource, Ownership},
    system::ServiceResource,
};
use std::{fs, io, path::Path};

pub const TIMER: &str = "skillet-nic-metrics.timer";
pub const UNIT: &str = "skillet-nic-metrics.service";

#[derive(Debug)]
pub struct Sample {
    pub interface: String,
    pub link_up: Option<bool>,
    pub speed_mbps: Option<u32>,
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 15
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
}

fn carrier(path: &Path) -> Option<bool> {
    match fs::read_to_string(path.join("carrier")).ok()?.trim() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

/// Sysfs root is injected for deterministic hardware-free tests. Virtual and
/// wireless interfaces have no physical Ethernet negotiated speed to report.
pub fn collect(root: &Path) -> io::Result<Vec<Sample>> {
    let mut samples = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str().filter(|name| valid_name(name)) else {
            continue;
        };
        let path = entry.path();
        if !path.join("device").exists()
            || path.join("wireless").exists()
            || path.join("phy80211").exists()
        {
            continue;
        }
        let kind = fs::read_to_string(path.join("type"));
        if kind.as_ref().is_ok_and(|kind| kind.trim() != "1") {
            continue;
        }
        if samples.len() >= 256 {
            return Err(io::Error::other("too many hardware network interfaces"));
        }
        let mut link_up = kind.ok().and_then(|_| carrier(&path));
        let mut speed_mbps = match link_up {
            Some(false) => Some(0),
            Some(true) => fs::read_to_string(path.join("speed"))
                .ok()
                .and_then(|speed| speed.trim().parse::<u32>().ok())
                .filter(|speed| *speed > 0),
            None => None,
        };
        if carrier(&path) != link_up {
            link_up = None;
            speed_mbps = None;
        }
        samples.push(Sample {
            interface: name.into(),
            link_up,
            speed_mbps,
        });
    }
    samples.sort_by(|a, b| a.interface.cmp(&b.interface));
    Ok(samples)
}

pub fn render(samples: &[Sample]) -> Vec<String> {
    let mut packets = Vec::new();
    for sample in samples
        .iter()
        .filter(|sample| valid_name(&sample.interface))
    {
        let tags = format!("|g|#interface:{}", sample.interface);
        packets.push(format!(
            "skillet.net.collection_success:{}{tags}",
            u8::from(sample.link_up.is_some() && sample.speed_mbps.is_some())
        ));
        if let Some(up) = sample.link_up {
            packets.push(format!("skillet.net.link_up:{}{tags}", u8::from(up)));
        }
        if let Some(speed) = sample.speed_mbps {
            packets.push(format!("skillet.net.link_speed_mbps:{speed}{tags}"));
        }
    }
    packets
}

pub fn image_units() -> Vec<skillet_core::vendor_units::Unit> {
    vec![
        skillet_core::vendor_units::Unit {
            name: UNIT,
            content: format!("[Unit]\nDescription=Physical Ethernet link metrics for Datadog\nAfter=datadog-agent.service\nConditionPathExists={}\n[Service]\nType=oneshot\nExecStart=/usr/bin/skillet metrics nic\nTimeoutStartSec=20\nNoNewPrivileges=yes\nProtectSystem=strict\nProtectHome=yes\nPrivateTmp=yes\nProtectKernelTunables=yes\nProtectControlGroups=yes\nRestrictAddressFamilies=AF_UNIX\nCapabilityBoundingSet=\n", crate::DOGSTATSD_SOCKET),
        },
        skillet_core::vendor_units::Unit {
            name: TIMER,
            content: "[Unit]\nDescription=Poll physical Ethernet links\n[Timer]\nOnBootSec=2min\nOnCalendar=*-*-* *:*:00,30\nAccuracySec=1s\n[Install]\nWantedBy=timers.target\n".into(),
        },
    ]
}

/// Called only after the host's Datadog Agent has converged successfully.
pub fn install<S, F>(system: &S, files: &F) -> Result<(), DatadogError>
where
    S: ServiceResource + ?Sized,
    F: FileMutationResource + FileReadResource + ?Sized,
{
    let root = Ownership::named(Some("root"), Some("root"));
    let units = image_units();
    files.ensure_directory(
        Path::new("/var/lib/skillet/nic-metrics"),
        Some(0o700),
        &root,
    )?;
    let vendor =
        skillet_core::vendor_units::reconcile(system, files, "nic-metrics", &units, &units, None)?;
    let mut changed = false;
    if !vendor {
        files.ensure_directory(Path::new("/etc/systemd/system"), Some(0o755), &root)?;
        for unit in &units {
            changed |= files.ensure_file(
                &Path::new("/etc/systemd/system").join(unit.name),
                unit.content.as_bytes(),
                Some(0o644),
                &root,
            )?;
        }
    }
    system.service_enable(TIMER)?;
    activate(
        system,
        files,
        &ActivationRequest {
            service: TIMER.into(),
            state_path: "/var/lib/skillet/nic-metrics/timer.applied".into(),
            revision: units
                .iter()
                .flat_map(|unit| unit.content.as_bytes())
                .copied()
                .collect(),
            definition_changed: changed,
            reload_daemon: true,
            consumer_kind: ConsumerKind::Persistent,
        },
    )?;
    Ok(())
}

#[cfg(test)]
#[path = "nic/tests.rs"]
mod tests;
