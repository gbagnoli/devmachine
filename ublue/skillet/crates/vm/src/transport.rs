//! Verified SSH execution/file delivery with explicit target and trust policy.
use crate::{Error, Result, SshTarget};
use std::{
    os::unix::process::CommandExt as _,
    path::Path,
    process::{Command, ExitStatus, Output},
    time::Duration,
};

/// A localhost listener forwarded only to the guest's localhost.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LoopbackForward {
    local: u16,
    guest: u16,
}

impl std::str::FromStr for LoopbackForward {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let invalid =
            || Error::Invalid("forward must be LOCAL:GUEST with nonzero TCP ports".into());
        let (local, guest) = value.split_once(':').ok_or_else(invalid)?;
        let local = local.parse::<u16>().map_err(|_| invalid())?;
        let guest = guest.parse::<u16>().map_err(|_| invalid())?;
        if local == 0 || guest == 0 {
            return Err(invalid());
        }
        Ok(Self { local, guest })
    }
}

pub struct GuestCommand<'a> {
    pub program: &'a str,
    pub arguments: &'a [&'a str],
}

pub trait GuestTransport {
    fn execute(&self, command: &GuestCommand<'_>, input: Option<&[u8]>) -> Result<Output>;
    fn upload(&self, source: &Path, destination: &str) -> Result<()>;
}

/// Check that the target is still owned immediately before and after each
/// remote operation. Lifecycle callers keep their run lock for the wrapper's
/// lifetime and supply the ownership check for the selected backend.
pub struct OwnershipCheckedTransport<'a, T> {
    transport: &'a T,
    ownership: &'a dyn Fn() -> Result<()>,
}

impl<'a, T> OwnershipCheckedTransport<'a, T> {
    pub fn new(transport: &'a T, ownership: &'a dyn Fn() -> Result<()>) -> Self {
        Self {
            transport,
            ownership,
        }
    }
}

impl<T: GuestTransport> GuestTransport for OwnershipCheckedTransport<'_, T> {
    fn execute(&self, command: &GuestCommand<'_>, input: Option<&[u8]>) -> Result<Output> {
        (self.ownership)()?;
        let result = self.transport.execute(command, input);
        (self.ownership)()?;
        result
    }

    fn upload(&self, source: &Path, destination: &str) -> Result<()> {
        (self.ownership)()?;
        let result = self.transport.upload(source, destination);
        (self.ownership)()?;
        result
    }
}

