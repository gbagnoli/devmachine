//! Build and run the provider-free Arr consumer contract in an owned smoke VM.
use skillet_vm::{
    backend::{VirshBackend, VmBackend},
    transport::{
        GuestCommand, GuestTransport, HostKeyPolicy, OwnershipCheckedTransport, SshTransport,
    },
    ManifestStore, RunIdentity,
};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn artifact() -> Result<PathBuf> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or("workspace path")?;
    let output = Command::new("cargo")
        .current_dir(workspace)
        .args([
            "test",
            "--release",
            "--target",
            "x86_64-unknown-linux-musl",
            "-p",
            "skillet_media",
            "--test",
            "consumer",
            "--no-run",
            "--message-format=json",
        ])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "consumer test build failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let artifacts: Vec<_> = String::from_utf8(output.stdout)?
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|value| {
            (value["reason"] == "compiler-artifact"
                && value["target"]["name"] == "consumer"
                && value["profile"]["test"] == true)
                .then(|| value["executable"].as_str().map(PathBuf::from))
                .flatten()
        })
        .collect();
    match artifacts.as_slice() {
        [artifact] => Ok(artifact.clone()),
        _ => Err("compiled consumer artifact absent or ambiguous".into()),
    }
}

fn execute(
    guest: &impl GuestTransport,
    program: &str,
    args: &[&str],
) -> Result<std::process::Output> {
    let output = guest.execute(
        &GuestCommand {
            program,
            arguments: args,
        },
        None,
    )?;
    if !output.status.success() {
        return Err(format!("media consumer guest operation failed: {program}").into());
    }
    Ok(output)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: media_consumer BUTANE HOST INSTANCE".into());
    }
    let butane = fs::canonicalize(&args[1])?;
    let id = RunIdentity::new(&args[2], &args[3])?;
    let config = skillet_hosts::profile_for_name(id.host())
        .and_then(|p| p.media_config())
        .ok_or("host does not declare media")?;
    let store = ManifestStore::new(&butane.join("runs"), skillet_vm::current_uid())?;
    let _lock = store.lock(&id)?;
    let run = store.load(&id)?;
    if run.environment != skillet_vm::Environment::Test {
        return Err("requires a disposable test manifest".into());
    }
    let backend = VirshBackend::for_run(&run, &butane.join("bin/virsh"))?;
    let ownership = || {
        backend
            .inspect(&run)?
            .ok_or_else(|| skillet_vm::Error::Invalid("owned test VM absent".into()))?
            .validate_owned(&run)
    };
    ownership()?;
    let artifact = artifact()?;
    let mut transport = SshTransport::new(run.ssh.clone(), HostKeyPolicy::Verify)?;
    transport.timeout = Duration::from_mins(25);
    let guest = OwnershipCheckedTransport::new(&transport, &ownership);
    let directory = String::from_utf8(
        execute(
            &guest,
            "mktemp",
            &["-d", "/var/tmp/skillet-media-contract.XXXXXXXX"],
        )?
        .stdout,
    )?
    .trim()
    .to_string();
    if !directory.starts_with("/var/tmp/skillet-media-contract.")
        || directory.contains(char::is_whitespace)
    {
        return Err("invalid guest temporary directory".into());
    }
    let executable = format!("{directory}/consumer");
    guest.upload(&artifact, &executable)?;
    execute(&guest, "chmod", &["0700", &executable])?;
    let environment = format!("SKILLET_DISPOSABLE_HOSTNAME={}", run.guest_hostname);
    let owner = format!("SKILLET_DISPOSABLE_DATA_OWNER={}", config.data_owner);
    let group = format!("SKILLET_DISPOSABLE_DATA_GROUP={}", config.data_group);
    let result = guest.execute(
        &GuestCommand {
            program: "sudo",
            arguments: &[
                "-n",
                "env",
                &environment,
                &owner,
                &group,
                "timeout",
                "1200",
                &executable,
                "--ignored",
                "--nocapture",
            ],
        },
        None,
    )?;
    let mut log = result.stdout;
    log.extend(result.stderr);
    fs::write(store.run_dir(&id).join("media-consumer.log"), &log)?;
    println!("{}", String::from_utf8_lossy(&log));
    execute(&guest, "rm", &["-rf", "--", &directory])?;
    ownership()?;
    if !result.status.success() {
        return Err("Arr consumer contract failed; see the retained media-consumer.log".into());
    }
    Ok(())
}
