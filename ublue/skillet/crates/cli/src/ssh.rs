use anyhow::{anyhow, Context, Result};
use skillet_vm::transport::{SshConnection, SshTransport};
use std::path::{Path, PathBuf};

pub(super) fn connection(
    target: &str,
    port: u16,
    identity: Option<&Path>,
    known_hosts: Option<&Path>,
    home: Option<&Path>,
) -> Result<SshConnection> {
    let (user, address) = target
        .split_once('@')
        .filter(|(user, address)| !user.is_empty() && !address.is_empty())
        .ok_or_else(|| anyhow!("SSH target must be USER@HOST"))?;
    let known_hosts = match known_hosts {
        Some(path) => path.to_path_buf(),
        None => home
            .filter(|path| path.is_absolute())
            .ok_or_else(|| anyhow!("set an absolute HOME or pass --known-hosts"))?
            .join(".ssh/known_hosts"),
    };
    Ok(SshConnection {
        user: user.into(),
        address: address.into(),
        port,
        identity: identity.map(Path::to_path_buf),
        known_hosts,
    })
}

pub(super) fn transport(
    target: &str,
    port: u16,
    identity: Option<&Path>,
    known_hosts: Option<&Path>,
) -> Result<SshTransport> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    SshTransport::workstation(connection(
        target,
        port,
        identity,
        known_hosts,
        home.as_deref(),
    )?)
    .context("validating workstation SSH connection")
}

#[cfg(test)]
#[path = "ssh/tests.rs"]
mod tests;
