use clap::Parser;
use serde::Serialize;
use skillet_core::credentials::{CredentialInputs, CredentialManager};
use skillet_core::files::{FileResource, LocalFileResource};
use skillet_core::recorder::Recorder;
use skillet_core::system::{LinuxSystemResource, SystemResource};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

pub mod hosts;
use hosts::ApplyPhase;

#[derive(Error, Debug)]
pub enum CliCommonError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("System error: {0}")]
    System(#[from] skillet_core::system::SystemError),
    #[error("Failed to set default tracing subscriber: {0}")]
    SetLogger(#[from] tracing::subscriber::SetGlobalDefaultError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Yaml(#[from] serde_yml::Error),
    #[error("Credential installation error: {0}")]
    CredentialInstall(#[from] skillet_core::credential_install::CredentialInstallError),
    #[error("Systemd credential error: {0}")]
    SystemdCredential(#[from] skillet_core::credentials::CredentialError),
    #[error("Configuration apply failed: {apply}; diagnostic recording also failed: {recording}")]
    ApplyAndRecord { apply: String, recording: String },
}

#[derive(Serialize)]
struct DiagnosticRecording<'a> {
    format_version: u32,
    host: &'a str,
    outcome: &'static str,
    operations: Vec<skillet_core::resource_op::RecordedOperation>,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct HostArgs {
    #[command(subcommand)]
    pub command: HostCommands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(clap::Subcommand, Debug)]
pub enum HostCommands {
    /// Apply configuration
    Apply {
        #[arg(long, value_enum, default_value_t = ApplyPhase::Full)]
        phase: ApplyPhase,
        /// Optional: Output recorded actions to this file path
        #[arg(long)]
        record: Option<PathBuf>,
    },
    /// Inspect or install an encrypted host credential
    Credential {
        #[command(subcommand)]
        command: CredentialCommands,
    },
}

#[derive(clap::Subcommand, Debug)]
pub enum CredentialCommands {
    /// Print `present` if a host credential or Podman secret exists, otherwise `absent`
    State { name: String },
    /// Encrypt stdin for this host, install it, and start the consuming unit
    Install {
        name: String,
        unit: String,
        /// Save the encrypted credential without starting its consuming unit.
        #[arg(long)]
        no_start: bool,
    },
}

pub fn run_host<F>(hostname: &str, apply_fn: F) -> Result<(), CliCommonError>
where
    F: Fn(
        ApplyPhase,
        &dyn SystemResource,
        &dyn FileResource,
        &CredentialInputs,
    ) -> Result<(), String>,
{
    let args = HostArgs::parse();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(if args.verbose {
            Level::DEBUG
        } else {
            Level::INFO
        })
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    match args.command {
        HostCommands::Apply { record, phase } => {
            handle_host_apply(hostname, phase, record, |system, files, credentials| {
                apply_fn(phase, system, files, credentials)
            })
        }
        HostCommands::Credential { command } => {
            match command {
                CredentialCommands::State { name } => {
                    println!("{}", skillet_core::credential_install::state(&name)?);
                }
                CredentialCommands::Install {
                    name,
                    unit,
                    no_start,
                } => skillet_core::credential_install::install(&name, &unit, !no_start)?,
            }
            Ok(())
        }
    }
}

pub fn handle_apply<F>(
    hostname: &str,
    record_path: Option<PathBuf>,
    apply_fn: F,
) -> Result<(), CliCommonError>
where
    F: Fn(&dyn SystemResource, &dyn FileResource) -> Result<(), String>,
{
    handle_apply_with_credentials(
        hostname,
        record_path,
        &CredentialInputs::default(),
        |system, files, _| apply_fn(system, files),
    )
}

pub fn handle_host_apply<F>(
    hostname: &str,
    phase: ApplyPhase,
    record_path: Option<PathBuf>,
    apply_fn: F,
) -> Result<(), CliCommonError>
where
    F: Fn(&dyn SystemResource, &dyn FileResource, &CredentialInputs) -> Result<(), String>,
{
    let phase = match phase {
        ApplyPhase::Base => skillet_hosts::HostApplyPhase::Base,
        ApplyPhase::Full => skillet_hosts::HostApplyPhase::Full,
        ApplyPhase::Caddy => skillet_hosts::HostApplyPhase::Caddy,
        ApplyPhase::Ddns => skillet_hosts::HostApplyPhase::Ddns,
        ApplyPhase::Datadog => skillet_hosts::HostApplyPhase::Datadog,
    };
    let required = skillet_hosts::credentials_for_phase(hostname, phase)
        .map_err(|error| CliCommonError::Config(error.to_string()))?;
    let mut credentials = CredentialInputs::default();
    if !required.is_empty() {
        let manager = CredentialManager::new()?;
        for name in required {
            credentials.insert(name, manager.read_secret(name)?);
        }
    }
    handle_apply_with_credentials(hostname, record_path, &credentials, apply_fn)
}

fn handle_apply_with_credentials<F>(
    hostname: &str,
    record_path: Option<PathBuf>,
    credentials: &CredentialInputs,
    apply_fn: F,
) -> Result<(), CliCommonError>
where
    F: Fn(&dyn SystemResource, &dyn FileResource, &CredentialInputs) -> Result<(), String>,
{
    info!("Starting Skillet configuration for {}...", hostname);

    let system = LinuxSystemResource::new();
    let files = LocalFileResource::new();

    if let Some(path) = record_path {
        handle_recorded_apply(hostname, &path, system, files, credentials, apply_fn)?;
    } else {
        apply_fn(&system, &files, credentials).map_err(CliCommonError::Config)?;
    }

    info!("Configuration applied successfully.");
    Ok(())
}

fn handle_recorded_apply<S, F, Apply>(
    hostname: &str,
    path: &std::path::Path,
    system: S,
    files: F,
    credentials: &CredentialInputs,
    apply_fn: Apply,
) -> Result<(), CliCommonError>
where
    S: SystemResource,
    F: FileResource,
    Apply: Fn(&dyn SystemResource, &dyn FileResource, &CredentialInputs) -> Result<(), String>,
{
    let recorder_system = Recorder::new(system);
    let recorder_files = Recorder::with_ops(files, recorder_system.shared_ops());
    let apply_result = apply_fn(&recorder_system, &recorder_files, credentials);
    let diagnostic = DiagnosticRecording {
        format_version: 2,
        host: hostname,
        outcome: if apply_result.is_ok() {
            "succeeded"
        } else {
            "failed"
        },
        operations: recorder_system.get_ops(),
    };
    let record_result = persist_recording(path, &diagnostic);
    if let Err(apply) = apply_result {
        if let Err(recording) = record_result {
            return Err(CliCommonError::ApplyAndRecord { apply, recording });
        }
        return Err(CliCommonError::Config(apply));
    }
    record_result.map_err(CliCommonError::Config)?;
    info!("Recording saved to {}", path.display());
    Ok(())
}

fn persist_recording(
    path: &std::path::Path,
    diagnostic: &DiagnosticRecording<'_>,
) -> Result<(), String> {
    use std::io::Write as _;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    fs::create_dir_all(parent).map_err(|error| format!("create {}: {error}", parent.display()))?;
    let yaml = serde_yml::to_string(diagnostic).map_err(|error| format!("serialize: {error}"))?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("create temporary recording: {error}"))?;
    temp.write_all(yaml.as_bytes())
        .map_err(|error| format!("write temporary recording: {error}"))?;
    temp.as_file()
        .sync_all()
        .map_err(|error| format!("sync temporary recording: {error}"))?;
    temp.persist(path)
        .map_err(|error| format!("persist {}: {}", path.display(), error.error))?;
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("sync recording directory {}: {error}", parent.display()))?;
    Ok(())
}

#[cfg(test)]
#[path = "recording_tests.rs"]
mod recording_tests;
