//! CLI presentation and legacy provider wiring. Ownership lives in `skillet_vm`.
use super::{butane_root, secret_delivery, VmDestroyArgs, VmTargetArgs};
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
