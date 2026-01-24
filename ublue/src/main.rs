use anyhow::Result;
use ublue::cli::{Cli, Commands, GsettingsCommands};
use ublue::commands;
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Gsettings(args)) => {
            let message = match args.command {
                GsettingsCommands::List => commands::gsettings::list(args.format)?,
                GsettingsCommands::Show => commands::gsettings::show(args.format)?,
                GsettingsCommands::Set => commands::gsettings::set(args.format)?,
            };
            println!("{}", message);
        }
        None => {
            use clap::CommandFactory;
            Cli::command().print_help()?;
            println!();
        }
    }

    Ok(())
}
