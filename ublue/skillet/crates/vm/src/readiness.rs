//! Signed boot and base/user-environment acceptance for retained guest captures.
use crate::{
    delivery::{checked, deliver, sha256},
    manifest::reject_symlinks,
    transport::{GuestCommand, GuestTransport, OwnershipCheckedTransport},
    Error, ManifestStore, Phase, Result, VmRun,
};
use std::{
    fs,
    io::Write as _,
    os::unix::fs::PermissionsExt as _,
    path::Path,
    time::{Duration, Instant},
};

/// Expectations belong to the selected profile, not the VM's mutable hostname.
pub struct ReadinessPolicy {
    pub signed_image: String,
    pub resolver_target: String,
    pub masked_units: Vec<String>,
    pub phase_timeout: Duration,
}

pub trait WaitClock {
    fn elapsed(&self) -> Duration;
    fn sleep(&self, duration: Duration);
}

pub struct MonotonicClock(Instant);
impl Default for MonotonicClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}
impl WaitClock for MonotonicClock {
    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// Probe transport uses a short deadline; installation may take several minutes.
pub struct ReadinessIo<'a, T> {
    pub probe: &'a T,
    pub operations: &'a T,
    pub ownership: &'a dyn Fn() -> Result<()>,
}

pub fn ready(
    run: &mut VmRun,
    store: &ManifestStore,
    run_dir: &Path,
    policy: &ReadinessPolicy,
    io: &ReadinessIo<'_, impl GuestTransport>,
    clock: &impl WaitClock,
    report: &mut impl FnMut(&str),
) -> Result<()> {
    if !matches!(run.phase, Phase::Started | Phase::Ready) {
        return Err(Error::Invalid("VM is not available for readiness".into()));
    }
    if policy.phase_timeout.is_zero() {
        return Err(Error::Invalid("readiness timeout must be positive".into()));
    }
    let host = run_dir.join(format!("source/files/skillet-{}", run.identity.host()));
    let generic = run_dir.join("source/files/skillet");
    let capture = run
        .captured
        .as_ref()
        .ok_or_else(|| Error::Invalid("creation artifact hashes are absent".into()))?;
    if sha256(&host)? != capture.host || sha256(&generic)? != capture.generic {
        return Err(Error::Invalid(
            "captured artifact bytes have changed".into(),
        ));
    }
    (io.ownership)()?;
    // Readiness can replace delivered executables and restart services. Mark
    // the old success stale before the first guest operation so a failed retry
    // cannot leave a Ready manifest behind.
    run.phase = Phase::Started;
    store.save(run)?;
    let probe = OwnershipCheckedTransport::new(io.probe, io.ownership);
    let operations = OwnershipCheckedTransport::new(io.operations, io.ownership);
    let result = (|| {
        wait(run, policy, &probe, io.ownership, clock, report, false)?;
        (io.ownership)()?;
        report("Installing captured Skillet binaries");
        deliver(run, &operations, &host, &generic)?;
        wait(run, policy, &probe, io.ownership, clock, report, true)?;
        (io.ownership)()?;
        expect(
            &operations,
            "cat",
            &["/etc/skillet/host"],
            run.identity.host(),
        )?;
        let status = checked(
            &operations,
            "sudo",
            &["-n", "rpm-ostree", "status", "--json"],
        )?;
        verify_signed_origin(&status, &policy.signed_image)?;
        report("Applying the base profile and user environment");
        let transport = &operations;
        for arguments in [
            vec!["reset-failed", "skillet-apply.service"],
            vec!["restart", "skillet-apply.service"],
            vec![
                "reset-failed",
                "brew-install.service",
                "dotfiles-install.service",
            ],
            vec!["start", "brew-install.service"],
            vec!["start", "dotfiles-install.service"],
        ] {
            let mut command = vec!["-n", "systemctl"];
            command.extend(arguments);
            checked(transport, "sudo", &command)?;
        }
        verify(run, run_dir, policy, transport)?;
        verify_root_profile(run, transport)
    })();
    if result.is_err() && (io.ownership)().is_ok() {
        // Diagnostics are best effort; never replace the original failure.
        let _ = diagnostics(&probe, run_dir);
    }
    result?;
    (io.ownership)()?;
    run.phase = Phase::Ready;
    run.deployed.clone_from(&run.captured);
    store.save(run)
}

