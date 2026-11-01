use anyhow::{anyhow, Context, Result};
use skillet_vm::transport::{SshConnection, SshTransport};
use skillet_workstation::sudo::{PasswordPrompt, SudoTransport};
use std::{io::IsTerminal as _, path::Path, sync::Arc};

pub(super) fn connection(
    target: &str,
    port: Option<u16>,
    identity: Option<&Path>,
    known_hosts: Option<&Path>,
) -> Result<SshConnection> {
    let (user, address) = match target.split_once('@') {
        Some((user, address)) if !user.is_empty() && !address.is_empty() => {
            (Some(user.to_owned()), address)
        }
        Some(_) => return Err(anyhow!("SSH target must be HOST or USER@HOST")),
        None => (None, target),
    };
    Ok(SshConnection {
        user,
        address: address.into(),
        port,
        identity: identity.map(Path::to_path_buf),
        known_hosts: known_hosts.map(Path::to_path_buf),
    })
}

pub(super) fn transport(
    target: &str,
    port: Option<u16>,
    identity: Option<&Path>,
    known_hosts: Option<&Path>,
) -> Result<SudoTransport> {
    let ssh = SshTransport::workstation(connection(target, port, identity, known_hosts)?)
        .context("validating workstation SSH connection")?;
    let prompt: Option<PasswordPrompt> = std::io::stdin().is_terminal().then(|| {
        let message = format!("Sudo password for {target}: ");
        Arc::new(move || rpassword::prompt_password(&message).map(zeroize::Zeroizing::new))
            as PasswordPrompt
    });
    Ok(SudoTransport::new(ssh, super::workspace_root()?, prompt))
}

#[cfg(test)]
#[path = "ssh/tests.rs"]
mod tests;
