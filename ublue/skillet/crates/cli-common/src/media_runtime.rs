//! Guest entry point for authenticated media API reconciliation.
use crate::CliCommonError;
use clap::Subcommand;

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Test and reconcile declared application links, download clients and roots
    Reconcile {
        #[arg(long, default_value = "/etc/skillet/host")]
        host_file: std::path::PathBuf,
    },
}

pub fn dispatch(command: Commands) -> Result<(), CliCommonError> {
    let Commands::Reconcile { host_file } = command;
    let host = std::fs::read_to_string(host_file)?;
    let config = skillet_hosts::profile_for_name(host.trim())
        .and_then(|p| p.media_config())
        .ok_or_else(|| CliCommonError::Config("host does not declare media".into()))?;
    let payload = skillet_core::credentials::CredentialManager::new()?
        .read_secret(skillet_media::CREDENTIAL)?;
    let credentials = skillet_media::Credentials::parse(&payload)
        .map_err(|e| CliCommonError::Config(e.to_string()))?;
    skillet_media::runtime::run(config, &credentials)
        .map_err(|e| CliCommonError::Config(e.to_string()))?;
    println!("Media connections passed; declared links and library roots reconciled");
    Ok(())
}
