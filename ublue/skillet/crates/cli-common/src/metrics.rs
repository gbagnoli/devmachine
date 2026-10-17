//! Guest metric collector dispatch.
use crate::CliCommonError;
use std::path::PathBuf;

#[derive(clap::Subcommand, Debug)]
pub enum MetricsCommands {
    /// Report native Home Assistant backup/export freshness to the local Agent
    Homeassistant {
        #[arg(long, default_value = skillet_datadog::DOGSTATSD_SOCKET)]
        dogstatsd_socket: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
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
        MetricsCommands::Homeassistant {
            dogstatsd_socket,
            dry_run,
        } => {
            let configured =
                std::path::Path::new("/etc/credstore.encrypted/skillet/homeassistant_backup.cred")
                    .try_exists()?;
            let state = std::fs::read(skillet_homeassistant::metrics::STATE)
                .ok()
                .and_then(|bytes| {
                    serde_json::from_slice::<skillet_homeassistant::metrics::State>(&bytes).ok()
                });
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| CliCommonError::Config("clock precedes epoch".into()))?
                .as_secs();
            let packets = skillet_homeassistant::metrics::render(now, configured, state.as_ref());
            if dry_run {
                println!("{}", packets.join("\n"));
            } else {
                let socket = std::os::unix::net::UnixDatagram::unbound()?;
                socket.set_write_timeout(Some(std::time::Duration::from_secs(2)))?;
                socket.connect(dogstatsd_socket)?;
                for packet in packets {
                    socket.send(packet.as_bytes())?;
                }
            }
            Ok(())
        }
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
