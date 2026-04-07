use clap::Args;

use super::NetworkOption;
use crate::{
    core::everest::{Branch, EverestBuild, EverestBuildExt},
    utils,
};

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
    print_builds(builds, display_n)
}

/// Prints the `n` most recent Everest build vesrions.
fn print_builds(builds: &[EverestBuild], n: usize) {
    let mut groups = builds.get_latest_builds(n);

    println!(
        "{:<10} {:<8} {:<10} {:<20} DETAILS",
        "BRANCH", "VERSION", "COMMIT", "DATE"
    );
    println!("{}", "-".repeat(80));

    for branch_name in ["dev", "beta", "stable"] {
        if let Some(builds_for_branch) = groups.remove(branch_name) {
            for (i, build) in builds_for_branch.into_iter().enumerate() {
                let branch_ptr = if i == 0 { branch_name } else { "" };

                let short_sha = if build.commit.len() > 7 {
                    &build.commit[..7]
                } else {
                    &build.commit
                };

                let details = match &build.branch {
                    Branch::Dev {
                        author,
                        description,
                    } => {
                        format!("[{}] {}", author, description)
                    }
                    _ => "-".to_string(),
                };

                let date = utils::format_date(build.date());

                println!(
                    "{:<10} {:<8} {:<10} {:<20} {}",
                    branch_ptr, build.version, short_sha, date, details
                );
            }
        }
    }
}
