//! CLI presentation and legacy provider wiring. Ownership lives in `skillet_vm`.
use super::{
    butane_root, secret_delivery, workspace_root, VmCreateArgs, VmDestroyArgs, VmDirectoryArgs,
    VmListArgs, VmTargetArgs,
};
use anyhow::{anyhow, Context, Result};
use skillet_vm::{
    backend::{DomainSnapshot, FlatpakVirtInstall, VirshBackend, VmBackend},
    transport::{GuestCommand, GuestTransport, HostKeyPolicy, SshTransport},
    ManifestStore, Phase, RunIdentity, VmRun,
};
use std::io::Write as _;
use std::process::Command;

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

pub(super) fn create(args: &VmCreateArgs) -> Result<()> {
    let butane = butane_root()?;
    let workspace = workspace_root()?;
    let identity = RunIdentity::new(&args.hostname, &args.instance)?;
    if !butane.join(format!("{}.bu", identity.host())).is_file()
        || !workspace
            .join("crates/hosts")
            .join(identity.host())
            .join("Cargo.toml")
            .is_file()
    {
        return Err(anyhow!(
            "host {} needs both its Butane configuration and host crate",
            identity.host()
        ));
    }
    let source_commit = skillet_vm::provisioning::source_revision(&workspace)?;
    let store = ManifestStore::new(&butane.join("runs"), current_uid())?;
    let run_dir = store.run_dir(&identity);
    let _prepared_run = if run_dir.exists() {
        let run = store.load(&identity)?;
        if run.ssh.port != args.port {
            return Err(anyhow!(
                "recorded VM uses SSH port {}; requested port is {}",
                run.ssh.port,
                args.port
            ));
        }
        if matches!(run.phase, Phase::Preparing | Phase::Defined)
            && run.source_commit != source_commit
        {
            return Err(anyhow!(
                "unfinished VM was prepared from a different source revision; inspect or dispose it before retrying"
            ));
        }
        skillet_vm::runtime::ensure_connection(&butane, current_uid(), &run.connection)?;
        run
    } else {
        skillet_vm::provisioning::validate_ssh_port(args.port)?;
        let runtime = skillet_vm::runtime::prepare(&butane, current_uid())?;
        let backend = VirshBackend::new(
            runtime.connection.clone(),
            current_uid(),
            &runtime.virsh_wrapper,
            skillet_vm::backend::ProcessExecutor::default(),
        )?;
        if backend.contains_name(&identity.domain_name())? {
            return Err(anyhow!(
                "refusing unrecorded existing libvirt domain {}",
                identity.domain_name()
            ));
        }
        store.prepare_intent(&identity, runtime.connection, args.port, &source_commit)?
    };
    let _create_lock = store.lock_create(&identity)?;
    let run = store.load(&identity)?;
    if matches!(run.phase, Phase::Preparing | Phase::Defined) && run.source_commit != source_commit
    {
        return Err(anyhow!(
            "unfinished VM was prepared from a different source revision; inspect or dispose it before retrying"
        ));
    }
    skillet_vm::runtime::validate_kvm_access()?;
    if run.phase == Phase::Preparing {
        skillet_vm::provisioning::validate_ssh_port(run.ssh.port)?;
    }

    if run.phase == Phase::Preparing {
        prepare_guest_artifacts(
            &butane,
            &workspace,
            &store,
            &identity,
            args.image.as_deref(),
        )?;
    } else if args.image.is_some() {
        return Err(anyhow!(
            "--image can only be selected for a new or preparing VM"
        ));
    }

    create_recorded_domain(&butane, &store, &identity)?;
    if let Err(error) = ready(&VmTargetArgs {
        hostname: args.hostname.clone(),
        instance: args.instance.clone(),
    }) {
        return Err(anyhow!(
            "VM was started but readiness failed; inspect it with `test-vm {} logs {}` or destroy it with `skillet test vm destroy {} {}`: {error}",
            args.hostname, args.instance, args.hostname, args.instance
        ));
    }
    println!(
        "Ready: {} at giacomo@127.0.0.1:{}; SSH key: {}",
        identity.domain_name(),
        args.port,
        run.ssh.identity.display()
    );
    Ok(())
}

