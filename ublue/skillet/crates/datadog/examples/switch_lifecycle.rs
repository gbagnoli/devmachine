//! Disposable-guest-only acceptance driver; inputs must contain dummy credentials.
use skillet_core::{files::LocalFileResource, system::LinuxSystemResource};
use skillet_datadog::{self as runtime, RuntimeConfig};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("expected fixture JSON path")?;
    let input = std::fs::read_to_string(path)?;
    runtime::apply(
        &LinuxSystemResource::new(),
        &LocalFileResource::new(),
        &input,
        &RuntimeConfig {
            hostname: "switch-lifecycle-fixture",
            network_monitoring: false,
            dogstatsd_loopback_udp: false,
            monitored_units: &["sshd.service".into()],
        },
    )?;
    Ok(())
}
