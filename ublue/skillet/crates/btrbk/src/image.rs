//! Pure vendor definitions selected by the host caller.
use super::{render_config, sources, timer_unit, BtrbkConfig, BtrbkError};
pub const MARKER: &str = "/usr/share/skillet/btrbk/ownership";
pub const CONFIG: &str = "/usr/share/skillet/btrbk/btrbk.conf";
pub const SERVICE: &str = "/usr/lib/systemd/system/skillet-btrbk.service";
pub const TIMER: &str = "/usr/lib/systemd/system/skillet-btrbk.timer";

pub struct Artifact {
    pub path: &'static str,
    pub content: String,
}

pub fn render(config: &BtrbkConfig, binary: &str) -> Result<Vec<Artifact>, BtrbkError> {
    if config.snapshot_subvolumes.is_empty() {
        return Ok(Vec::new());
    }
    if std::path::Path::new(binary).parent() != Some(std::path::Path::new("/usr/bin"))
        || binary.ends_with("/.")
        || binary.ends_with("/..")
        || !binary
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
    {
        return Err(BtrbkError::Conflict(
            "invalid image helper executable".into(),
        ));
    }
    Ok(vec![
        Artifact { path: MARKER, content: "1\n".into() },
        Artifact { path: CONFIG, content: render_config(&sources(config)?) },
        Artifact { path: SERVICE, content: format!("[Unit]\nDescription=Create configured Btrfs snapshots with btrbk\nRequires=var-lib-data.mount\nAfter=var-lib-data.mount\nBindsTo=var-lib-data.mount\nAssertPathIsMountPoint=/var/lib/data\n[Service]\nType=oneshot\nExecStartPre={binary} btrbk prepare\nExecStart=/usr/bin/btrbk --config {CONFIG} run\n") },
        Artifact { path: TIMER, content: timer_unit().into() },
    ])
}