fn verify_root_profile(run: &VmRun, transport: &impl GuestTransport) -> Result<()> {
    if run.root_profile == crate::install::RootProfile::Unencrypted {
        return Ok(());
    }
    let secure = checked(transport, "sudo", &["-n", "mokutil", "--sb-state"])?;
    let status = checked(transport, "sudo", &["-n", "cryptsetup", "status", "root"])?;
    let source = checked(transport, "findmnt", &["-T", "/var", "-n", "-o", "SOURCE"])?;
    let binding = checked(
        transport,
        "sudo",
        &[
            "-n",
            "clevis",
            "luks",
            "list",
            "-d",
            "/dev/disk/by-partlabel/root",
        ],
    )?;
    let secure = String::from_utf8_lossy(&secure);
    let status = String::from_utf8_lossy(&status);
    let source = String::from_utf8_lossy(&source);
    let binding = String::from_utf8_lossy(&binding);
    if !secure.lines().any(|line| line == "SecureBoot enabled")
        || !status.contains("LUKS2")
        || !source.trim().starts_with("/dev/mapper/root")
    {
        return Err(Error::Invalid(
            "encrypted readiness requires active Secure Boot and mapper-backed LUKS2 root".into(),
        ));
    }
    let entries: Vec<_> = binding
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if entries.len() != 1 {
        return Err(Error::Invalid(
            "root must have exactly one Clevis TPM binding".into(),
        ));
    }
    let (_, policy) = entries[0]
        .split_once("tpm2 ")
        .ok_or_else(|| Error::Invalid("unexpected root unlock pin".into()))?;
    let policy: serde_json::Value = serde_json::from_str(policy.trim().trim_matches('\''))?;
    if policy["pcr_bank"] != "sha256" || policy["pcr_ids"] != "7" {
        return Err(Error::Invalid(
            "root unlock policy is not the selected SHA256 PCR7 policy".into(),
        ));
    }
    Ok(())
}

fn wait(
    run: &VmRun,
    policy: &ReadinessPolicy,
    transport: &impl GuestTransport,
    ownership: &dyn Fn() -> Result<()>,
    clock: &impl WaitClock,
    report: &mut impl FnMut(&str),
    signed: bool,
) -> Result<()> {
    let phase = if signed { "signed boot" } else { "SSH" };
    report(&format!(
        "Waiting for {phase} on {}",
        run.identity.domain_name()
    ));
    let start = clock.elapsed();
    let mut next_report = Duration::from_mins(1);
    loop {
        // The owner may have disappeared even when a failed SSH probe was an
        // expected condition. Recheck before another probe or a backoff sleep.
        ownership()?;
        let elapsed = clock.elapsed().saturating_sub(start);
        if elapsed >= policy.phase_timeout {
            return Err(Error::Invalid(format!("timed out waiting for {phase}")));
        }
        let arguments: &[&str] = if signed {
            &["-n", "test", "-e", "/run/ucore-bootstrap-ready"]
        } else {
            &["-n", "true"]
        };
        if succeeds(transport, "sudo", arguments) {
            return Ok(());
        }
        if signed
            && succeeds(
                transport,
                "sudo",
                &[
                    "-n",
                    "systemctl",
                    "is-failed",
                    "--quiet",
                    "ucore-bootstrap.service",
                ],
            )
        {
            return Err(Error::Invalid("signed bootstrap service failed".into()));
        }
        if elapsed >= next_report {
            report(&format!(
                "Still waiting for {phase}; {} seconds elapsed",
                elapsed.as_secs()
            ));
            next_report = elapsed + Duration::from_mins(1);
        }
        clock.sleep(
            Duration::from_secs(5).min(
                policy
                    .phase_timeout
                    .saturating_sub(clock.elapsed().saturating_sub(start)),
            ),
        );
    }
}

fn succeeds(transport: &impl GuestTransport, program: &str, arguments: &[&str]) -> bool {
    transport
        .execute(&GuestCommand { program, arguments }, None)
        .is_ok_and(|output| output.status.success())
}

