use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Apply GNOME settings
    Gsettings(GsettingsArgs),
}

#[derive(Args)]
pub struct GsettingsArgs {
    #[command(subcommand)]
    pub command: GsettingsCommands,

    /// Output format
    #[arg(long, value_enum, default_value_t = Format::Human)]
    pub format: Format,
}

#[derive(Subcommand)]
pub enum GsettingsCommands {
    /// List available settings
    List,
    /// Show current values
    Show,
    /// Set optimized values
    Set,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Format {
    Human,
    Json,
    Csv,
    Tsv,
}
