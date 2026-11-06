//! Guest-only entry point for scheduled PostgreSQL backups and explicit restores.
use crate::CliCommonError;
use clap::Subcommand;
use skillet_core::system::{AccountLookupResource, LinuxSystemResource};
use skillet_hosts::ServiceConfig;
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Create and atomically publish a custom-format dump for one app database.
    Backup {
        #[arg(long, default_value = "/etc/skillet/host")]
        host_file: PathBuf,
        #[arg(long)]
        instance: String,
    },
    /// Restore an explicitly selected archive and restart the application service.
    Restore {
        #[arg(long, default_value = "/etc/skillet/host")]
        host_file: PathBuf,
        #[arg(long)]
        instance: String,
        #[arg(long)]
        archive: PathBuf,
    },
}

pub fn dispatch(command: Commands) -> Result<(), CliCommonError> {
    let (host_file, instance, operation) = match command {
        Commands::Backup {
            host_file,
            instance,
        } => (host_file, instance, Operation::Backup),
        Commands::Restore {
            host_file,
            instance,
            archive,
        } => (host_file, instance, Operation::Restore(archive)),
    };
    let host = std::fs::read_to_string(host_file)?;
    let profile = skillet_hosts::profile_for_name(host.trim())
        .ok_or_else(|| CliCommonError::Config("unknown PostgreSQL host".into()))?;
    let config = profile
        .services
        .iter()
        .find_map(|service| match service.config {
            ServiceConfig::Joplin(config) if instance == "joplin" => {
                Some(skillet_joplin::backup_config(config))
            }
            _ => None,
        })
        .ok_or_else(|| {
            CliCommonError::Config("host does not declare this PostgreSQL instance".into())
        })?;
    let system = LinuxSystemResource::new();
    match operation {
        Operation::Backup => {
            let user = system.user_by_name(config.owner)?.ok_or_else(|| {
                CliCommonError::Config("PostgreSQL backup owner is missing".into())
            })?;
            let group = system.group_by_name(config.group)?.ok_or_else(|| {
                CliCommonError::Config("PostgreSQL backup group is missing".into())
            })?;
            let archive = skillet_postgres::backup::create(&config, user.uid, group.gid)
                .map_err(|error| CliCommonError::Config(error.to_string()))?;
            println!("PostgreSQL archive published: {}", archive.display());
        }
        Operation::Restore(archive) => {
            let user = system.user_by_name(config.owner)?.ok_or_else(|| {
                CliCommonError::Config("PostgreSQL backup owner is missing".into())
            })?;
            let group = system.group_by_name(config.group)?.ok_or_else(|| {
                CliCommonError::Config("PostgreSQL backup group is missing".into())
            })?;
            let recovery =
                skillet_postgres::backup::create_recovery(&config, user.uid, group.gid, &archive)
                    .map_err(|error| CliCommonError::Config(error.to_string()))?;
            println!("Pre-restore recovery archive: {}", recovery.display());
            skillet_postgres::backup::restore(&config, &archive)
                .map_err(|error| CliCommonError::Config(error.to_string()))?;
            println!("PostgreSQL archive restored into {instance}");
        }
    }
    Ok(())
}

enum Operation {
    Backup,
    Restore(PathBuf),
}
