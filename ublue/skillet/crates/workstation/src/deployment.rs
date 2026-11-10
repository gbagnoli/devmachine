//! Verified host identity, authoritative image versions and temporary remote apply.
use skillet_vm::{
    artifacts,
    transport::{GuestCommand, GuestTransport},
    Error as VmError,
};
use std::{path::Path, process::Output};

#[derive(Debug, thiserror::Error)]
pub enum DeploymentError {
    #[error("unknown deployment host profile")]
    UnknownProfile,
    #[error("SSH connection/authentication failed (exit {code:?}): {diagnostic}\n{agent_hint}")]
    Connection {
        code: Option<i32>,
        diagnostic: String,
        agent_hint: &'static str,
    },
    #[error(
        "could not read /etc/skillet/host on the guest (exit {code:?}); no credentials delivered"
    )]
    IdentityRead { code: Option<i32> },
    #[error("SSH host identity differs from selected host; no credentials delivered")]
    HostMismatch,
    #[error(transparent)]
    Operation(#[from] VmError),
}
type Result<T> = std::result::Result<T, DeploymentError>;

fn ssh_agent_hint(socket: Option<&std::ffi::OsStr>) -> &'static str {
    if socket.is_some_and(|value| !value.is_empty()) {
        include_str!("help/ssh-agent-present.txt")
    } else {
        include_str!("help/ssh-agent-missing.txt")
    }
}

pub fn verify_host(host: &str, guest: &impl GuestTransport) -> Result<()> {
    if skillet_hosts::profile_for_name(host).is_none() {
        return Err(DeploymentError::UnknownProfile);
    }
    let output = guest.execute(
        &GuestCommand {
            program: "cat",
            arguments: &["/etc/skillet/host"],
        },
        None,
    )?;
    if output.status.code() == Some(255) {
        let diagnostic = String::from_utf8_lossy(&output.stderr)
            .chars()
            .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
            .take(1024)
            .collect::<String>();
        return Err(DeploymentError::Connection {
            code: output.status.code(),
            diagnostic: diagnostic.trim().into(),
            agent_hint: ssh_agent_hint(std::env::var_os("SSH_AUTH_SOCK").as_deref()),
        });
    }
    if !output.status.success() {
        return Err(DeploymentError::IdentityRead {
            code: output.status.code(),
        });
    }
    if std::str::from_utf8(&output.stdout).ok().map(str::trim) != Some(host) {
        return Err(DeploymentError::HostMismatch);
    }
    Ok(())
}

#[derive(Debug)]
pub struct Report {
    pub generic_version: String,
    pub host_version: String,
}

const APPLY_LOCK: &str = "/run/skillet-remote-apply.lock";

#[derive(Debug)]
pub struct TemporaryRunner {
    host: String,
    path: String,
    upload: String,
    token: String,
    generic_version: String,
}

impl TemporaryRunner {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn version(&self) -> &str {
        &self.generic_version
    }
}

/// Build and stage the generic CLI for one remote apply transaction. The
/// staged file uses the image executable label and never replaces the
/// authoritative `/usr/bin/skillet` path.
pub fn stage_temporary_runner(
    host: &str,
    workspace: &Path,
    guest: &impl GuestTransport,
) -> Result<TemporaryRunner> {
    verify_host(host, guest)?;
    let built = artifacts::build(&artifacts::BuildRequest {
        workspace,
        packages: &["skillet"],
        target: "x86_64-unknown-linux-musl",
        profile: "release",
    })?;
    let binary = built
        .get("skillet")
        .ok_or_else(|| VmError::Invalid("missing generic apply artifact".into()))?;
    let generic_version = artifacts::binary_version(binary)?;
    let token = uuid::Uuid::new_v4().simple().to_string();
    let path = format!("/var/usrlocal/bin/.skillet-remote-apply-{token}");
    let upload = format!("/var/tmp/skillet-remote-apply-{token}.upload");
    run_sudo(guest, &["mkdir", "-m", "0700", APPLY_LOCK], None).map_err(|_| {
        VmError::Invalid("could not acquire the remote-apply lock; another apply may be active, or a previous interrupted apply may have left /run/skillet-remote-apply.lock".into())
    })?;
    let staged =
        stage_one(guest, binary, &upload, &path).and_then(|()| verify_staged(guest, binary, &path));
    match staged {
        Ok(()) => Ok(TemporaryRunner {
            host: host.into(),
            path,
            upload,
            token,
            generic_version,
        }),
        Err(error) => {
            let cleanup = run_sudo(guest, &["rm", "-f", &path, &upload], None);
            if let Err(cleanup_error) = cleanup {
                return Err(VmError::Invalid(format!(
                    "temporary runner staging failed ({error}); cleanup also failed ({cleanup_error}), leaving the remote apply lock in place"
                ))
                .into());
            }
            run_sudo(guest, &["rmdir", APPLY_LOCK], None).map_err(|cleanup_error| {
                VmError::Invalid(format!(
                    "temporary runner staging failed ({error}); could not release the remote apply lock ({cleanup_error})"
                ))
            })?;
            Err(error)
        }
    }
}

