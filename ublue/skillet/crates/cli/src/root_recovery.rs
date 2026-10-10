//! CLI boundaries for production and lifecycle-owned disposable recovery.
use super::{butane_root, workspace_root, RootRecoveryArgs, SecretUnlockArgs, VmRecoveryArgs};
use anyhow::{anyhow, Result};
use skillet_vm::{
    backend::{VirshBackend, VmBackend},
    transport::{GuestTransport, HostKeyPolicy, OwnershipCheckedTransport, SshTransport},
    ManifestStore, Phase, RunIdentity, SshTarget,
};
use skillet_workstation::{
    root_recovery::{self, VaultStore},
    vault::Vault,
};
pub(super) fn production(args: &RootRecoveryArgs) -> Result<()> {
    if skillet_hosts::profile_for_name(&args.hostname).is_none() {
        return Err(anyhow!("unknown host profile"));
    }
    let (user, address) = args
        .target
        .split_once('@')
        .ok_or_else(|| anyhow!("SSH target must be USER@HOST"))?;
    let guest = SshTransport::new(
        SshTarget {
            user: user.into(),
            address: address.into(),
            port: args.port,
            identity: args.identity.clone(),
            known_hosts: args.known_hosts.clone(),
        },
        HostKeyPolicy::Verify,
    )?;
    save(
        &args.hostname,
        &format!("skillet/hosts/{}/storage/root-recovery-key", args.hostname),
        &args.vault,
        &guest,
    )
}
pub(super) fn vm(args: &VmRecoveryArgs) -> Result<()> {
    let butane = butane_root()?;
    let id = RunIdentity::new(&args.target.hostname, &args.target.instance)?;
    let store = ManifestStore::new(&butane.join("runs"), skillet_vm::current_uid())?;
    let _lock = store.lock(&id)?;
    let run = store.load(&id)?;
    if run.phase != Phase::Ready || run.root_profile != skillet_vm::install::RootProfile::Tpm {
        return Err(anyhow!("recovery requires a ready TPM-profile VM"));
    }
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    let ownership = || {
        backend
            .inspect(&run)?
            .ok_or_else(|| skillet_vm::Error::Invalid("owned VM is absent".into()))?
            .validate_owned(&run)
    };
    ownership()?;
    let ssh = SshTransport::new(run.ssh.clone(), HostKeyPolicy::Verify)?;
    let guest = OwnershipCheckedTransport::new(&ssh, &ownership);
    save(
        id.host(),
        &format!(
            "skillet/environments/test/hosts/{}/instances/{}/storage/root-recovery-key",
            id.host(),
            args.target.instance
        ),
        &args.vault,
        &guest,
    )
}
fn save(
    host: &str,
    path: &str,
    args: &SecretUnlockArgs,
    guest: &impl GuestTransport,
) -> Result<()> {
    let database = match &args.database {
        Some(path) => path.clone(),
        None => Vault::default_path()?,
    };
    let mut vault = Vault::open(&database, args.key_file.as_deref())?;
    root_recovery::provision(
        host,
        path,
        &workspace_root()?.join("scripts/root-recovery.sh"),
        &mut VaultStore {
            vault: &mut vault,
            key_file: args.key_file.as_deref(),
        },
        guest,
        root_recovery::generate_key,
    )?;
    println!("Saved and verified recovery passphrase in KeePassXC: {path}");
    Ok(())
}
