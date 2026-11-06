//! Guest-side activation with systemd-owned interruption cleanup.
use super::{select_entry, BootEntry};
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::{fs, io, path::Path, process::Command};

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
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
    /// Supervised activation worker; invoke through the service
    Run,
    /// Clean up a kernel owned by this service invocation
    Cleanup,
}

const OWNER: &str = "/run/skillet-kexec-owner";

pub fn dispatch(command: Commands) -> Result<(), RuntimeError> {
    dispatch_inner(command)
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
                if Path::new("/run/skillet-maintenance/paused").exists() {
                    checked("systemctl", &["unmask", "--runtime", "rpm-ostreed.service"])?;
                    fs::remove_file("/run/skillet-maintenance/paused")?;
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
    let _configuration = skillet_core::maintenance::activation()?;
    let mut owner = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(OWNER)?;
    owner.write_all(identity.as_bytes())?;
    owner.sync_all()?;
    checked("mount", &["--bind", "/sysroot", "/sysroot"])?;
    checked("mount", &["-o", "remount,bind,rw", "/sysroot"])?;
    let entry = prepare_target()?;
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

fn prepare_target() -> Result<BootEntry, RuntimeError> {
    let output = Command::new("rpm-ostree")
        .args(["status", "--json"])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other("rpm-ostree status failed").into());
    }
    let status: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let item = super::activation_target(&status)?.clone();
    // Pause the existing staging owner; do not introduce a second updater.
    // The native lock excludes in-flight OSTree mutations while pausing it.
    let native = sysroot_lock()?;
    let fresh = Command::new("rpm-ostree")
        .args(["status", "--json"])
        .output()?;
    if !fresh.status.success() {
        return Err(io::Error::other("fresh status failed").into());
    }
    let fresh: serde_json::Value = serde_json::from_slice(&fresh.stdout)?;
    if super::activation_target(&fresh)? != &item {
        return Err(io::Error::other("deployment changed before staging exclusion").into());
    }
    let enabled = Command::new("systemctl")
        .args(["is-enabled", "rpm-ostreed.service"])
        .output()?;
    if String::from_utf8_lossy(&enabled.stdout).contains("masked") {
        return Err(io::Error::other("staging daemon was already masked").into());
    }
    fs::write("/run/skillet-maintenance/paused", b"owned")?;
    checked("systemctl", &["mask", "--runtime", "rpm-ostreed.service"])?;
    checked("systemctl", &["stop", "rpm-ostreed.service"])?;
    drop(native);
    if item["staged"].as_bool() == Some(true) {
        // Retain this gate even on failure: after finalization, further live
        // /etc writes would not reach the finalized deployment. Reboot clears it.
        fs::write("/run/skillet-maintenance/finalized", b"reboot required")?;
        checked("ostree", &["admin", "finalize-staged"])?;
    }
    let _native = sysroot_lock()?;
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

fn sysroot_lock() -> io::Result<fs::File> {
    use nix::fcntl::{fcntl, FcntlArg};
    use std::os::fd::AsRawFd as _;
    use std::os::unix::fs::MetadataExt as _;
    loop {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open("/sysroot/ostree/lock")?;
        let lock = nix::libc::flock {
            l_type: nix::libc::F_WRLCK.try_into().map_err(io::Error::other)?,
            l_whence: nix::libc::SEEK_SET.try_into().map_err(io::Error::other)?,
            l_start: 0,
            l_len: 0,
            l_pid: 0,
        };
        fcntl(file.as_raw_fd(), FcntlArg::F_OFD_SETLK(&lock)).map_err(io::Error::from)?;
        if file.metadata()?.nlink() > 0 {
            return Ok(file);
        }
    }
}