fn stage_one(guest: &impl GuestTransport, binary: &Path, upload: &str, path: &str) -> Result<()> {
    guest.upload(binary, upload)?;
    run_sudo(
        guest,
        &["install", "-o", "0", "-g", "0", "-m", "0700", upload, path],
        None,
    )?;
    run_sudo(guest, &["restorecon", "-F", path], None)?;
    run_checked(guest, "rm", &["-f", upload], None)?;
    Ok(())
}

fn verify_staged(guest: &impl GuestTransport, binary: &Path, path: &str) -> Result<()> {
    let expected_hash = skillet_vm::delivery::sha256(binary)?;
    let expected_version = artifacts::binary_version(binary)?;
    let hash = run_checked(guest, "sha256sum", &[path], None)?;
    let actual_hash = std::str::from_utf8(&hash)
        .ok()
        .and_then(|output| output.split_whitespace().next())
        .unwrap_or_default();
    if actual_hash != expected_hash {
        return Err(
            VmError::Invalid("temporary apply binary failed hash verification".into()).into(),
        );
    }
    let version = run_checked(guest, path, &["--version"], None)?;
    if artifacts::version_text(&version)? != expected_version {
        return Err(VmError::Invalid(
            "temporary apply binary reports an unexpected version".into(),
        )
        .into());
    }
    Ok(())
}

/// Remove the unique apply artifact and release the host-wide runtime lock.
pub fn cleanup_temporary_runner(
    runner: &TemporaryRunner,
    guest: &impl GuestTransport,
) -> Result<()> {
    let profile = skillet_hosts::profile_for_name(&runner.host)
        .ok_or_else(|| VmError::Invalid("temporary apply host profile disappeared".into()))?;
    for step in crate::host_apply::steps(&profile) {
        if let crate::host_apply::Step::Activate(unit) = step {
            let directory = format!("/run/systemd/system/{unit}.d");
            let dropin = format!(
                "{directory}/zzzz-skillet-remote-apply-{}.conf",
                runner.token
            );
            run_sudo(
                guest,
                &[
                    "install", "-d", "-o", "0", "-g", "0", "-m", "0755", &directory,
                ],
                None,
            )?;
            run_sudo(guest, &["rm", "-f", &dropin], None)?;
            run_sudo(
                guest,
                &["rmdir", "--ignore-fail-on-non-empty", &directory],
                None,
            )?;
        }
    }
    run_sudo(guest, &["systemctl", "daemon-reload"], None)?;
    run_sudo(guest, &["rm", "-f", &runner.path, &runner.upload], None)?;
    run_sudo(guest, &["rmdir", APPLY_LOCK], None)?;
    Ok(())
}

/// Override only one apply transaction's executable, then remove the runtime
/// drop-in even when unit activation fails. The unit's persistent default stays
/// pointed at its `/usr/bin` image binary.
pub fn activate_with_runner(
    guest: &impl GuestTransport,
    unit: &str,
    binary: &str,
    token: &str,
    arguments: &[&str],
) -> Result<()> {
    let expected_binary = format!("/var/usrlocal/bin/.skillet-remote-apply-{token}");
    if binary != expected_binary
        || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
        || token.len() != 32
    {
        return Err(VmError::Invalid("invalid temporary apply unit or executable".into()).into());
    }
    skillet_vm::temporary_override::with_exec_start(guest, unit, token, binary, arguments, || {
        crate::host_apply::activate(guest, unit)
    })
    .map_err(Into::into)
}

fn run_sudo(
    guest: &impl GuestTransport,
    arguments: &[&str],
    input: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let mut command = vec!["-n"];
    command.extend_from_slice(arguments);
    run_checked(guest, "/usr/bin/sudo", &command, input)
}

fn run_checked(
    guest: &impl GuestTransport,
    program: &str,
    arguments: &[&str],
    input: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let output: Output = guest.execute(&GuestCommand { program, arguments }, input)?;
    if !output.status.success() {
        let diagnostic = String::from_utf8_lossy(&output.stderr)
            .chars()
            .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
            .take(1024)
            .collect::<String>();
        return Err(VmError::Invalid(format!(
            "remote operation {program} failed: {}",
            diagnostic.trim()
        ))
        .into());
    }
    Ok(output.stdout)
}

/// Inspect authoritative image-owned versions without changing the guest.
pub fn installed_versions(host: &str, guest: &impl GuestTransport) -> Result<Report> {
    if skillet_hosts::profile_for_name(host).is_none() {
        return Err(DeploymentError::UnknownProfile);
    }
    let read = |path: &str| -> Result<String> {
        let output = guest.execute(
            &GuestCommand {
                program: path,
                arguments: &["--version"],
            },
            None,
        )?;
        if !output.status.success() {
            return Err(VmError::Invalid(format!(
                "installed binary could not report its version: {path}"
            ))
            .into());
        }
        Ok(artifacts::version_text(&output.stdout)?)
    };
    Ok(Report {
        generic_version: read("/usr/bin/skillet")?,
        host_version: read(&format!("/usr/bin/skillet-{host}"))?,
    })
}

#[cfg(test)]
#[path = "deployment_tests.rs"]
mod tests;
