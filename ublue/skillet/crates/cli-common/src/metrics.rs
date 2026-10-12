//! Guest metric collector dispatch.
use crate::CliCommonError;
use std::path::PathBuf;

#[derive(clap::Subcommand, Debug)]
pub enum MetricsCommands {
    /// Poll a NUT UPS read-only and submit metrics to local `DogStatsD`
    Nut {
        #[arg(long)]
        ups: String,
        #[arg(long, default_value = "127.0.0.1:3493")]
        server: std::net::SocketAddr,
        #[arg(long, default_value = skillet_datadog::DOGSTATSD_SOCKET)]
        dogstatsd_socket: PathBuf,
        /// Print datagrams without sending telemetry
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn run_metrics(command: MetricsCommands) -> Result<(), CliCommonError> {
    match command {
        MetricsCommands::Nut {
            ups,
            server,
            dogstatsd_socket,
            dry_run,
        } => {
            skillet_nut::metrics::validate_name(&ups)
                .map_err(|error| CliCommonError::Config(error.to_string()))?;
            let sample = skillet_nut::metrics::collect(server, &ups);
            let packets = skillet_nut::metrics::render(&ups, sample.as_ref().ok())
                .map_err(|error| CliCommonError::Config(error.to_string()))?;
            if dry_run {
                println!("{}", packets.join("\n"));
            } else {
                skillet_nut::metrics::submit(&dogstatsd_socket, &packets)
                    .map_err(|error| CliCommonError::Config(error.to_string()))?;
            }
            sample
                .map(|_| ())
                .map_err(|error| CliCommonError::Config(error.to_string()))
        }
    }
}
