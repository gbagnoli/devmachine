use clap::Parser;
use skillet_core::files::{FileResource, LocalFileResource};
use skillet_core::recorder::Recorder;
use skillet_core::system::{LinuxSystemResource, SystemResource};
use std::fs;
use std::path::PathBuf;
use thiserror::Error;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

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
        /// Optional: Output recorded actions to this file path
        #[arg(long)]
        record: Option<PathBuf>,
    },
}

pub fn run_host<F>(hostname: &str, apply_fn: F) -> Result<(), CliCommonError>
where
    F: Fn(&dyn SystemResource, &dyn FileResource) -> Result<(), String>,
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
        HostCommands::Apply { record } => handle_apply(hostname, record, apply_fn),
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
    info!("Starting Skillet configuration for {}...", hostname);

    let system = LinuxSystemResource::new();
    let files = LocalFileResource::new();

    if let Some(path) = record_path {
        let recorder_system = Recorder::new(system);
        let recorder_files = Recorder::with_ops(files, recorder_system.shared_ops());

        apply_fn(&recorder_system, &recorder_files).map_err(CliCommonError::Config)?;

        let ops = recorder_system.get_ops();
        let yaml = serde_yml::to_string(&ops)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, yaml)?;
        info!("Recording saved to {}", path.display());
    } else {
        apply_fn(&system, &files).map_err(CliCommonError::Config)?;
    }

    info!("Configuration applied successfully.");
    Ok(())
}
