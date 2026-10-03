//! Native VM definition and start orchestration with ownership recovery.
use crate::{
    backend::{DomainSnapshot, VmBackend},
    domain_xml, Backend, Error, ManifestStore, Phase, Result, RunIdentity, VmRun,
};
use std::{
    fs::{self, File},
    io::Write as _,
    os::unix::fs::PermissionsExt as _,
    path::Path,
};

/// Define and start a native VM, persisting ownership after each successful
/// external transition so a retry can inspect and continue safely.
pub fn create_native(
    store: &ManifestStore,
    identity: &RunIdentity,
    backend: &impl VmBackend,
    emulator: &Path,
    tool_versions: &str,
) -> Result<VmRun> {
    create_with(
        store,
        identity,
        backend,
        Backend::Native,
        tool_versions,
        |run| {
            let xml = domain_xml::write_native_domain_xml(store, run, emulator)?;
            backend.define(run, &xml)
        },
    )
}

/// Create a Flatpak-backed guest through the caller's focused virt-install
/// adapter, then use the same ownership and recovery sequence as native VM
/// creation. `virt-install` may start the guest as part of definition.
pub fn create_flatpak(
    store: &ManifestStore,
    identity: &RunIdentity,
    backend: &impl VmBackend,
    tool_versions: &str,
    define: impl FnOnce(&VmRun) -> Result<()>,
) -> Result<VmRun> {
    create_with(
        store,
        identity,
        backend,
        Backend::Flatpak,
        tool_versions,
        define,
    )
}

fn create_with(
    store: &ManifestStore,
    identity: &RunIdentity,
    backend: &impl VmBackend,
    expected_backend: Backend,
    tool_versions: &str,
    define: impl FnOnce(&VmRun) -> Result<()>,
) -> Result<VmRun> {
    let _lock = store.lock(identity)?;
    let mut run = store.load(identity)?;
    store.validate(&run, identity)?;
    if run.connection.backend != expected_backend
        || !matches!(
            run.phase,
            Phase::Preparing | Phase::Defined | Phase::Started | Phase::Ready
        )
    {
        return Err(Error::Invalid(
            "VM creation requires an active run for the selected backend".into(),
        ));
    }
    save_tool_versions(store, &run, tool_versions)?;

    let mut domain = backend.inspect(&run)?;
    if let Some(existing) = &domain {
        existing.validate_owned(&run)?;
        if run.phase == Phase::Preparing {
            run = store.mark_defined(identity)?;
        }
    } else {
        if run.phase != Phase::Preparing {
            return Err(Error::Invalid(
                "recorded native VM domain is absent; refusing to recreate it".into(),
            ));
        }
        define(&run)?;
        domain = backend.inspect(&run)?;
        let defined = owned_domain(domain.as_ref(), &run)?;
        if run.phase == Phase::Preparing {
            run = store.mark_defined(identity)?;
        }
        domain = Some(defined.clone());
    }

    let domain = owned_domain(domain.as_ref(), &run)?;
    if domain.state == "shut off" {
        backend.start(&run)?;
    } else if domain.state != "running" {
        return Err(Error::Invalid(format!(
            "cannot start VM in libvirt state: {}",
            domain.state
        )));
    }

    let running = backend
        .inspect(&run)?
        .ok_or_else(|| Error::Invalid("VM disappeared after start".into()))?;
    running.validate_owned(&run)?;
    if running.state != "running" {
        return Err(Error::Invalid(format!(
            "VM start returned in libvirt state: {}",
            running.state
        )));
    }
    save_domain_uuid(store, &run)?;
    if matches!(run.phase, Phase::Preparing | Phase::Defined) {
        run = store.mark_started(identity)?;
    }
    Ok(run)
}

fn owned_domain<'a>(domain: Option<&'a DomainSnapshot>, run: &VmRun) -> Result<&'a DomainSnapshot> {
    let domain =
        domain.ok_or_else(|| Error::Invalid("VM definition returned without a domain".into()))?;
    domain.validate_owned(run)?;
    Ok(domain)
}

fn save_tool_versions(store: &ManifestStore, run: &VmRun, content: &str) -> Result<()> {
    if content.trim().is_empty() {
        return Err(Error::Invalid("tool version record cannot be empty".into()));
    }
    save_private_file(store, run, "tool-versions.txt", content.as_bytes())
}

fn save_domain_uuid(store: &ManifestStore, run: &VmRun) -> Result<()> {
    save_private_file(
        store,
        run,
        "domain.uuid",
        format!("{}\n", run.uuid).as_bytes(),
    )
}

fn save_private_file(store: &ManifestStore, run: &VmRun, name: &str, content: &[u8]) -> Result<()> {
    store.validate(run, &run.identity)?;
    let dir = store.run_dir(&run.identity);
    let path = dir.join(name);
    crate::manifest::reject_symlinks(&path)?;
    let mut output = tempfile::NamedTempFile::new_in(&dir)?;
    output
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    output.write_all(content)?;
    output.as_file().sync_all()?;
    output
        .persist(&path)
        .map_err(|error| Error::Io(error.error))?;
    File::open(dir)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
#[path = "creation_tests.rs"]
mod tests;
