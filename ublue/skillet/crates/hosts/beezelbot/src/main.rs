use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
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
struct Args {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Apply configuration
    Apply {
        /// Optional: Output recorded actions to this file path
        #[arg(long)]
        record: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(if args.verbose {
            Level::DEBUG
        } else {
            Level::INFO
        })
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    match args.command {
        Commands::Apply { record } => handle_apply(record),
    }
}

fn handle_apply(record_path: Option<PathBuf>) -> Result<()> {
    info!("Starting Skillet configuration for beezelbot...");

    let system = LinuxSystemResource::new();
    let files = LocalFileResource::new();

    if let Some(path) = record_path {
        let recorder_system = Recorder::new(system);
        let recorder_files = Recorder::with_ops(files, recorder_system.shared_ops());

        apply(&recorder_system, &recorder_files).map_err(|e| anyhow!(e))?;

        let ops = recorder_system.get_ops();
        let yaml = serde_yaml::to_string(&ops)?;
        fs::write(&path, yaml).context("Failed to write recording")?;
        info!("Recording saved to {}", path.display());
    } else {
        apply(&system, &files).map_err(|e| anyhow!(e))?;
    }

    info!("Configuration applied successfully.");
    Ok(())
}
