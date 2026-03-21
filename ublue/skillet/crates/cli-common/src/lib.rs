use anyhow::{Context, Result};
use clap::Parser;
use skillet_core::files::LocalFileResource;
use skillet_core::recorder::Recorder;
use skillet_core::system::LinuxSystemResource;
use skillet_hardening::apply;
use std::fs;
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

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

pub fn run_host(hostname: &str) -> Result<()> {
    let args = HostArgs::parse();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(if args.verbose {
            Level::DEBUG
        } else {
            Level::INFO
        })
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    match args.command {
        HostCommands::Apply { record } => handle_apply(hostname, record),
    }
}

fn handle_apply(hostname: &str, record_path: Option<PathBuf>) -> Result<()> {
    info!("Starting Skillet configuration for {}...", hostname);

    let system = LinuxSystemResource::new();
    let files = LocalFileResource::new();

    if let Some(path) = record_path {
        let recorder_system = Recorder::new(system);
        let recorder_files = Recorder::with_ops(files, recorder_system.shared_ops());

        apply(&recorder_system, &recorder_files)?;

        let ops = recorder_system.get_ops();
        let yaml = serde_yaml::to_string(&ops)?;
        fs::write(&path, yaml).context("Failed to write recording")?;
        info!("Recording saved to {}", path.display());
    } else {
        apply(&system, &files)?;
    }

    info!("Configuration applied successfully.");
    Ok(())
}
