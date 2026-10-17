//! Run a guest assertion with manifest trust and ownership checks, holding its run lock.
//! stdin is passed in memory; the assertion must redact its own diagnostic output.
use sha2::Digest as _;
use skillet_vm::{
    backend::{VirshBackend, VmBackend},
    transport::{
        GuestCommand, GuestTransport, HostKeyPolicy, OwnershipCheckedTransport, SshTransport,
    },
    ManifestStore, RunIdentity,
};
use std::{
    io::{Read, Write},
    path::PathBuf,
    time::Duration,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        return Err("usage: guest_command BUTANE HOST INSTANCE PROGRAM [ARGS...]".into());
    }
    let butane = PathBuf::from(&args[0]);
    let identity = RunIdentity::new(&args[1], &args[2])?;
    let store = ManifestStore::new(&butane.join("runs"), skillet_vm::current_uid())?;
    let _lock = store.lock(&identity)?;
    let run = store.load(&identity)?;
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    let guard = || {
        backend
            .inspect(&run)?
            .ok_or_else(|| skillet_vm::Error::Invalid("owned domain absent".into()))?
            .validate_owned(&run)
    };
    guard()?;
    if args[3] == "--grow-disk" {
        if args.len() != 5 {
            return Err("disk growth requires SIZE_GIB".into());
        }
        backend.grow_disk(&run, args[4].parse()?)?;
        println!("Owned disk capacity verified and grown");
        return Ok(());
    }
    let mut ssh = SshTransport::new(run.ssh.clone(), HostKeyPolicy::Verify)?;
    ssh.timeout = Duration::from_mins(10);
    let checked = OwnershipCheckedTransport::new(&ssh, &guard);
    if args[3] == "--upload" {
        if args.len() != 6 {
            return Err("upload requires LOCAL DESTINATION".into());
        }
        checked.upload(PathBuf::from(&args[4]).as_path(), &args[5])?;
        let output = checked.execute(
            &GuestCommand {
                program: "sha256sum",
                arguments: &[&args[5]],
            },
            None,
        )?;
        let digest = hex::encode(sha2::Sha256::digest(std::fs::read(&args[4])?));
        if !output.status.success()
            || !String::from_utf8_lossy(&output.stdout).starts_with(&format!("{digest} "))
        {
            return Err("uploaded artifact verification failed".into());
        }
        println!("Uploaded and verified test artifact");
        return Ok(());
    }
    let mut input = zeroize::Zeroizing::new(Vec::new());
    std::io::stdin()
        .take(16 * 1024 * 1024)
        .read_to_end(&mut input)?;
    let arguments: Vec<_> = args[4..].iter().map(String::as_str).collect();
    let output = checked.execute(
        &GuestCommand {
            program: &args[3],
            arguments: &arguments,
        },
        Some(&input),
    )?;
    std::io::stdout().write_all(&output.stdout)?;
    std::io::stderr().write_all(&output.stderr)?;
    if !output.status.success() {
        return Err("guest assertion failed".into());
    }
    Ok(())
}
