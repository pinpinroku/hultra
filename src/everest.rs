pub mod client;

use std::fmt;

use console::style;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EverestBuild {
    date: String,
    /// Four digits number of version. This value does not follows semantic versiong.
    version: u32,
    /// Commit hash.
    commit: String,

    /// Build branch.
    #[serde(flatten)]
    pub branch: Branch,

    // NOTE: Currently all entries have this value but still an optional according to specification.
    // NOTE: Also, I don't have any idea what this value means.
    is_native: Option<bool>,

    /// Download link for `main.zip`
    main_download: String,
    /// Download size of `main.zip`
    main_file_size: u64,
}

impl EverestBuild {
    /// Gets first 19 charcters from "2026-03-07T19:48:53.0343351Z", replace 'T' with ' '
    fn formatted_date(&self) -> String {
        self.date
            .get(0..19)
            .map(|s| s.replace('T', " "))
            .unwrap_or_else(|| self.date.clone())
    }
}

impl fmt::Display for EverestBuild {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let line = format!(
            "{} ({}) at {}",
            self.version,
            self.branch,
            self.formatted_date()
        );
        match &self.branch {
            Branch::Dev {
                author,
                description,
            } => {
                writeln!(
                    f,
                    "{} at {} by {}",
                    style(&self.version).bold(),
                    style(self.formatted_date()).dim(),
                    style(author).dim()
                )?;
                writeln!(f, "  {}", style(description).bright())
            }
            Branch::Stable => {
                writeln!(f, "{}", style(line).on_green().black())
            }
            Branch::Beta => {
                writeln!(f, "{}", style(line).on_yellow().black())
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase", tag = "branch")]
pub enum Branch {
    Stable,
    Dev { author: String, description: String },
    Beta,
}

impl fmt::Display for Branch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Branch::Stable => write!(f, "stable"),
            Branch::Dev { .. } => write!(f, "dev"),
            Branch::Beta => write!(f, "beta"),
        }
    }
}

/// Lists currently avaiable Everest builds.
pub fn list_available_builds(builds: Vec<EverestBuild>) {
    for build in builds {
        println!("{}", build)
    }
}

/// Gets latest Everest build for given branch.
///
/// ## Example
/// ```ignore
/// let builds = vec![EverestBuild::default()];
/// let latest = get_latest_build(builds, Branch::Stable);
///
/// if let Some(build) = latest {
///     println!("Latest version on {}: {}", branch, build.version);
///     println!("{}", build);
/// } else {
///     println!("No builds found for branch: {}", branch);
/// }
/// ```
pub async fn get_latest_build(builds: Vec<EverestBuild>, branch: Branch) -> Option<EverestBuild> {
    builds
        .into_iter()
        .filter(|build| {
            matches!(
                (&build.branch, &branch),
                (Branch::Stable, Branch::Stable)
                    | (Branch::Beta, Branch::Beta)
                    | (Branch::Dev { .. }, Branch::Dev { .. })
            )
        })
        .max_by_key(|build| build.version)
}
