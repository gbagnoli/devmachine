//! Source templates and recorded runs, without importing or mutating them.
use crate::{manifest::reject_symlinks, ManifestStore, Result, RunIdentity};
use std::{collections::BTreeMap, fs, path::Path};

pub fn templates(butane: &Path, workspace: &Path) -> Result<BTreeMap<String, bool>> {
    let mut hosts = BTreeMap::new();
    for entry in fs::read_dir(butane)? {
        let path = entry?.path();
        if path.extension().is_some_and(|extension| extension == "bu") {
            if let Some(host) = path.file_stem().and_then(|name| name.to_str()) {
                if RunIdentity::new(host, "catalog").is_ok() {
                    hosts.insert(host.to_string(), false);
                }
            }
        }
    }
    let crates = workspace.join("crates/hosts");
    if crates.exists() {
        for entry in fs::read_dir(&crates)? {
            let entry = entry?;
            if entry.path().join("Cargo.toml").is_file() {
                if let Some(host) = entry.file_name().to_str() {
                    if RunIdentity::new(host, "catalog").is_ok() {
                        hosts.insert(host.to_string(), false);
                    }
                }
            }
        }
    }
    for (host, available) in &mut hosts {
        *available = butane.join(format!("{host}.bu")).is_file()
            && crates.join(host).join("Cargo.toml").is_file();
    }
    Ok(hosts)
}

pub fn recorded_runs(store: &ManifestStore, host: &str) -> Result<Vec<RunIdentity>> {
    RunIdentity::new(host, "catalog")?;
    let root = store.root();
    reject_symlinks(root)?;
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut runs = Vec::new();
    let prefix = format!("{host}-test-");
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if let Some(instance) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_prefix(&prefix))
        {
            if let Ok(identity) = RunIdentity::new(host, instance) {
                runs.push(identity);
            }
        }
    }
    runs.sort_by(|left, right| left.instance().cmp(right.instance()));
    Ok(runs)
}

#[cfg(test)]
#[path = "catalog/tests.rs"]
mod tests;