fn expect(
    transport: &impl GuestTransport,
    program: &str,
    args: &[&str],
    expected: &str,
) -> Result<()> {
    let output = checked(transport, program, args)?;
    if std::str::from_utf8(&output).ok().map(str::trim) != Some(expected) {
        return Err(Error::Invalid(format!(
            "guest {program} readiness assertion failed"
        )));
    }
    Ok(())
}

fn verify(
    run: &VmRun,
    run_dir: &Path,
    policy: &ReadinessPolicy,
    transport: &impl GuestTransport,
) -> Result<()> {
    let status = checked(transport, "sudo", &["-n", "rpm-ostree", "status", "--json"])?;
    save(run_dir, "final-status.json", &status)?;
    verify_signed_origin(&status, &policy.signed_image)?;
    let captured = run
        .captured
        .as_ref()
        .ok_or_else(|| Error::Invalid("creation artifact hashes are absent".into()))?;
    for (path, hash) in [
        (
            format!("/var/usrlocal/bin/skillet-{}", run.identity.host()),
            &captured.host,
        ),
        ("/var/usrlocal/bin/skillet".into(), &captured.generic),
    ] {
        if !crate::delivery::matches_installed(transport, &path, hash)? {
            return Err(Error::Invalid(
                "installed creation artifact differs after apply".into(),
            ));
        }
    }
    for (name, program, args) in [
        (
            "final-boot-id",
            "cat",
            vec!["/proc/sys/kernel/random/boot_id"],
        ),
        ("guest-skillet.sha256", "sha256sum", vec![]),
        (
            "guest-skillet-generic.sha256",
            "sha256sum",
            vec!["/var/usrlocal/bin/skillet"],
        ),
    ] {
        let host_path = format!("/var/usrlocal/bin/skillet-{}", run.identity.host());
        let args = if args.is_empty() {
            vec![host_path.as_str()]
        } else {
            args
        };
        save(run_dir, name, &checked(transport, program, &args)?)?;
    }
    expect(
        transport,
        "cat",
        &["/etc/skillet/host"],
        run.identity.host(),
    )?;
    expect(transport, "getenforce", &[], "Enforcing")?;
    expect(
        transport,
        "readlink",
        &["-f", "/etc/resolv.conf"],
        &policy.resolver_target,
    )?;
    checked(transport, "getent", &["hosts", "ghcr.io"])?;
    for unit in [
        "skillet-apply.service",
        "brew-install.service",
        "dotfiles-install.service",
    ] {
        expect(
            transport,
            "systemctl",
            &["show", "-p", "Result", "--value", unit],
            "success",
        )?;
        expect(
            transport,
            "systemctl",
            &["show", "-p", "ExecMainStatus", "--value", unit],
            "0",
        )?;
    }
    let definition = checked(transport, "systemctl", &["cat", "skillet-apply.service"])?;
    if !String::from_utf8_lossy(&definition).lines().any(|line| {
        line == "ExecStart=/var/usrlocal/bin/skillet apply --host-file /etc/skillet/host --phase base"
    }) {
        return Err(Error::Invalid("base unit does not use the stable host profile".into()));
    }
    for unit in &policy.masked_units {
        // systemctl is-enabled returns nonzero for a correctly masked unit.
        let output = transport.execute(
            &GuestCommand {
                program: "systemctl",
                arguments: &["is-enabled", unit],
            },
            None,
        )?;
        if String::from_utf8_lossy(&output.stdout).trim() != "masked" {
            return Err(Error::Invalid("required unit is not masked".into()));
        }
    }
    verify_user_environment(transport)?;
    diagnostics(transport, run_dir)?;
    Ok(())
}

