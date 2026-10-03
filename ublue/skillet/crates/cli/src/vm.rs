//! CLI presentation and legacy provider wiring. Ownership lives in `skillet_vm`.
use super::{
    butane_root, secret_delivery, workspace_root, VmDestroyArgs, VmListArgs, VmTargetArgs,
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
