//! Everest commands and the sub commands.
use clap::{Args, Subcommand};

#[derive(Debug, Clone, Subcommand)]
pub enum EverestSubCommand {
    /// Print the current installed version and branch information
    Version,

    #[command(flatten)]
    NetworkRequired(NetworkCommand),
}

/// Commands that requires network action
#[derive(Debug, Clone, Subcommand)]
pub enum NetworkCommand {
    /// Update Everest to the latest version if available
    Update(NetworkOption),

    /// Install a specific version of Everest
    Install {
        /// The version of Everest to install (e.g., "6194")
        version: u32,

        #[command(flatten)]
        option: NetworkOption,
    },

    /// List all available Everest versions from the database
    List {
        /// Prints all versions
        #[arg(short, long)]
        all: bool,

        /// Prints latest versions up to specified number
        #[arg(short, long, default_value_t = 3)]
        limit: usize,

        #[command(flatten)]
        option: NetworkOption,
    },
}

#[derive(Debug, Clone, Args)]
pub struct NetworkOption {
    /// Enables GitHub mirror for database retrieval.
    #[arg(short = 'm', long)]
    pub use_api_mirror: bool,
}

impl NetworkCommand {
    pub fn network_option(&self) -> &NetworkOption {
        match self {
            NetworkCommand::Update(opt) => opt,
            NetworkCommand::Install { option, .. } => option,
            NetworkCommand::List { option, .. } => option,
        }
    }
}
