//! CLI presentation and legacy provider wiring. Ownership lives in `skillet_vm`.
use super::{
    butane_root, secret_delivery, workspace_root, VmDestroyArgs, VmDirectoryArgs, VmListArgs,
    VmPrepareLocalArgs, VmStageLocalArgs, VmTargetArgs,
};
use anyhow::{anyhow, Result};
use skillet_vm::{
    backend::{VirshBackend, VmBackend},
    ManifestStore, RunIdentity,
};

pub(super) fn status(args: &VmTargetArgs) -> Result<()> {
    let butane = butane_root()?;
    let identity = RunIdentity::new(&args.hostname, &args.instance)?;
    let store = ManifestStore::new(&butane.join("runs"), current_uid())?;
    let run = store.load(&identity)?;
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    let domain = backend.inspect(&run)?.ok_or_else(|| {
        anyhow!(
            "owned domain is absent; recovery metadata remains at {}",
            store.run_dir(&identity).display()
        )
    })?;
    domain.validate_owned(&run)?;
    println!(
        "Name: {}\nUUID: {}\nState: {}\nSSH: {}@{}:{}",
        domain.name, domain.uuid, domain.state, run.ssh.user, run.ssh.address, run.ssh.port
    );
    for disk in domain.disks {
        println!("Disk: {}", disk.display());
    }
    Ok(())
}

pub(super) fn record_started(args: &VmTargetArgs) -> Result<()> {
    let butane = butane_root()?;
    let identity = RunIdentity::new(&args.hostname, &args.instance)?;
    let store = ManifestStore::new(&butane.join("runs"), current_uid())?;
    let _lock = store.lock(&identity)?;
    let run = store.load(&identity)?;
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    backend
        .inspect(&run)?
        .ok_or_else(|| anyhow!("VM creation returned without an owned domain"))?
        .validate_owned(&run)?;
    store.mark_started(&identity)?;
    Ok(())
}

pub(super) fn prepare_local(args: &VmPrepareLocalArgs) -> Result<()> {
    let butane = butane_root()?;
    let identity = RunIdentity::new(&args.hostname, &args.instance)?;
    let store = ManifestStore::new(&butane.join("runs"), current_uid())?;
    let public_key =
        skillet_vm::provisioning::prepare_local_artifacts(&store, &identity, &args.image)?;
    println!("{public_key}");
    Ok(())
}

pub(super) fn stage_local(args: &VmStageLocalArgs) -> Result<()> {
    let butane = butane_root()?;
    let identity = RunIdentity::new(&args.hostname, &args.instance)?;
    let store = ManifestStore::new(&butane.join("runs"), current_uid())?;
    let run = store.load(&identity)?;
    skillet_vm::staging::stage_butane_source(
        &store,
        &run,
        &butane.join(format!("{}.bu", identity.host())),
        &butane.join("includes"),
        &args.host_binary,
        &args.generic_binary,
        &args.image,
    )?;
    Ok(())
}

pub(super) fn create_native(args: &VmTargetArgs) -> Result<()> {
    let butane = butane_root()?;
    let identity = RunIdentity::new(&args.hostname, &args.instance)?;
    let store = ManifestStore::new(&butane.join("runs"), current_uid())?;
    let run = store.load(&identity)?;
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    let emulator = backend.x86_64_emulator(&run)?;
    let versions = format!(
        "Native VM launcher: virsh XML\n{}{}{}",
        backend.version(&run)?,
        skillet_vm::capture_version("podman")?,
        skillet_vm::capture_version("yq")?,
    );
    let run =
        skillet_vm::creation::create_native(&store, &identity, &backend, &emulator, &versions)?;
    println!("Started {} ({})", identity.domain_name(), run.uuid);
    Ok(())
}

pub(super) fn destroy(args: &VmDestroyArgs) -> Result<()> {
    let butane = butane_root()?;
    let identity = RunIdentity::new(&args.hostname, &args.instance)?;
    let store = ManifestStore::new(&butane.join("runs"), current_uid())?;
    let run = store.load(&identity)?;
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    skillet_vm::lifecycle::destroy(&store, &identity, &backend, |_| {
        secret_delivery::remove_vm_external_resources(args)
            .map_err(|error| skillet_vm::Error::ExternalCleanup(error.to_string()))
    })?;
    println!("Disposed {}", identity.domain_name());
    Ok(())
}

fn current_uid() -> u32 {
    skillet_vm::current_uid()
}

pub(super) fn ready_directory(args: &VmDirectoryArgs) -> Result<()> {
    let butane = butane_root()?;
    let store = ManifestStore::new(&butane.join("runs"), current_uid())?;
    let run = store.load_directory(&args.run_dir)?;
    ready(&VmTargetArgs {
        hostname: run.identity.host().into(),
        instance: run.identity.instance().into(),
    })
}

