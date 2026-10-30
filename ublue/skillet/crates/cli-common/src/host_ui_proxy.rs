//! Guest dispatch for the shared Caddy host-loopback upstream transport.
use crate::CliCommonError;

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
    Serve {
        #[arg(long)]
        name: String,
        #[arg(long)]
        port: u16,
    },
}

pub fn dispatch(command: Commands) -> Result<(), CliCommonError> {
    let Commands::Serve { name, port } = command;
    skillet_caddy::host_proxy::serve(&name, port)?;
    Ok(())
}
