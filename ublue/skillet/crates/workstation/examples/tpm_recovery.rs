//! Explicit native-only recovery acceptance. Never clone a provisioned service VM.
use sha2::{Digest as _, Sha256};
use skillet_vm::{
    backend::{VirshBackend, VmBackend},
    transport::{
        GuestCommand, GuestTransport, HostKeyPolicy, OwnershipCheckedTransport, SshTransport,
    },
    Backend, ManifestStore, Phase, RunIdentity, VmRun,
};
use skillet_workstation::{
    root_recovery::{Record, RecoveryStore, VaultStore},
    vault::Vault,
};
use std::{
    error::Error,
    fs,
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    process::Command,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
#[derive(serde::Serialize, serde::Deserialize)]
struct Ancestry {
    source: RunIdentity,
    source_uuid: uuid::Uuid,
    clone_uuid: uuid::Uuid,
    volume: String,
    original_slot: String,
    source_header_hash: String,
}
fn path(id: &RunIdentity) -> String {
    format!(
        "skillet/environments/test/hosts/{}/instances/{}/storage/root-recovery-key",
        id.host(),
        id.instance()
    )
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 7 {
        return Err(
            "usage: tpm_recovery BUTANE HOST SOURCE CLONE PORT clone|unlock|rebind|verify".into(),
        );
    }
    let butane = fs::canonicalize(&args[1])?;
    let source_id = RunIdentity::new(&args[2], &args[3])?;
    let clone_id = RunIdentity::new(&args[2], &args[4])?;
    if source_id == clone_id {
        return Err("source and recovery instance must differ".into());
    }
    let port: u16 = args[5].parse()?;
    let store = ManifestStore::new(&butane.join("runs"), skillet_vm::current_uid())?;
    let _source_lock = store.lock(&source_id)?;
    let source = store.load(&source_id)?;
    let _clone_lock = if args[6] == "clone" {
        None
    } else {
        Some(store.lock(&clone_id)?)
    };
    if source.connection.backend != Backend::Native
        || source.root_profile != skillet_vm::install::RootProfile::Tpm
    {
        return Err("requires a native TPM source VM".into());
    }
    let backend = VirshBackend::for_run(&source, &butane.join("bin/virsh"))?;
    owned(&backend, &source)?;
    let mut vault = Vault::open(&Vault::default_path()?, None)?;
    let mut recovery = VaultStore {
        vault: &mut vault,
        key_file: None,
    };
    let record = recovery
        .load(&path(&source_id))?
        .ok_or("source recovery is absent")?;
    if !record.verified || record.header.is_none() {
        return Err("source recovery and header must be verified first".into());
    }
    if args[6] == "clone" {
        if recovery.load(&path(&clone_id))?.is_some() {
            return Err("clone archive already exists; use another instance".into());
        }
        return clone_vm(&butane, &store, &backend, &source, &clone_id, port, &record);
    }
    let clone = store.load(&clone_id)?;
    let ancestry: Ancestry = serde_json::from_slice(&fs::read(
        store.run_dir(&clone_id).join("recovery-source.json"),
    )?)?;
    if ancestry.source != source_id
        || ancestry.source_uuid != source.uuid
        || ancestry.clone_uuid != clone.uuid
        || ancestry.volume != record.volume
        || ancestry.source_header_hash
            != hex::encode(Sha256::digest(
                record.header.as_ref().ok_or("source header absent")?,
            ))
    {
        return Err("recovery ancestry mismatch".into());
    }
    if clone.ssh.port != port {
        return Err("recorded recovery port differs".into());
    }
    owned(&backend, &clone)?;
    match args[6].as_str() {
        "unlock" => unlock(&backend, &clone, &record)?,
        "rebind" => rebind(
            &butane,
            &store,
            &backend,
            clone,
            &record,
            &ancestry,
            &mut recovery,
        )?,
        "verify" => {
            verify(&backend, &clone, &record, &recovery)?;
            verify_source(&backend, &source, &record, &ancestry.original_slot)?;
        }
        _ => return Err("unknown recovery action".into()),
    }
    Ok(())
}
fn verify(
    backend: &VirshBackend,
    clone: &VmRun,
    record: &Record,
    recovery: &VaultStore<'_>,
) -> Result<()> {
    let ssh = SshTransport::new(clone.ssh.clone(), HostKeyPolicy::Verify)?;
    let guard = || owned(backend, clone);
    let guest = OwnershipCheckedTransport::new(&ssh, &guard);
    wait_ssh(&guest)?;
    assert_volume(&guest, record)?;
    require_base_only(&guest)?;
    let metadata = execute(
        &guest,
        "sudo",
        &[
            "-n",
            "cryptsetup",
            "luksDump",
            "--dump-json-metadata",
            "/dev/disk/by-partlabel/root",
        ],
        None,
    )?;
    let metadata: serde_json::Value = serde_json::from_slice(&metadata)?;
    if metadata["keyslots"].as_object().map(serde_json::Map::len) != Some(2) {
        return Err("expected exactly new TPM plus recovery slots".into());
    }
    let marker = execute(
        &guest,
        "sudo",
        &["-n", "cat", "/var/lib/data/.skillet-tpm-loss-sentinel"],
        None,
    )?;
    if marker != b"TPM replacement preserves encrypted data\n" {
        return Err("data sentinel changed".into());
    }
    if recovery.load(&path(&clone.identity))?.is_none_or(|r| {
        !r.verified || r.header.is_none() || r.volume != record.volume || *r.key != *record.key
    }) {
        return Err("clone archive incomplete".into());
    }
    println!("PASS: unattended cold boot with fresh TPM, same volume/data, exactly two slots and verified recovery header");
    Ok(())
}

fn verify_source(
    backend: &VirshBackend,
    source: &VmRun,
    record: &Record,
    slot: &str,
) -> Result<()> {
    let ssh = SshTransport::new(source.ssh.clone(), HostKeyPolicy::Verify)?;
    let guard = || owned(backend, source);
    let guest = OwnershipCheckedTransport::new(&ssh, &guard);
    wait_ssh(&guest)?;
    assert_volume(&guest, record)?;
    require_base_only(&guest)?;
    let pass = Zeroizing::new(execute(
        &guest,
        "sudo",
        &[
            "-n",
            "clevis",
            "luks",
            "pass",
            "-d",
            "/dev/disk/by-partlabel/root",
            "-s",
            slot,
        ],
        None,
    )?);
    execute(
        &guest,
        "sudo",
        &[
            "-n",
            "cryptsetup",
            "open",
            "--test-passphrase",
            "--key-file=-",
            "/dev/disk/by-partlabel/root",
        ],
        Some(&pass),
    )?;
    println!("PASS: original source TPM still unlocks its unchanged volume and original header archive is preserved");
    Ok(())
}

fn rebind(
    butane: &Path,
    store: &ManifestStore,
    backend: &VirshBackend,
    mut clone: VmRun,
    record: &Record,
    ancestry: &Ancestry,
    recovery: &mut VaultStore<'_>,
) -> Result<()> {
    let ssh = SshTransport::new(clone.ssh.clone(), HostKeyPolicy::Verify)?;
    let guard = || owned(backend, &clone);
    let guest = OwnershipCheckedTransport::new(&ssh, &guard);
    assert_volume(&guest, record)?;
    guest.upload(
        &butane.join("../skillet/scripts/root-tpm-rebind.sh"),
        "/var/tmp/skillet-root-tpm-rebind.sh",
    )?;
    execute(
        &guest,
        "sudo",
        &[
            "-n",
            "bash",
            "/var/tmp/skillet-root-tpm-rebind.sh",
            &ancestry.original_slot,
        ],
        Some(record.key.as_bytes()),
    )?;
    execute(
        &guest,
        "sudo",
        &["-n", "hostnamectl", "set-hostname", &clone.guest_hostname],
        None,
    )?;
    // Separate archive: do not replace the source's original TPM header.
    if recovery.load(&path(&clone.identity))?.is_none() {
        recovery.save(
            &path(&clone.identity),
            &Record {
                key: Zeroizing::new(record.key.to_string()),
                volume: record.volume.clone(),
                verified: true,
                header: None,
            },
        )?;
    }
    skillet_workstation::root_recovery::provision(
        clone.identity.host(),
        &path(&clone.identity),
        &butane.join("../skillet/scripts/root-recovery.sh"),
        recovery,
        &guest,
        || {
            Err(skillet_workstation::root_recovery::RecoveryError::Invalid(
                "clone must reuse its recorded recovery key",
            ))
        },
    )?;

    clone.phase = Phase::Started;
    store.save(&clone)?;
    backend.stop(&clone)?;
    owned(backend, &clone)?;
    backend.start(&clone)?;
    owned(backend, &clone)?;
    println!("PASS: new TPM verified, original binding removed, refreshed clone header archived; cold boot started");
    Ok(())
}

fn owned(backend: &VirshBackend, run: &VmRun) -> skillet_vm::Result<()> {
    backend
        .inspect(run)?
        .ok_or_else(|| skillet_vm::Error::Invalid("owned recovery VM absent".into()))?
        .validate_owned(run)
}
fn execute(
    guest: &impl GuestTransport,
    program: &str,
    arguments: &[&str],
    input: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let out = guest.execute(&GuestCommand { program, arguments }, input)?;
    if !out.status.success() {
        return Err("recovery guest assertion failed; sensitive output suppressed".into());
    }
    Ok(out.stdout)
}
fn assert_volume(guest: &impl GuestTransport, record: &Record) -> Result<()> {
    let volume = execute(
        guest,
        "sudo",
        &[
            "-n",
            "cryptsetup",
            "luksUUID",
            "/dev/disk/by-partlabel/root",
        ],
        None,
    )?;
    if std::str::from_utf8(&volume)?.trim() != record.volume {
        return Err("recovered volume differs".into());
    }
    execute(
        guest,
        "sudo",
        &[
            "-n",
            "cryptsetup",
            "open",
            "--test-passphrase",
            "--key-file=-",
            "/dev/disk/by-partlabel/root",
        ],
        Some(record.key.as_bytes()),
    )?;
    Ok(())
}
fn wait_ssh(guest: &impl GuestTransport) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(180);
    while Instant::now() < deadline {
        if guest
            .execute(
                &GuestCommand {
                    program: "true",
                    arguments: &[],
                },
                None,
            )
            .is_ok_and(|out| out.status.success())
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    Err("recovery SSH deadline exceeded".into())
}
fn private_copy(source: &Path, destination: &Path) -> Result<()> {
    if fs::symlink_metadata(source)?.file_type().is_symlink() {
        return Err("refusing linked source artifact".into());
    }
    if source.is_dir() {
        fs::create_dir_all(destination)?;
        fs::set_permissions(destination, fs::Permissions::from_mode(0o700))?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            private_copy(&entry.path(), &destination.join(entry.file_name()))?;
        }
    } else if source.is_file() {
        fs::copy(source, destination)?;
        fs::set_permissions(destination, fs::Permissions::from_mode(0o600))?;
    } else {
        return Err("refusing non-regular clone artifact".into());
    }
    Ok(())
}
fn refuse_provider_journals(source_dir: &Path) -> Result<()> {
    // Provider delivery would make a disk clone impersonate live services.
    for entry in fs::read_dir(source_dir)? {
        let name = entry?.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".json") && !matches!(name.as_ref(), "vm.json" | "final-status.json") {
            return Err(
                "source has additional journals; use a fresh base-only encryption VM".into(),
            );
        }
    }
    Ok(())
}