fn prepare_guest_artifacts(
    butane: &std::path::Path,
    workspace: &std::path::Path,
    store: &ManifestStore,
    identity: &RunIdentity,
    requested_image: Option<&std::path::Path>,
) -> Result<()> {
    let host_package = format!("skillet-{}", identity.host());
    let packages = ["skillet", host_package.as_str()];
    let artifacts = skillet_vm::artifacts::build(&skillet_vm::artifacts::BuildRequest {
        workspace,
        packages: &packages,
        target: "x86_64-unknown-linux-musl",
        profile: "release",
    })?;
    let host_binary = artifacts
        .get(&host_package)
        .ok_or_else(|| anyhow!("Cargo did not report {host_package}"))?;
    let generic_binary = artifacts
        .get("skillet")
        .ok_or_else(|| anyhow!("Cargo did not report skillet"))?;
    let image =
        skillet_vm::provisioning::resolve_coreos_image(&butane.join("images"), requested_image)?;
    skillet_vm::provisioning::prepare_local_artifacts(store, identity, &image)?;
    let run = store.load(identity)?;
    skillet_vm::staging::stage_butane_source(
        store,
        &run,
        &butane.join(format!("{}.bu", identity.host())),
        &butane.join("includes"),
        host_binary,
        generic_binary,
        &image,
    )?;
    compile_butane(butane, &store.run_dir(identity), identity)
}

fn compile_butane(
    butane: &std::path::Path,
    run_dir: &std::path::Path,
    identity: &RunIdentity,
) -> Result<()> {
    let compiler = butane.join("bin/butane");
    let source = run_dir
        .join("source")
        .join(format!("{}.bu", identity.host()));
    let output_dir = run_dir.join("ignition");
    let mut command = Command::new(&compiler);
    command
        .args(["-s", "-o"])
        .arg(&output_dir)
        .arg(source)
        .arg(format!("{}.ign", identity.host()));
    let output = skillet_vm::capture_command(command, std::time::Duration::from_mins(30))
        .with_context(|| {
            format!(
                "starting the Butane compiler at {} failed",
                compiler.display()
            )
        })?;
    std::io::stdout().write_all(&output.stdout)?;
    std::io::stderr().write_all(&output.stderr)?;
    if !output.status.success() {
        return Err(anyhow!(
            "Butane compilation failed with status {:?}",
            output.status.code()
        ));
    }
    let run_ignition = run_dir
        .join("ignition")
        .join(format!("{}.ign", identity.host()));
    if !run_ignition.is_file() {
        return Err(anyhow!("Butane did not produce {}", run_ignition.display()));
    }
    Ok(())
}

fn create_recorded_domain(
    butane: &std::path::Path,
    store: &ManifestStore,
    identity: &RunIdentity,
) -> Result<VmRun> {
    let run = store.load(identity)?;
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    let podman_version = skillet_vm::capture_version("podman")?;
    let yq_version = skillet_vm::capture_version("yq")?;
    match run.connection.backend {
        skillet_vm::Backend::Native => {
            let emulator = backend.x86_64_emulator(&run)?;
            let versions = format!(
                "Native VM launcher: virsh XML\n{}{}{}",
                backend.version(&run)?,
                podman_version,
                yq_version
            );
            skillet_vm::creation::create_native(store, identity, &backend, &emulator, &versions)
                .map_err(Into::into)
        }
        skillet_vm::Backend::Flatpak => {
            let creator = FlatpakVirtInstall::for_run(&run, &butane.join("bin/virt-install"))?;
            let versions = format!(
                "Flatpak VM launcher: virt-install\n{}{}{}{}",
                creator.version()?,
                backend.version(&run)?,
                podman_version,
                yq_version
            );
            skillet_vm::creation::create_flatpak(store, identity, &backend, &versions, |run| {
                creator.define(run)
            })
            .map_err(Into::into)
        }
    }
}