pub(super) fn ready(args: &VmTargetArgs) -> Result<()> {
    let butane = butane_root()?;
    let identity = RunIdentity::new(&args.hostname, &args.instance)?;
    let store = ManifestStore::new(&butane.join("runs"), current_uid())?;
    let _lock = store.lock(&identity)?;
    let run = store.load(&identity)?;
    let boot = skillet_cli_common::hosts::boot_policy_for_host(identity.host())
        .ok_or_else(|| anyhow!("unknown readiness profile: {}", identity.host()))?;
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    backend
        .inspect(&run)?
        .ok_or_else(|| anyhow!("owned domain is absent"))?
        .validate_owned(&run)?;
    let mut run = store.import(&identity)?;
    let ownership_run = run.clone();
    let mut probe = skillet_vm::transport::SshTransport::new(
        run.ssh.clone(),
        skillet_vm::transport::HostKeyPolicy::Enroll,
    )?;
    probe.timeout = std::time::Duration::from_secs(15);
    let operations = skillet_vm::transport::SshTransport::new(
        run.ssh.clone(),
        skillet_vm::transport::HostKeyPolicy::Enroll,
    )?;
    let policy = skillet_vm::readiness::ReadinessPolicy {
        signed_image: boot.signed_image,
        resolver_target: "/run/NetworkManager/resolv.conf".into(),
        masked_units: boot.masked_units.into_iter().map(String::from).collect(),
        phase_timeout: std::time::Duration::from_mins(45),
    };
    skillet_vm::readiness::ready(
        &mut run,
        &store,
        &store.run_dir(&identity),
        &policy,
        &skillet_vm::readiness::ReadinessIo {
            probe: &probe,
            operations: &operations,
            ownership: &|| {
                backend
                    .inspect(&ownership_run)?
                    .ok_or_else(|| {
                        skillet_vm::Error::Invalid(
                            "owned domain disappeared during readiness".into(),
                        )
                    })?
                    .validate_owned(&ownership_run)
            },
        },
        &skillet_vm::readiness::MonotonicClock::default(),
        &mut |message| tracing::info!("{message}"),
    )?;
    println!(
        "Ready: {} on {}; SSH: {}@{}:{}",
        identity.domain_name(),
        run.connection.uri,
        run.ssh.user,
        run.ssh.address,
        run.ssh.port
    );
    Ok(())
}

pub(super) fn update(args: &VmDestroyArgs) -> Result<()> {
    let butane = butane_root()?;
    let identity = RunIdentity::new(&args.hostname, &args.instance)?;
    let store = ManifestStore::new(&butane.join("runs"), current_uid())?;
    let _lock = store.lock(&identity)?;
    let mut run = store.load(&identity)?;
    if !matches!(
        run.phase,
        skillet_vm::Phase::Started | skillet_vm::Phase::Ready
    ) {
        return Err(anyhow!(
            "finish VM creation or disposal before updating its binaries"
        ));
    }
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    backend
        .inspect(&run)?
        .ok_or_else(|| anyhow!("owned domain is absent"))?
        .validate_owned(&run)?;
    let root = workspace_root()?;
    let package = format!("skillet-{}", identity.host());
    let artifacts = skillet_vm::artifacts::build(&skillet_vm::artifacts::BuildRequest {
        workspace: &root,
        packages: &["skillet", &package],
        target: "x86_64-unknown-linux-musl",
        profile: "release",
    })?;
    let host = artifacts
        .get(&package)
        .ok_or_else(|| anyhow!("Cargo did not report {package}"))?;
    let generic = artifacts
        .get("skillet")
        .ok_or_else(|| anyhow!("Cargo did not report skillet"))?;
    let transport = skillet_vm::transport::SshTransport::new(
        run.ssh.clone(),
        skillet_vm::transport::HostKeyPolicy::Verify,
    )?;
    run = store.import(&identity)?;
    backend
        .inspect(&run)?
        .ok_or_else(|| anyhow!("owned domain disappeared during the build"))?
        .validate_owned(&run)?;
    let hashes = skillet_vm::delivery::deliver(&run, &transport, host, generic)?;
    run.deployed = Some(hashes);
    store.save(&run)?;
    println!(
        "Updated skillet and {package} in {}",
        identity.domain_name()
    );
    Ok(())
}

pub(super) fn list(args: &VmListArgs) -> Result<()> {
    let butane = butane_root()?;
    let store = ManifestStore::new(&butane.join("runs"), current_uid())?;
    let mut templates = skillet_vm::catalog::templates(&butane, &workspace_root()?)?;
    if let Some(host) = &args.hostname {
        RunIdentity::new(host, "catalog")?;
        let available = templates.get(host).copied();
        if available.is_none() && skillet_vm::catalog::recorded_runs(&store, host)?.is_empty() {
            return Err(anyhow!("unknown host: {host}"));
        }
        templates = std::collections::BTreeMap::from([(host.clone(), available.unwrap_or(false))]);
    }
    println!(
        "{:<12} {:<12} {:<16} {:<12} PORT",
        "HOST", "TEMPLATE", "INSTANCE", "STATE"
    );
    for (host, available) in templates {
        let availability = if available {
            "available"
        } else {
            "unavailable"
        };
        let runs = skillet_vm::catalog::recorded_runs(&store, &host)?;
        if runs.is_empty() {
            println!("{host:<12} {availability:<12} {:<16} {:<12} -", "-", "none");
        }
        for identity in runs {
            let (state, port) = match store.load(&identity) {
                Err(_) => ("invalid".into(), "-".into()),
                Ok(run) => {
                    let result = VirshBackend::for_run(&run, &butane.join("bin/virsh"))
                        .and_then(|backend| backend.inspect(&run));
                    let state = match result {
                        Ok(None) => "missing".into(),
                        Ok(Some(domain)) if domain.validate_owned(&run).is_ok() => domain.state,
                        Ok(Some(_)) => "mismatch".into(),
                        Err(_) => "unavailable".into(),
                    };
                    (state, run.ssh.port.to_string())
                }
            };
            println!(
                "{host:<12} {availability:<12} {:<16} {state:<12} {port}",
                identity.instance()
            );
        }
    }
    Ok(())
}
