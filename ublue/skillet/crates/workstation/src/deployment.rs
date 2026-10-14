//! Production deployment uses the same Cargo artifact and delivery owners as VMs.
use skillet_vm::{
    artifacts, delivery,
    transport::{GuestCommand, GuestTransport},
    Error as VmError,
};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum DeploymentError {
    #[error("unknown deployment host profile")]
    UnknownProfile,
    #[error("SSH connection/authentication failed (exit {code:?}): {diagnostic}. For an encrypted key, load it into the SSH agent with ssh-add, then retry")]
    Connection {
        code: Option<i32>,
        diagnostic: String,
    },
    #[error("could not read /etc/skillet/host on the guest (exit {code:?}); no binaries or credentials delivered")]
    IdentityRead { code: Option<i32> },
    #[error("SSH host identity differs from selected host; no binaries or credentials delivered")]
    HostMismatch,
    #[error(transparent)]
    Operation(#[from] VmError),
}
type Result<T> = std::result::Result<T, DeploymentError>;

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

pub fn deploy(host: &str, workspace: &Path, guest: &impl GuestTransport) -> Result<Report> {
    verify_host(host, guest)?;
    let package = format!("skillet-{host}");
    let built = artifacts::build(&artifacts::BuildRequest {
        workspace,
        packages: &["skillet", &package],
        target: "x86_64-unknown-linux-musl",
        profile: "release",
    })?;
    let generic = built
        .get("skillet")
        .ok_or_else(|| VmError::Invalid("missing generic deployment artifact".into()))?;
    let host_binary = built
        .get(&package)
        .ok_or_else(|| VmError::Invalid("missing host deployment artifact".into()))?;
    let generic_version = artifacts::binary_version(generic)?;
    let host_version = artifacts::binary_version(host_binary)?;
    verify_host(host, guest)?;
    delivery::deliver_host(host, guest, host_binary, generic)?;
    let installed = installed_versions(host, guest)?;
    if installed.generic_version != generic_version || installed.host_version != host_version {
        return Err(VmError::Invalid(
            "installed binary version differs from the selected artifact".into(),
        )
        .into());
    }
    Ok(Report {
        generic_version,
        host_version,
    })
}

/// Inspect guest versions without deploying or activating services.
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
        generic_version: read("/var/usrlocal/bin/skillet")?,
        host_version: read(&format!("/var/usrlocal/bin/skillet-{host}"))?,
    })
}

#[cfg(test)]
#[path = "deployment_tests.rs"]
mod tests;
