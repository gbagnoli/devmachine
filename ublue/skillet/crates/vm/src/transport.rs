//! Verified SSH execution/file delivery with explicit target and trust policy.
use crate::{Error, Result, SshTarget};
use std::{
    path::Path,
    process::{Command, ExitStatus, Output},
    time::Duration,
};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostKeyPolicy {
    Enroll,
    Verify,
}

pub struct SshTransport {
    target: SshTarget,
    policy: HostKeyPolicy,
    pub timeout: Duration,
}

impl SshTransport {
    pub fn new(target: SshTarget, policy: HostKeyPolicy) -> Result<Self> {
        if target.user.is_empty()
            || target.user.starts_with('-')
            || !target
                .user
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            || !valid_address(&target.address)
            || target.port == 0
        {
            return Err(Error::Invalid("invalid SSH user, address or port".into()));
        }
        for path in [&target.identity, &target.known_hosts] {
            crate::manifest::reject_symlinks(path)?;
            if !path.is_absolute() {
                return Err(Error::Invalid("SSH key paths must be absolute".into()));
            }
        }
        if !target.identity.is_file()
            || (policy == HostKeyPolicy::Verify && !target.known_hosts.is_file())
        {
            return Err(Error::Invalid(
                "SSH identity or recorded host key is absent".into(),
            ));
        }
        Ok(Self {
            target,
            policy,
            timeout: Duration::from_mins(10),
        })
    }

    fn options(&self, command: &mut Command, port_flag: &str) {
        command
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "IdentitiesOnly=yes",
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
            ))
            .arg("-o")
            .arg(format!(
                "UserKnownHostsFile={}",
                self.target.known_hosts.display()
            ))
            .arg("-i")
            .arg(&self.target.identity)
            .arg(port_flag)
            .arg(self.target.port.to_string());
    }

    fn remote_target(&self) -> String {
        format!("{}@{}", self.target.user, self.target.address)
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
            Ok(std::net::IpAddr::V6(_)) => {
                format!("{}@[{}]", self.target.user, self.target.address)
            }
            _ => self.remote_target(),
        };
        command.arg(source).arg(format!("{target}:{destination}"));
        Ok(command)
    }

    fn interactive_command(&self) -> Command {
        let mut command = Command::new("ssh");
        command.arg("-tt");
        self.options(&mut command, "-p");
        command.arg(self.remote_target());
        command
    }

    /// Run an interactive guest shell with the caller's terminal attached.
    /// Unlike captured guest commands this is intentionally unbounded until
    /// the user exits, and inherits stdin/stdout/stderr directly.
    pub fn interactive(&self) -> Result<ExitStatus> {
        self.interactive_command().status().map_err(Error::Io)
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
