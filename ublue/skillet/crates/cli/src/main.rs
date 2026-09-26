use anyhow::{anyhow, Context, Result};
use clap::Parser;
use skillet_cli_common::hosts::ApplyPhase;
use std::path::PathBuf;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Apply host configuration
    Apply {
        #[arg(long, value_enum, default_value_t = ApplyPhase::Full)]
        phase: ApplyPhase,
        #[arg(long)]
        host: Option<String>,
        #[arg(long)]
        host_file: Option<PathBuf>,
        /// Optional diagnostic recording of resource operations
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
    tracing::subscriber::set_global_default(subscriber)
        .context("setting default subscriber failed")?;

    match args.command {
        Commands::Apply {
            phase,
            host,
            host_file,
            record,
        } => {
            let mut hostname = host.unwrap_or_else(|| "(Agent Mode)".to_string());
            if let Some(path) = host_file {
                hostname = std::fs::read_to_string(path)
                    .context("Failed to read host file")?
                    .trim()
                    .to_string();
            }
            skillet_cli_common::handle_apply(&hostname, record, |system, files| {
                skillet_cli_common::hosts::apply_host_phase(&hostname, phase, system, files)
                    .map_err(|error| error.to_string())
            })
            .map_err(|error| anyhow!("Failed to apply configuration: {error}"))?;
        }
    }
    Ok(())
}
