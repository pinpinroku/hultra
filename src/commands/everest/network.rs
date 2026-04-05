//! Everest commands that require network action.
use clap::{Args, Subcommand};

use install::InstallArgs;
use list::ListArgs;

pub mod install;
pub mod list;
pub mod update;

/// Commands that requires network action
#[derive(Debug, Clone, Subcommand)]
pub enum NetworkCommand {
    /// Update Everest to the latest version if available
    Update(NetworkOption),

    /// Install a specific version of Everest
    Install(InstallArgs),

    /// List all available Everest versions from the database
    List(ListArgs),
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
            NetworkCommand::Install(args) => &args.option,
            NetworkCommand::List(args) => &args.option,
        }
    }
}
