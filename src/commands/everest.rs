//! Everest commands and the sub commands.
use clap::Subcommand;

use crate::commands::everest::network::NetworkCommand;

pub mod network;
pub mod version;

#[derive(Debug, Clone, Subcommand)]
pub enum EverestSubCommand {
    /// Print the current installed version
    Version,

    #[command(flatten)]
    NetworkRequired(NetworkCommand),
}
