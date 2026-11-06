//! Thin native Gokapi operations, shared by generic and per-host executables.
use crate::CliCommonError;
use skillet_core::{
    credentials::CredentialManager,
    system::{AccountLookupResource, LinuxSystemResource},
};
use skillet_hosts::ServiceConfig;
use std::path::PathBuf;
#[derive(Debug, clap::Subcommand)]
pub enum Commands {
    /// Initialize native header authentication and reconcile registered accounts
    Initialize {
        #[arg(long, default_value = "/etc/skillet/host")]
        host_file: PathBuf,
    },
    /// Publish a consistent configuration/SQLite/upload backup set
    Backup {
        #[arg(long, default_value = "/etc/skillet/host")]
        host_file: PathBuf,
    },
    /// Restore a selected set, retaining a pre-restore recovery set
    Restore {
        #[arg(long, default_value = "/etc/skillet/host")]
        host_file: PathBuf,
        #[arg(long)]
        archive: PathBuf,
    },
}
pub fn dispatch(command: Commands) -> Result<(), CliCommonError> {
    let host_file = match &command {
        Commands::Initialize { host_file }
        | Commands::Backup { host_file }
        | Commands::Restore { host_file, .. } => host_file,
    };
    let host = std::fs::read_to_string(host_file)?;
    let profile = skillet_hosts::profile_for_name(host.trim())
        .ok_or_else(|| CliCommonError::Config("unknown Gokapi host".into()))?;
    let config = profile
        .services
        .iter()
        .find_map(|service| match service.config {
            ServiceConfig::Gokapi(config) => Some(config),
            _ => None,
        })
        .ok_or_else(|| CliCommonError::Config("host does not declare Gokapi".into()))?;
    let system = LinuxSystemResource::new();
    let uid = system
        .user_by_name(config.owner)?
        .ok_or_else(|| CliCommonError::Config("Gokapi owner absent".into()))?
        .uid;
    let gid = system
        .group_by_name(config.group)?
        .ok_or_else(|| CliCommonError::Config("Gokapi group absent".into()))?
        .gid;
    let failure = |error: skillet_gokapi::Error| CliCommonError::Config(error.to_string());
    match command {
        Commands::Initialize { .. } => {
            let credentials = CredentialManager::new()?;
            let sites = skillet_caddy::CaddySites::parse(
                &credentials.read_secret(skillet_hosts::CADDY_SITES_CREDENTIAL)?,
                host.trim(),
                &profile.ui_services(),
            )
            .map_err(|error| CliCommonError::Config(error.to_string()))?;
            let settings = skillet_hosts::gokapi_settings(
                &sites,
                &credentials.read_secret(skillet_gokapi::ADMIN_CREDENTIAL)?,
            )
            .map_err(|error| CliCommonError::Config(error.to_string()))?;
            skillet_gokapi::initial::prepare_proxies(
                &skillet_caddy::private_network(host.trim(), "gokapi").unit_name,
            )
            .map_err(failure)?;
            skillet_gokapi::initial::initialize(config, &settings, uid, gid).map_err(failure)?;
            println!("Gokapi native account preparation complete");
        }
        Commands::Backup { .. } => println!(
            "Gokapi backup: {}",
            skillet_gokapi::backup::create(config, uid, gid)
                .map_err(failure)?
                .display()
        ),
        Commands::Restore { archive, .. } => println!(
            "Gokapi restored; recovery set: {}",
            skillet_gokapi::backup::restore(config, &archive, uid, gid)
                .map_err(failure)?
                .display()
        ),
    }
    Ok(())
}