fn verify_user_environment(transport: &impl GuestTransport) -> Result<()> {
    for path in [
        "/var/lib/ucore-bootstrap/brew-installed",
        "/var/lib/ucore-bootstrap/dotfiles-installed",
    ] {
        checked(transport, "test", &["-e", path])?;
    }
    for (link, source) in [
        (
            "/home/giacomo/.config/Brewfile",
            "/home/giacomo/.local/src/dotfiles/brew/Brewfile.core",
        ),
        (
            "/home/giacomo/.ssh/authorized_keys",
            "/home/giacomo/.local/src/dotfiles/ssh/authorized_keys",
        ),
    ] {
        let expected = checked(transport, "readlink", &["-f", source])?;
        let actual = checked(transport, "readlink", &["-f", link])?;
        if expected != actual || expected.is_empty() {
            return Err(Error::Invalid(
                "user environment link target differs".into(),
            ));
        }
    }
    checked(
        transport,
        "/home/linuxbrew/.linuxbrew/bin/brew",
        &[
            "bundle",
            "check",
            "--file",
            "/home/giacomo/.local/src/dotfiles/brew/Brewfile.core",
        ],
    )?;
    Ok(())
}

pub fn verify_signed_origin(status: &[u8], expected: &str) -> Result<()> {
    let status: serde_json::Value = serde_json::from_slice(status)?;
    let deployments = status["deployments"]
        .as_array()
        .ok_or_else(|| Error::Invalid("rpm-ostree deployments are absent".into()))?;
    let booted: Vec<_> = deployments
        .iter()
        .filter(|entry| entry["booted"] == true)
        .collect();
    if booted.len() != 1 {
        return Err(Error::Invalid(
            "expected exactly one booted deployment".into(),
        ));
    }
    let origin = booted[0]["container-image-reference"]
        .as_str()
        .ok_or_else(|| Error::Invalid("booted image reference is absent".into()))?;
    let signed = format!("ostree-image-signed:docker://{expected}");
    let digest = origin.strip_prefix(&format!("{signed}@sha256:"));
    if origin != format!("{signed}:latest")
        && !digest.is_some_and(|hash| {
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        })
    {
        return Err(Error::Invalid(
            "booted deployment is not the expected signed origin".into(),
        ));
    }
    Ok(())
}

fn diagnostics(transport: &impl GuestTransport, run_dir: &Path) -> Result<()> {
    let mut log = Vec::new();
    for arguments in [
        vec!["-n", "rpm-ostree", "status"],
        vec![
            "-n",
            "journalctl",
            "-b",
            "-u",
            "ucore-bootstrap.service",
            "-u",
            "skillet-apply.service",
            "-u",
            "brew-install.service",
            "-u",
            "dotfiles-install.service",
            "--no-pager",
            "-n",
            "200",
        ],
    ] {
        capture_diagnostic(transport, "sudo", &arguments, &mut log);
    }
    for (program, arguments) in [
        ("getenforce", vec![]),
        ("readlink", vec!["-f", "/etc/resolv.conf"]),
        ("cat", vec!["/etc/resolv.conf"]),
        ("getent", vec!["hosts", "ghcr.io"]),
        (
            "systemctl",
            vec!["status", "--no-pager", "skillet-apply.service"],
        ),
    ] {
        capture_diagnostic(transport, program, &arguments, &mut log);
    }
    save(run_dir, "readiness.log", &log)?;
    let mut user_log = Vec::new();
    for unit in ["brew-install.service", "dotfiles-install.service"] {
        for property in ["Result", "ExecMainStatus"] {
            capture_diagnostic(
                transport,
                "systemctl",
                &["show", "-p", property, "--value", unit],
                &mut user_log,
            );
        }
    }
    save(run_dir, "user-environment.log", &user_log)
}

fn capture_diagnostic(
    transport: &impl GuestTransport,
    program: &str,
    args: &[&str],
    log: &mut Vec<u8>,
) {
    log.extend(format!("{program} {args:?}\n").bytes());
    match transport.execute(
        &GuestCommand {
            program,
            arguments: args,
        },
        None,
    ) {
        Ok(output) => {
            log.extend(output.stdout);
            log.extend(output.stderr);
        }
        Err(_) => log.extend(b"Guest diagnostic command unavailable\n"),
    }
}

fn save(run_dir: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let path = run_dir.join(name);
    reject_symlinks(&path)?;
    let mut tmp = tempfile::NamedTempFile::new_in(run_dir)?;
    tmp.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|error| Error::Io(error.error))?;
    Ok(())
}

#[cfg(test)]
#[path = "readiness/tests.rs"]
mod tests;