pub(super) fn reboot(args: &VmTargetArgs) -> Result<()> {
    let (butane, store, identity) = context(args)?;
    let _lock = store.lock(&identity)?;
    let mut run = store.load(&identity)?;
    if !matches!(run.phase, Phase::Started | Phase::Ready) {
        return Err(anyhow!("VM must be started or ready before reboot"));
    }
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    require_owned_domain(&backend, &run)?;
    run = store.mark_reboot_pending(&identity)?;
    backend.reboot(&run)?;
    require_owned_domain(&backend, &run)?;
    println!("Reboot requested for {}", identity.domain_name());
    Ok(())
}

pub(super) fn ssh(args: &VmTargetArgs) -> Result<()> {
    let (butane, store, identity) = context(args)?;
    let _lock = store.lock(&identity)?;
    let run = store.load(&identity)?;
    if !matches!(run.phase, Phase::Started | Phase::Ready) {
        return Err(anyhow!("VM must be started or ready before opening SSH"));
    }
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    require_running_domain(&backend, &run)?;
    let transport = SshTransport::new(run.ssh.clone(), HostKeyPolicy::Enroll)?;
    let status = transport.interactive()?;
    require_owned_domain(&backend, &run)?;
    if !status.success() {
        return Err(anyhow!(
            "interactive SSH exited with status {:?}",
            status.code()
        ));
    }
    Ok(())
}

pub(super) fn logs(args: &VmTargetArgs) -> Result<()> {
    let (butane, store, identity) = context(args)?;
    let _lock = store.lock(&identity)?;
    let run = store.load(&identity)?;
    if !matches!(run.phase, Phase::Started | Phase::Ready) {
        return Err(anyhow!("VM must be started or ready before reading logs"));
    }
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    require_running_domain(&backend, &run)?;
    let transport = SshTransport::new(run.ssh.clone(), HostKeyPolicy::Enroll)?;
    for command in [
        GuestCommand {
            program: "sudo",
            arguments: &["-n", "rpm-ostree", "status"],
        },
        GuestCommand {
            program: "sudo",
            arguments: &[
                "-n",
                "journalctl",
                "-b",
                "-u",
                "ucore-bootstrap.service",
                "-u",
                "skillet-apply.service",
                "--no-pager",
                "-n",
                "200",
            ],
        },
    ] {
        require_running_domain(&backend, &run)?;
        let output = transport.execute(&command, None)?;
        require_owned_domain(&backend, &run)?;
        std::io::stdout().write_all(&output.stdout)?;
        std::io::stderr().write_all(&output.stderr)?;
        if !output.status.success() {
            return Err(anyhow!(
                "guest diagnostic command {} failed with status {:?}",
                command.program,
                output.status.code()
            ));
        }
    }
    Ok(())
}

fn context(args: &VmTargetArgs) -> Result<(std::path::PathBuf, ManifestStore, RunIdentity)> {
    let butane = butane_root()?;
    let identity = RunIdentity::new(&args.hostname, &args.instance)?;
    let store = ManifestStore::new(&butane.join("runs"), current_uid())?;
    Ok((butane, store, identity))
}

fn require_owned_domain(backend: &impl VmBackend, run: &VmRun) -> Result<DomainSnapshot> {
    let domain = backend
        .inspect(run)?
        .ok_or_else(|| anyhow!("owned VM domain is absent"))?;
    domain.validate_owned(run)?;
    Ok(domain)
}

fn require_running_domain(backend: &impl VmBackend, run: &VmRun) -> Result<DomainSnapshot> {
    let domain = require_owned_domain(backend, run)?;
    if domain.state != "running" {
        return Err(anyhow!(
            "VM is not running (libvirt state: {})",
            domain.state
        ));
    }
    Ok(domain)
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
    let boot = skillet_hosts::boot_policy_for_host(identity.host())
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