fn clone_vm(
    butane: &Path,
    store: &ManifestStore,
    backend: &VirshBackend,
    source: &VmRun,
    id: &RunIdentity,
    port: u16,
    record: &Record,
) -> Result<()> {
    if source.phase != Phase::Ready || store.run_dir(id).join("vm.json").exists() {
        return Err("requires ready source and unused clone instance".into());
    }
    skillet_vm::provisioning::validate_ssh_port(port)?;
    let source_dir = store.run_dir(&source.identity);
    refuse_provider_journals(&source_dir)?;
    if backend.contains_name(&id.domain_name())? {
        return Err("clone domain already exists".into());
    }
    let intent = store.prepare_intent_with_profile(
        id,
        source.connection.clone(),
        port,
        &source.source_commit,
        source.root_profile,
    )?;
    let _lock = store.lock(id)?;
    let slot = prepare_source(butane, backend, source, record)?;
    let dir = store.run_dir(id);
    let mut clone = source.clone();
    clone.identity = id.clone();
    clone.uuid = intent.uuid;
    clone.guest_hostname = id.domain_name();
    clone.disk = dir.join(format!("{}.qcow2", id.domain_name()));
    clone.ignition = dir.join("ignition").join(format!("{}.ign", id.host()));
    clone.ssh.port = port;
    clone.ssh.identity = dir.join("ssh/id_ed25519");
    clone.ssh.known_hosts = dir.join("ssh/known_hosts");
    clone.phase = Phase::Preparing;
    store.save(&clone)?;
    fs::write(
        dir.join("recovery-source.json"),
        serde_json::to_vec_pretty(&Ancestry {
            source: source.identity.clone(),
            source_uuid: source.uuid,
            clone_uuid: clone.uuid,
            volume: record.volume.clone(),
            original_slot: slot,
            source_header_hash: hex::encode(Sha256::digest(
                record.header.as_ref().ok_or("source header absent")?,
            )),
        })?,
    )?;
    fs::set_permissions(
        dir.join("recovery-source.json"),
        fs::Permissions::from_mode(0o600),
    )?;
    let mut stopped = source.clone();
    stopped.phase = Phase::Started;
    store.save(&stopped)?;
    owned(backend, source)?;
    backend.stop(source)?;
    if backend.inspect(source)?.ok_or("source absent")?.state != "shut off" {
        return Err("source is not stopped".into());
    }
    owned(backend, source)?;
    let copied = (|| -> Result<()> {
        let mut command = Command::new("qemu-img");
        command
            .args(["convert", "-f", "qcow2", "-O", "qcow2"])
            .arg(&source.disk)
            .arg(&clone.disk);
        let output = skillet_vm::capture_command(command, Duration::from_secs(600))?;
        if !output.status.success() {
            return Err("disk clone failed".into());
        }
        owned(backend, source)?;
        fs::set_permissions(&clone.disk, fs::Permissions::from_mode(0o600))?;
        for name in ["ignition", "ssh", "source"] {
            private_copy(&source_dir.join(name), &dir.join(name))?;
        }
        let known = fs::read_to_string(&clone.ssh.known_hosts)?.replace(
            &format!("[127.0.0.1]:{}", source.ssh.port),
            &format!("[127.0.0.1]:{port}"),
        );
        fs::write(&clone.ssh.known_hosts, known)?;
        Ok(())
    })();
    owned(backend, source)?;
    backend.start(source)?;
    owned(backend, source)?;
    copied?;
    let xml = skillet_vm::domain_xml::write_recovery_domain_xml(
        store,
        &clone,
        &backend.x86_64_emulator(&clone)?,
    )?;
    if backend.inspect(&clone)?.is_some() {
        return Err("clone domain appeared during preparation".into());
    }
    VmBackend::define(backend, &clone, &xml)?;
    clone.phase = Phase::Defined;
    store.save(&clone)?;
    owned(backend, &clone)?;
    println!("PASS: archived header restored on tmpfs fixture; stopped disk cloned; independent TPM/NVRAM and unlogged console defined");
    Ok(())
}
fn require_base_only(guest: &impl GuestTransport) -> Result<()> {
    for (program, arguments) in [
        ("sudo", vec!["-n", "podman", "ps", "-aq"]),
        (
            "sudo",
            vec!["-n", "podman", "secret", "ls", "--format", "{{.Name}}"],
        ),
        (
            "sudo",
            vec![
                "-n",
                "find",
                "/etc",
                "-maxdepth",
                "3",
                "-name",
                "skillet*.cred",
            ],
        ),
    ] {
        if !execute(guest, program, &arguments, None)?.is_empty() {
            return Err(
                "source contains containers or delivered credentials; use a fresh base-only VM"
                    .into(),
            );
        }
    }
    Ok(())
}

