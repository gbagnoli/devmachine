//! Guest-side native `UniFi` backup publication.
use crate::CliCommonError;
use skillet_core::{
    files::{LocalFileResource, StorageResource as _},
    system::{AccountLookupResource as _, LinuxSystemResource},
};
use std::{fs::OpenOptions, os::unix::fs::OpenOptionsExt as _, path::PathBuf, time::SystemTime};

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
    /// Export completed automatic backups into the caller's shared Syncthing folder
    Export {
        #[arg(long, default_value = "/etc/skillet/host")]
        host_file: PathBuf,
    },
}

pub fn dispatch(command: Commands) -> Result<(), CliCommonError> {
    let Commands::Export { host_file } = command;
    let host = std::fs::read_to_string(host_file)?;
    let config = skillet_hosts::profile_for_name(host.trim())
        .and_then(|profile| profile.unifi_backup_config())
        .ok_or_else(|| {
            CliCommonError::Config(
                "host does not declare UniFi with shared Syncthing storage".into(),
            )
        })?;
    let _maintenance = skillet_core::maintenance::writer()?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open("/run/skillet-unifi-backup.lock")?;
    lock.try_lock()
        .map_err(|_| CliCommonError::Config("UniFi export already running".into()))?;
    let files = LocalFileResource::new();
    files
        .require_btrfs_subvolume_mount(
            std::path::Path::new("/var/lib/data"),
            std::path::Path::new("/var"),
            "/data",
        )
        .map_err(|error| CliCommonError::Config(error.to_string()))?;
    let system = LinuxSystemResource::new();
    let user = system
        .user_by_name(config.owner)?
        .ok_or_else(|| CliCommonError::Config("UniFi backup owner missing".into()))?;
    let group = system
        .group_by_name(config.group)?
        .ok_or_else(|| CliCommonError::Config("UniFi backup group missing".into()))?;
    let outcome = skillet_unifi::backup::export(
        std::path::Path::new(skillet_unifi::backup::SOURCE),
        &config.destination(),
        user.uid,
        group.gid,
        SystemTime::now(),
    )
    .map_err(|error| CliCommonError::Config(error.to_string()))?;
    println!(
        "UniFi backups: {} exported, {} pruned",
        outcome.published, outcome.pruned
    );
    Ok(())
}
