use clap::Args;

use super::NetworkOption;
use crate::everest::{self, EverestBuild};

#[derive(Debug, Clone, Args)]
pub struct ListArgs {
    /// Prints all versions
    #[arg(short, long)]
    all: bool,
    /// List all available Everest versions from the database
    #[arg(short, long, default_value_t = 3)]
    limit: u8,
    #[command(flatten)]
    pub option: NetworkOption,
}

pub fn run(args: &ListArgs, builds: &[EverestBuild]) {
    let display_n = if args.all {
        builds.len()
    } else {
        args.limit as usize
    };
    everest::print_builds(builds.to_vec(), display_n)
}