fn prepare_source(
    butane: &Path,
    backend: &VirshBackend,
    source: &VmRun,
    record: &Record,
) -> Result<String> {
    let ssh = SshTransport::new(source.ssh.clone(), HostKeyPolicy::Verify)?;
    let guard = || owned(backend, source);
    let guest = OwnershipCheckedTransport::new(&ssh, &guard);
    assert_volume(&guest, record)?;
    require_base_only(&guest)?;
    let metadata = execute(
        &guest,
        "sudo",
        &[
            "-n",
            "cryptsetup",
            "luksDump",
            "--dump-json-metadata",
            "/dev/disk/by-partlabel/root",
        ],
        None,
    )?;
    let metadata: serde_json::Value = serde_json::from_slice(&metadata)?;
    if metadata["keyslots"].as_object().map(serde_json::Map::len) != Some(2) {
        return Err("source must have only TPM and independent recovery slots".into());
    }
    let bindings = execute(
        &guest,
        "sudo",
        &[
            "-n",
            "clevis",
            "luks",
            "list",
            "-d",
            "/dev/disk/by-partlabel/root",
        ],
        None,
    )?;
    let bindings = std::str::from_utf8(&bindings)?;
    if bindings.lines().count() != 1 {
        return Err("expected one source TPM binding".into());
    }
    let slot = bindings.split_once(':').ok_or("source slot absent")?.0;
    if !slot.bytes().all(|b| b.is_ascii_digit()) {
        return Err("invalid source slot".into());
    }
    guest.upload(
        &butane.join("../skillet/integration_tests/root-header-restore.sh"),
        "/var/tmp/skillet-header-restore.sh",
    )?;
    let mut payload = Zeroizing::new(record.key.as_bytes().to_vec());
    payload.extend_from_slice(record.header.as_ref().ok_or("header absent")?);
    execute(
        &guest,
        "sudo",
        &["-n", "bash", "/var/tmp/skillet-header-restore.sh"],
        Some(&payload),
    )?;
    execute(
        &guest,
        "sudo",
        &["-n", "tee", "/var/lib/data/.skillet-tpm-loss-sentinel"],
        Some(b"TPM replacement preserves encrypted data\n"),
    )?;
    execute(
        &guest,
        "sudo",
        &[
            "-n",
            "sync",
            "-f",
            "/var/lib/data/.skillet-tpm-loss-sentinel",
        ],
        None,
    )?;
    Ok(slot.to_owned())
}