/// The same ownership boundary also guards reverse credential forwarding.
pub trait ReverseUnixForward {
    fn reverse_unix_forward(&self, remote: &str, local: &Path) -> Result<UnixForward>;
}
impl<T> Clone for OwnershipCheckedTransport<'_, T> {
    fn clone(&self) -> Self {
        Self {
            transport: self.transport,
            ownership: self.ownership,
        }
    }
}
impl<T: ReverseUnixForward> ReverseUnixForward for OwnershipCheckedTransport<'_, T> {
    fn reverse_unix_forward(&self, remote: &str, local: &Path) -> Result<UnixForward> {
        (self.ownership)()?;
        let result = self.transport.reverse_unix_forward(remote, local);
        (self.ownership)()?;
        result
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostKeyPolicy {
    Enroll,
    Verify,
}

#[derive(Clone)]
pub struct SshTransport {
    target: SshConnection,
    policy: HostKeyPolicy,
    agent_socket: Option<std::path::PathBuf>,
    pub timeout: Duration,
}

/// SSH configuration for a workstation target. An omitted identity delegates
/// authentication to OpenSSH configuration, default keys and the SSH agent.
#[derive(Clone)]
pub struct SshConnection {
    pub user: Option<String>,
    pub address: String,
    pub port: Option<u16>,
    pub identity: Option<std::path::PathBuf>,
    pub known_hosts: Option<std::path::PathBuf>,
}

impl From<SshTarget> for SshConnection {
    fn from(target: SshTarget) -> Self {
        Self {
            user: Some(target.user),
            address: target.address,
            port: Some(target.port),
            identity: Some(target.identity),
            known_hosts: Some(target.known_hosts),
        }
    }
}

impl SshTransport {
    pub fn new(target: SshTarget, policy: HostKeyPolicy) -> Result<Self> {
        Self::from_connection(target.into(), policy)
    }

    /// Production connections always require a previously trusted host key.
    pub fn workstation(target: SshConnection) -> Result<Self> {
        Self::from_connection(target, HostKeyPolicy::Verify)
    }

    fn from_connection(target: SshConnection, policy: HostKeyPolicy) -> Result<Self> {
        if target.user.as_ref().is_some_and(|user| {
            user.is_empty()
                || user.starts_with('-')
                || !user
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        }) || !valid_address(&target.address)
            || target.port == Some(0)
        {
            return Err(Error::Invalid("invalid SSH user, address or port".into()));
        }
        for path in target.identity.iter().chain(target.known_hosts.iter()) {
            crate::manifest::reject_symlinks(path)?;
            if !path.is_absolute() {
                return Err(Error::Invalid("SSH key paths must be absolute".into()));
            }
        }
        if target.identity.as_ref().is_some_and(|path| !path.is_file())
            || (policy == HostKeyPolicy::Verify
                && target
                    .known_hosts
                    .as_ref()
                    .is_some_and(|path| !path.is_file()))
        {
            return Err(Error::Invalid(
                "SSH identity or recorded host key is absent".into(),
            ));
        }
        Ok(Self {
            target,
            policy,
            agent_socket: None,
            timeout: Duration::from_mins(10),
        })
    }

    /// Scope an SSH-agent override to this connection, without mutating the process environment.
    pub fn with_agent_socket(mut self, socket: std::path::PathBuf) -> Result<Self> {
        if !socket.is_absolute() {
            return Err(Error::Invalid("SSH agent socket must be absolute".into()));
        }
        self.agent_socket = Some(socket);
        Ok(self)
    }

    fn options(&self, command: &mut Command, port_flag: &str) {
        if let Some(socket) = &self.agent_socket {
            command
                .env("SSH_AUTH_SOCK", socket)
                .arg("-o")
                .arg(format!("IdentityAgent={}", socket.display()));
        }
        command
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=5",
                "-o",
                "ConnectionAttempts=1",
                "-o",
            ])
            .arg(format!(
                "StrictHostKeyChecking={}",
                match self.policy {
                    HostKeyPolicy::Enroll => "accept-new",
                    HostKeyPolicy::Verify => "yes",
                }
            ));
        if let Some(known_hosts) = &self.target.known_hosts {
            command
                .arg("-o")
                .arg(format!("UserKnownHostsFile={}", known_hosts.display()));
        }
        if let Some(identity) = &self.target.identity {
            command
                .args(["-o", "IdentitiesOnly=yes", "-i"])
                .arg(identity);
        }
        if let Some(port) = self.target.port {
            command.arg(port_flag).arg(port.to_string());
        }
    }

    fn remote_target(&self) -> String {
        self.target.user.as_ref().map_or_else(
            || self.target.address.clone(),
            |user| format!("{user}@{}", self.target.address),
        )
    }

    fn command(&self, request: &GuestCommand<'_>) -> Result<Command> {
        if request.program.is_empty()
            || request.program.contains('\0')
            || request
                .arguments
                .iter()
                .any(|argument| argument.contains('\0'))
        {
            return Err(Error::Invalid(
                "empty or NUL-containing guest command".into(),
            ));
        }
        let mut command = Command::new("ssh");
        command.arg("-T");
        self.options(&mut command, "-p");
        // OpenSSH uses a remote shell to launch executables. Quote each argument
        // literally; never construct a shell program or interpolate payloads.
        let remote = std::iter::once(request.program)
            .chain(request.arguments.iter().copied())
            .map(quote_argument)
            .collect::<Vec<_>>()
            .join(" ");
        command.arg(self.remote_target()).arg(remote);
        Ok(command)
    }

    fn upload_command(&self, source: &Path, destination: &str) -> Result<Command> {
        if !destination.starts_with("/var/tmp/")
            || !destination.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.')
            })
            || destination
                .split('/')
                .any(|part| matches!(part, "." | ".."))
        {
            return Err(Error::Invalid(
                "upload destination must be a literal staging path".into(),
            ));
        }
        crate::manifest::reject_symlinks(source)?;
        if !source.is_absolute() || !source.is_file() {
            return Err(Error::Invalid(
                "upload source must be an existing absolute file".into(),
            ));
        }
        let mut command = Command::new("scp");
        command.arg("-O");
        self.options(&mut command, "-P");
        let target = match self.target.address.parse::<std::net::IpAddr>() {
            Ok(std::net::IpAddr::V6(_)) => self.target.user.as_ref().map_or_else(
                || format!("[{}]", self.target.address),
                |user| format!("{user}@[{}]", self.target.address),
            ),
            _ => self.remote_target(),
        };
        command.arg(source).arg(format!("{target}:{destination}"));
        Ok(command)
    }

    fn interactive_command(&self, forwards: &[LoopbackForward]) -> Result<Command> {
        let mut command = Command::new("ssh");
        command.arg("-tt");
        self.options(&mut command, "-p");
        let mut ports = std::collections::BTreeSet::new();
        for forward in forwards {
            if !ports.insert(forward.local) {
                return Err(Error::Invalid("duplicate local forwarding port".into()));
            }
            command.arg("-L").arg(format!(
                "127.0.0.1:{}:127.0.0.1:{}",
                forward.local, forward.guest
            ));
        }
        if !forwards.is_empty() {
            command.args([
                "-o",
                "ExitOnForwardFailure=yes",
                "-o",
                "ForkAfterAuthentication=no",
                "-o",
                "ControlMaster=no",
                "-o",
                "ControlPath=none",
                "-o",
                "ControlPersist=no",
            ]);
        }
        command.arg(self.remote_target());
        Ok(command)
    }

    /// A private remote Unix socket forwarded to a local credential broker.
    /// The caller creates the remote parent privately and owns its cleanup.
    pub fn reverse_unix_forward(&self, remote: &str, local: &Path) -> Result<UnixForward> {
        if !remote.starts_with("/var/tmp/skillet-sudo-")
            || remote.split('/').any(|part| matches!(part, "." | ".."))
            || !remote
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/-_.".contains(&b))
            || !local.is_absolute()
            || local.as_os_str().as_encoded_bytes().contains(&b':')
        {
            return Err(Error::Invalid(
                "invalid private askpass forwarding path".into(),
            ));
        }
        let mut command = Command::new("ssh");
        self.options(&mut command, "-p");
        command
            .args([
                "-N",
                "-T",
                "-o",
                "ExitOnForwardFailure=yes",
                "-o",
                "ForkAfterAuthentication=no",
                "-o",
                "ControlMaster=no",
                "-o",
                "ControlPath=none",
                "-o",
                "ControlPersist=no",
                "-o",
                "ServerAliveInterval=5",
                "-o",
                "ServerAliveCountMax=2",
                "-o",
                "StreamLocalBindMask=0177",
                "-o",
                "StreamLocalBindUnlink=no",
                "-R",
            ])
            .arg(format!("{remote}:{}", local.display()))
            .arg(self.remote_target())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .process_group(0);
        let mut forward = UnixForward(command.spawn()?);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let mut probe = self.clone();
        probe.timeout = Duration::from_secs(5);
        loop {
            if forward.0.try_wait()?.is_some() {
                return Err(Error::Invalid(
                    "SSH askpass forwarding failed; check server forwarding policy".into(),
                ));
            }
            if probe
                .execute(
                    &GuestCommand {
                        program: "test",
                        arguments: &["-S", remote],
                    },
                    None,
                )?
                .status
                .success()
            {
                return Ok(forward);
            }
            if std::time::Instant::now() >= deadline {
                return Err(Error::Timeout);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Run an interactive guest shell with the caller's terminal attached.
    /// Unlike captured guest commands this is intentionally unbounded until
    /// the user exits, and inherits stdin/stdout/stderr directly.
    pub fn interactive(&self) -> Result<ExitStatus> {
        self.interactive_with_forwards(&[])
    }

    /// Keep forwards scoped to the interactive SSH process and caller's run lock.
    pub fn interactive_with_forwards(&self, forwards: &[LoopbackForward]) -> Result<ExitStatus> {
        self.interactive_command(forwards)?
            .status()
            .map_err(Error::Io)
    }
}

impl ReverseUnixForward for SshTransport {
    fn reverse_unix_forward(&self, remote: &str, local: &Path) -> Result<UnixForward> {
        Self::reverse_unix_forward(self, remote, local)
    }
}

impl GuestTransport for SshTransport {
    fn execute(&self, command: &GuestCommand<'_>, input: Option<&[u8]>) -> Result<Output> {
        crate::process::capture_with_input(self.command(command)?, self.timeout, input)
    }
    fn upload(&self, source: &Path, destination: &str) -> Result<()> {
        let output =
            crate::process::capture(self.upload_command(source, destination)?, self.timeout)?;
        if !output.status.success() {
            return Err(Error::Guest {
                operation: "upload".into(),
                code: output.status.code(),
            });
        }
        Ok(())
    }
}

/// Owns the SSH forwarding process. Never leaves a detached master behind.
pub struct UnixForward(std::process::Child);
impl Drop for UnixForward {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            if let Ok(id) = i32::try_from(self.0.id()) {
                let _ = nix::sys::signal::killpg(
                    nix::unistd::Pid::from_raw(id),
                    nix::sys::signal::Signal::SIGKILL,
                );
            }
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

fn quote_argument(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn valid_address(value: &str) -> bool {
    if value.parse::<std::net::IpAddr>().is_ok() {
        return true;
    }
    !value.is_empty()
        && value.len() <= 253
        && value.trim_end_matches('.').split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

#[cfg(test)]
#[path = "transport/tests.rs"]
mod tests;
