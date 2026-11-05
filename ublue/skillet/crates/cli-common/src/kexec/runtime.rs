//! Guest-side activation with systemd-owned interruption cleanup.
use super::{select_entry, BootEntry};
use crate::CliCommonError;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::{fs, io, path::Path, process::Command};

#[derive(Debug, thiserror::Error)]
enum RuntimeError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Environment(#[from] std::env::VarError),
    #[error(transparent)]
    Entry(#[from] super::Error),
}

#[derive(clap::Subcommand, Debug, Clone, Copy)]
pub enum Commands {
    /// Install the opt-in activation service (does not start it)
    Install,
    /// Supervised activation worker; invoke through the service
    Run,
    /// Clean up a kernel owned by this service invocation
    Cleanup,
}

const UNIT: &str = "skillet-kexec-activate.service";
const OWNER: &str = "/run/skillet-kexec-owner";

pub fn dispatch(command: Commands) -> Result<(), CliCommonError> {
    dispatch_inner(command).map_err(|e| CliCommonError::Config(format!("kexec activation: {e}")))
}

fn checked(program: &str, args: &[&str]) -> Result<(), RuntimeError> {
    if !Command::new(program).args(args).status()?.success() {
        return Err(io::Error::other(format!("{program} failed")).into());
    }
    Ok(())
}

fn armed() -> io::Result<bool> {
    Ok(fs::read_to_string("/sys/kernel/kexec_loaded")?.trim() == "1")
}

fn dispatch_inner(command: Commands) -> Result<(), RuntimeError> {
    if !nix::unistd::Uid::effective().is_root() {
        return Err(io::Error::other("root is required").into());
    }
    if matches!(command, Commands::Install) {
        let binary = fs::canonicalize(std::env::current_exe()?)?;
        let binary = binary
            .to_str()
            .ok_or_else(|| io::Error::other("invalid executable path"))?;
        if !binary
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b))
        {
            return Err(io::Error::other("unsupported executable path").into());
        }
        let unit = format!("[Unit]\nDescription=Controlled Skillet kexec activation\nDefaultDependencies=no\nAfter=local-fs.target\n[Service]\nType=exec\nExecStart={binary} kexec run\nExecStopPost={binary} kexec cleanup\nRuntimeMaxSec=120\nTimeoutStopSec=20\nRestart=no\n");
        fs::write(format!("/etc/systemd/system/{UNIT}"), unit)?;
        checked("systemctl", &["daemon-reload"])?;
        return Ok(());
    }
    let invocation = std::env::var("INVOCATION_ID")?;
    if invocation.len() != 32 || !invocation.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(io::Error::other("missing valid service invocation identity").into());
    }
    let identity = format!(
        "{}:{}",
        fs::read_to_string("/proc/sys/kernel/random/boot_id")?.trim(),
        invocation
    );
    if matches!(command, Commands::Cleanup) {
        match fs::read_to_string(OWNER) {
            Ok(owner) if owner == identity => {
                if armed()? {
                    checked("kexec", &["-s", "-u"])?;
                }
                if armed()? {
                    return Err(io::Error::other("kernel remains armed").into());
                }
                fs::remove_file(OWNER)?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            _ => {
                return Err(io::Error::other(
                    "kernel ownership differs; manual intervention required",
                )
                .into())
            }
        }
        return Ok(());
    }
    if armed()? || Path::new(OWNER).exists() {
        return Err(io::Error::other("preexisting armed kernel or unresolved attempt").into());
    }
    let entry = select_booted()?;
    let mut owner = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(OWNER)?;
    owner.write_all(identity.as_bytes())?;
    owner.sync_all()?;
    let loaded = Command::new("kexec")
        .args(["-s", "-l"])
        .arg(&entry.kernel)
        .arg(format!("--initrd={}", entry.initrd.display()))
        .arg(format!("--command-line={}", entry.options))
        .status()?;
    if !loaded.success() || !armed()? {
        return Err(io::Error::other("signed kernel load failed").into());
    }
    checked("systemctl", &["--check-inhibitors=yes", "kexec"])?;
    // Remain alive until shutdown. RuntimeMaxSec makes a cancelled handoff
    // fail and invokes independent ExecStopPost cleanup, including on SIGKILL.
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

fn select_booted() -> Result<BootEntry, RuntimeError> {
    let output = Command::new("rpm-ostree")
        .args(["status", "--json"])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("rpm-ostree status failed").into());
    }
    let status: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let deployments = status["deployments"]
        .as_array()
        .ok_or_else(|| io::Error::other("missing deployments"))?;
    if deployments.iter().any(|v| {
        v["staged"].as_bool() == Some(true)
            || v["booted"].as_bool() != Some(true) && deployments.first() == Some(v)
    }) {
        return Err(io::Error::other(
            "pending deployment requires coordinated finalization; refusing activation",
        )
        .into());
    }
    let booted: Vec<_> = deployments
        .iter()
        .filter(|v| v["booted"].as_bool() == Some(true))
        .collect();
    if booted.len() != 1 {
        return Err(io::Error::other("ambiguous booted deployment").into());
    }
    let item = booted[0];
    let checksum = item["checksum"]
        .as_str()
        .ok_or_else(|| io::Error::other("missing checksum"))?;
    let osname = item["osname"]
        .as_str()
        .ok_or_else(|| io::Error::other("missing osname"))?;
    let serial = item["serial"]
        .as_u64()
        .ok_or_else(|| io::Error::other("missing serial"))?;
    if checksum.len() != 64
        || !checksum.bytes().all(|b| b.is_ascii_hexdigit())
        || !osname
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(io::Error::other("invalid deployment identity").into());
    }
    let expected = fs::canonicalize(format!(
        "/sysroot/ostree/deploy/{osname}/deploy/{checksum}.{serial}"
    ))?;
    let boot = fs::canonicalize("/boot")?;
    let mut contents = Vec::new();
    for file in fs::read_dir("/boot/loader/entries")? {
        let file = file?.path();
        if file.extension().is_none_or(|ext| ext != "conf") {
            continue;
        }
        contents.push(fs::read_to_string(file)?);
    }
    let entry = select_entry(
        contents.iter().map(String::as_str),
        Path::new("/boot"),
        &expected,
        |path| fs::canonicalize(path).ok(),
    )?;
    for artifact in [&entry.kernel, &entry.initrd] {
        let path = fs::canonicalize(artifact)?;
        if !path.starts_with(&boot) || !path.is_file() {
            return Err(io::Error::other("invalid boot artifact").into());
        }
    }
    Ok(entry)
}
