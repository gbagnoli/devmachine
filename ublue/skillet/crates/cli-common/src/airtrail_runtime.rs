//! Guest-only commands for paired `AirTrail` database/upload backup sets.
use crate::CliCommonError;
use clap::Subcommand;
use skillet_core::system::{AccountLookupResource, LinuxSystemResource};
use skillet_hosts::ServiceConfig;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Create an atomic backup set while `AirTrail` is stopped
    Backup {
        #[arg(long, default_value = "/etc/skillet/host")]
        host_file: PathBuf,
    },
    /// Restore a selected paired database/uploads backup set
    Restore {
        #[arg(long, default_value = "/etc/skillet/host")]
        host_file: PathBuf,
        #[arg(long)]
        archive: PathBuf,
    },
}

pub fn dispatch(command: Commands) -> Result<(), CliCommonError> {
    let (host_file, operation) = match command {
        Commands::Backup { host_file } => (host_file, Operation::Backup),
        Commands::Restore { host_file, archive } => (host_file, Operation::Restore(archive)),
    };
    let host = std::fs::read_to_string(host_file)?;
    let profile = skillet_hosts::profile_for_name(host.trim())
        .ok_or_else(|| CliCommonError::Config("unknown AirTrail host".into()))?;
    let config = profile
        .services
        .iter()
        .find_map(|service| match service.config {
            ServiceConfig::Airtrail(config) => Some(config),
            _ => None,
        })
        .ok_or_else(|| CliCommonError::Config("host does not declare AirTrail".into()))?;
    let system = LinuxSystemResource::new();
    let owner = system
        .user_by_name(config.backup_owner)?
        .ok_or_else(|| CliCommonError::Config("AirTrail backup owner is missing".into()))?;
    let group = system
        .group_by_name(config.backup_group)?
        .ok_or_else(|| CliCommonError::Config("AirTrail backup group is missing".into()))?;
    match operation {
        Operation::Backup => {
            let archive = skillet_airtrail::backup::create(&config, owner.uid, group.gid)
                .map_err(|error| CliCommonError::Config(error.to_string()))?;
            println!("AirTrail backup set published: {}", archive.display());
        }
        Operation::Restore(archive) => {
            let recovery =
                skillet_airtrail::backup::restore(&config, &archive, owner.uid, group.gid)
                    .map_err(|error| CliCommonError::Config(error.to_string()))?;
            println!("Pre-restore AirTrail recovery set: {}", recovery.display());
            println!("AirTrail backup set restored");
        }
    }
    Ok(())
}

enum Operation {
    Backup,
    Restore(PathBuf),
}
