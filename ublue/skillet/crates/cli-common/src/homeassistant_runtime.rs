//! Guest adapters; no workstation vault or provider dependency.
use crate::CliCommonError;
use skillet_core::{
    credentials::CredentialManager,
    files::{FileMutationResource, LocalFileResource, Ownership, StorageResource},
    system::{AccountLookupResource, LinuxSystemResource},
};
use skillet_homeassistant::{
    backup::{self, Api},
    Config,
};
use skillet_hosts::ServiceConfig;
use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
    /// Apply the selected Home Assistant module without unrelated services
    Apply {
        #[arg(long, default_value = "/etc/skillet/host")]
        host_file: PathBuf,
    },
    /// Configure native encrypted backups and export completed original archives
    Backup {
        #[arg(long, default_value = "/etc/skillet/host")]
        host_file: PathBuf,
        /// Request a fresh backup and wait for completion before publication
        #[arg(long)]
        create: bool,
    },
}
fn declaration(path: &Path) -> Result<(String, Config), CliCommonError> {
    let host = std::fs::read_to_string(path)?;
    let profile = skillet_hosts::profile_for_name(host.trim())
        .ok_or_else(|| CliCommonError::Config("unknown host".into()))?;
    let config = profile
        .services
        .iter()
        .find_map(|service| match service.config {
            ServiceConfig::HomeAssistant(config) => Some(config),
            _ => None,
        })
        .ok_or_else(|| CliCommonError::Config("host does not select Home Assistant".into()))?;
    Ok((profile.id.as_str().into(), config))
}
pub fn dispatch(command: &Commands) -> Result<(), CliCommonError> {
    let host_file = match &command {
        Commands::Apply { host_file } | Commands::Backup { host_file, .. } => host_file,
    };
    let (host, config) = declaration(host_file)?;
    let files = LocalFileResource;
    let system = LinuxSystemResource::new();
    files
        .require_btrfs_subvolume_mount(Path::new("/var/lib/data"), Path::new("/var"), "/data")
        .map_err(|e| CliCommonError::Config(e.to_string()))?;
    match command {
        Commands::Apply { .. } => {
            skillet_homeassistant::apply(&system, &files, config).map_err(error)
        }
        Commands::Backup { create, .. } => {
            let state = Path::new("/var/lib/skillet/homeassistant");
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(state.join("backup.lock"))?;
            lock.try_lock().map_err(|_| {
                CliCommonError::Config("Home Assistant backup export already running".into())
            })?;
            let payload =
                Zeroizing::new(CredentialManager::new()?.read_secret(backup::CREDENTIAL)?);
            let input = backup::Settings::parse(&payload).map_err(error)?;
            if input.host != host
                || input.runtime_hostname != std::fs::read_to_string("/etc/hostname")?.trim()
            {
                return Err(error(skillet_homeassistant::Error::Invalid));
            }
            let user = system
                .user_by_name(config.owner)?
                .ok_or_else(|| CliCommonError::Config("backup owner missing".into()))?;
            let group = system
                .group_by_name(config.group)?
                .ok_or_else(|| CliCommonError::Config("backup group missing".into()))?;
            let mut api = backup::LocalApi::connect(&input.api_token).map_err(error)?;
            backup::configure(&mut api, &input).map_err(error)?;
            let info = api
                .request(serde_json::json!({"type":"backup/info"}))
                .map_err(error)?;
            if *create || backup::completed(&info).map_err(error)?.is_empty() {
                backup::create_and_wait(&mut api).map_err(error)?;
            }
            files
                .require_btrfs_subvolume(Path::new(config.backup_subvolume))
                .map_err(|e| CliCommonError::Config(e.to_string()))?;
            let store = backup::LocalArchives {
                source: Path::new(skillet_homeassistant::DATA).join("config/backups"),
                destination: config.backup_files.into(),
                uid: user.uid,
                gid: group.gid,
            };
            let outcome =
                backup::export(&mut api, &store, &input.backup_password).map_err(error)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| error(skillet_homeassistant::Error::Invalid))?
                .as_secs();
            let info = api
                .request(serde_json::json!({"type":"backup/info"}))
                .map_err(error)?;
            let completed = backup::latest_completed(&info).map_err(error)?;
            if completed > now {
                return Err(error(skillet_homeassistant::Error::Invalid));
            }
            let state = skillet_homeassistant::metrics::State {
                completed,
                exported: now,
            };
            files
                .ensure_file(
                    Path::new(skillet_homeassistant::metrics::STATE),
                    &serde_json::to_vec(&state)
                        .map_err(|_| error(skillet_homeassistant::Error::Invalid))?,
                    Some(0o600),
                    &Ownership::named(Some("root"), Some("root")),
                )
                .map_err(|e| CliCommonError::Config(e.to_string()))?;
            println!(
                "Home Assistant backup export completed ({} new, {} pruned)",
                outcome.published, outcome.pruned
            );
            Ok(())
        }
    }
}
fn error(e: skillet_homeassistant::Error) -> CliCommonError {
    CliCommonError::HomeAssistant(e)
}