fn unlock(backend: &VirshBackend, run: &VmRun, record: &Record) -> Result<()> {
    if backend.inspect(run)?.ok_or("clone absent")?.state != "shut off" {
        return Err("unlock requires a stopped recovery clone".into());
    }
    owned(backend, run)?;
    backend.start(run)?;
    owned(backend, run)?;
    let tty = backend.recovery_console(run)?;
    owned(backend, run)?;
    let mut terminal = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(nix::libc::O_NONBLOCK | nix::libc::O_NOCTTY)
        .open(tty)?;
    let mut settings = nix::sys::termios::tcgetattr(&terminal)?;
    nix::sys::termios::cfmakeraw(&mut settings);
    nix::sys::termios::tcsetattr(&terminal, nix::sys::termios::SetArg::TCSANOW, &settings)?;
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut output = Zeroizing::new(Vec::new());
    let mut bytes = Zeroizing::new([0u8; 4096]);
    let mut prompted = false;
    while Instant::now() < deadline {
        owned(backend, run)?;
        match terminal.read(&mut *bytes) {
            Ok(count) => {
                output.extend_from_slice(&bytes[..count]);
                if output.len() > 65536 {
                    output.drain(..32768);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => (),
            Err(error) => return Err(error.into()),
        }
        let text = String::from_utf8_lossy(&output);
        if text.contains("Please enter passphrase") || text.contains("Enter passphrase for") {
            prompted = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if !prompted {
        return Err("fresh TPM did not reach the recovery prompt within deadline".into());
    }
    let ssh = SshTransport::new(run.ssh.clone(), HostKeyPolicy::Verify)?;
    let guard = || owned(backend, run);
    let guest = OwnershipCheckedTransport::new(&ssh, &guard);
    if guest
        .execute(
            &GuestCommand {
                program: "true",
                arguments: &[],
            },
            None,
        )
        .is_ok_and(|out| out.status.success())
    {
        return Err("fresh TPM unexpectedly unlocked".into());
    }
    owned(backend, run)?;
    terminal.write_all(record.key.as_bytes())?;
    terminal.write_all(b"\r")?;
    // Stop collecting console output before credential transfer; never print it.
    drop(terminal);
    drop(output);
    drop(bytes);
    owned(backend, run)?;
    wait_ssh(&guest)?;
    assert_volume(&guest, record)?;
    println!("PASS: fresh TPM blocked unattended boot; vault recovery passphrase booted the same encrypted volume");
    Ok(())
}

#[cfg(test)]
#[path = "tpm_recovery/tests.rs"]
mod tests;
