//! Thin physical-media command adapter.
use super::RootProfileName;
use anyhow::Result;
use std::path::PathBuf;

#[derive(clap::Subcommand, Debug)]
pub(super) enum Commands {
    /// Build a USB-ready live ISO; installation requires confirmation on the server
    Prepare(PrepareArgs),
}
#[derive(clap::Args, Debug)]
pub(super) struct PrepareArgs {
    host: String,
    /// Stable whole-disk path as seen on the destination server
    #[arg(long)]
    disk: String,
    /// Verified Fedora `CoreOS` `x86_64` live ISO
    #[arg(long)]
    iso: PathBuf,
    /// New private bundle directory, outside version control
    #[arg(long)]
    output: PathBuf,
    #[arg(long, value_enum, default_value_t = RootProfileName::Unencrypted)]
    root_profile: RootProfileName,
    /// Operator's Ed25519 public key, replacing inherited authorized keys
    #[arg(long)]
    ssh_public_key: PathBuf,
    /// Private file containing a fresh crypt-format bootstrap password hash
    #[arg(long)]
    password_hash_file: PathBuf,
    /// Trusted ublue DER signing certificate; mandatory for TPM media
    #[arg(long)]
    mok_certificate: Option<PathBuf>,
}
pub(super) fn dispatch(command: Commands) -> Result<()> {
    let Commands::Prepare(args) = command;
    let butane = super::butane_root()?;
    let workspace = super::workspace_root()?;
    let output = std::path::absolute(&args.output)?;
    let iso =
        skillet_workstation::installation::prepare(&skillet_workstation::installation::Request {
            host: &args.host,
            disk: &args.disk,
            root_profile: args.root_profile.policy(),
            iso: &args.iso,
            output: &output,
            public_key: &args.ssh_public_key,
            password_hash: &args.password_hash_file,
            mok_certificate: args.mok_certificate.as_deref(),
            butane: &butane,
            workspace: &workspace,
        })?;
    println!("Prepared {}\nTransfer the destination Ignition over SSH to the live server, then run: sudo /var/usrlocal/bin/skillet-install /path/to/destination.ign\nNo block device was written.", iso.display());
    Ok(())
}

#[cfg(test)]
#[path = "installation/tests.rs"]
mod tests;
