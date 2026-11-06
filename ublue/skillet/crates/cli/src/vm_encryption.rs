//! Named disposable encryption acceptance, guarded by the VM lifecycle owner.
use super::{butane_root, vm, workspace_root, VmTargetArgs};
use anyhow::{anyhow, Result};
use skillet_vm::{
    backend::{VirshBackend, VmBackend},
    transport::{
        GuestCommand, GuestTransport, HostKeyPolicy, OwnershipCheckedTransport, SshTransport,
    },
    ManifestStore, Phase, RunIdentity,
};
use std::{
    fs,
    io::{Read as _, Write as _},
    os::unix::fs::PermissionsExt as _,
};
use zeroize::Zeroizing;

pub(super) fn check(args: &VmTargetArgs) -> Result<()> {
    let butane = butane_root()?;
    let identity = RunIdentity::new(&args.hostname, &args.instance)?;
    let store = ManifestStore::new(&butane.join("runs"), skillet_vm::current_uid())?;
    let _lock = store.lock(&identity)?;
    let mut run = store.load(&identity)?;
    if run.root_profile != skillet_vm::install::RootProfile::Tpm || run.phase != Phase::Ready {
        return Err(anyhow!(
            "encryption acceptance requires a ready TPM-profile VM"
        ));
    }
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    let ownership = || {
        backend
            .inspect(&run)?
            .ok_or_else(|| skillet_vm::Error::Invalid("owned encryption VM is absent".into()))?
            .validate_owned(&run)
    };
    ownership()?;
    let ssh = SshTransport::new(run.ssh.clone(), HostKeyPolicy::Verify)?;
    let transport = OwnershipCheckedTransport::new(&ssh, &ownership);
    let script = "/var/tmp/skillet-tpm-root-test.sh";
    transport.upload(
        &workspace_root()?.join("integration_tests/tpm-root.sh"),
        script,
    )?;
    let private = store.run_dir(&identity).join("recovery");
    store.validate(&run, &identity)?;
    fs::create_dir_all(&private)?;
    fs::set_permissions(&private, fs::Permissions::from_mode(0o700))?;
    let key_path = private.join("root-recovery.key");
    if !key_path.exists() {
        let mut key = Zeroizing::new(vec![0u8; 64]);
        fs::File::open("/dev/urandom")?.read_exact(&mut key)?;
        save_private(&key_path, &key)?;
    }
    let key = Zeroizing::new(fs::read(&key_path)?);
    if key.len() != 64 {
        return Err(anyhow!("disposable recovery key is invalid"));
    }
    execute(&transport, &["bash", script, "prepare"], Some(&key))?;
    let header = Zeroizing::new(execute(
        &transport,
        &["cat", "/run/skillet-encryption-test/luks.header"],
        None,
    )?);
    save_private(&private.join("root-luks.header"), &header)?;
    execute(&transport, &["bash", script, "verify"], Some(&key))?;
    let metadata = execute(
        &transport,
        &[
            "cryptsetup",
            "luksDump",
            "--dump-json-metadata",
            "/dev/disk/by-partlabel/root",
        ],
        None,
    )?;
    let metadata: serde_json::Value = serde_json::from_slice(&metadata)?;
    if metadata["keyslots"].as_object().map(serde_json::Map::len) != Some(2) {
        return Err(anyhow!(
            "expected only TPM and independent recovery keyslots"
        ));
    }
    ownership()?;
    run.phase = Phase::Started;
    store.save(&run)?;
    backend.stop(&run)?;
    backend
        .inspect(&run)?
        .ok_or_else(|| anyhow!("encryption VM disappeared"))?
        .validate_owned(&run)?;
    backend.start(&run)?;
    vm::ready_locked(&butane, &store, &identity)?;
    let fresh = store.load(&identity)?;
    let ownership = || {
        backend
            .inspect(&fresh)?
            .ok_or_else(|| skillet_vm::Error::Invalid("encryption VM disappeared".into()))?
            .validate_owned(&fresh)
    };
    let transport = OwnershipCheckedTransport::new(&ssh, &ownership);
    execute(&transport, &["bash", script, "verify"], Some(&key))?;
    println!("Encrypted root acceptance passed: Secure Boot, LUKS2, data persistence, independent recovery key and unattended cold boot.");
    println!(
        "Disposable recovery artifacts: {} (removed with this VM)",
        private.display()
    );
    Ok(())
}

fn execute(
    transport: &impl GuestTransport,
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let arguments: Vec<_> = std::iter::once("-n").chain(args.iter().copied()).collect();
    let out = transport.execute(
        &GuestCommand {
            program: "sudo",
            arguments: &arguments,
        },
        input,
    )?;
    if !out.status.success() {
        return Err(anyhow!(
            "encrypted-root guest assertion failed; no credential output is printed"
        ));
    }
    Ok(out.stdout)
}

fn save_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("recovery artifact has no parent"))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(path)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}
