use super::Result;
use serde_json::json;
use yaml_serde::Value;

pub(super) fn configure(config: &mut Value, primary: &str, secondary: &str) -> Result<()> {
    config["boot_device"] = yaml_serde::to_value(json!({
        "layout": "x86_64",
        "mirror": {"devices": [primary, secondary]},
    }))?;
    config["storage"]["disks"] = yaml_serde::to_value(json!([
        {"device": primary, "partitions": [
            {"label": "root-1", "size_mib": 32768}, {"label": "skillet-data-1"}
        ]},
        {"device": secondary, "partitions": [
            {"label": "root-2", "size_mib": 32768}, {"label": "skillet-data-2"}
        ]}
    ]))?;
    config["storage"]["filesystems"] = yaml_serde::to_value(json!([{
        "device": "/dev/disk/by-partlabel/skillet-data-1",
        "format": "btrfs",
        "label": "skillet-data",
        "wipe_filesystem": true,
        "options": ["-f", "-d", "raid1", "-m", "raid1", "/dev/disk/by-partlabel/skillet-data-2"]
    }]))?;
    let includes = config["ignition"]["config"]["merge"]
        .as_sequence_mut()
        .ok_or_else(|| super::Error::Invalid("missing storage include".into()))?;
    let include = includes
        .iter_mut()
        .find(|entry| entry["local"].as_str() == Some("includes/data-storage.bu"))
        .ok_or_else(|| super::Error::Invalid("missing storage include".into()))?;
    include["local"] = Value::String("includes/mirrored-data.bu".into());
    Ok(())
}
