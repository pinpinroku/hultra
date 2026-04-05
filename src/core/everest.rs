use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};

pub mod installer;
pub mod network;
pub mod version;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EverestBuild {
    date: String,
    /// Four digits number of version. This value does not follows semantic versiong.
    pub version: u32,
    /// Commit hash.
    pub commit: String,

    /// Build branch.
    #[serde(flatten)]
    pub branch: Branch,

    // NOTE: Currently all entries have this value but still an optional according to specification.
    // NOTE: Also, I don't have any idea what this value means.
    is_native: Option<bool>,

    /// Download link for `main.zip`
    pub main_download: String,
    /// Download size of `main.zip`
    pub main_file_size: u64,
}

impl fmt::Display for EverestBuild {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{}: version {} (released {})",
            self.branch.as_str(),
            self.version,
            self.formatted_date()
        )
    }
}

impl EverestBuild {
    /// Gets first 19 charcters from "2026-03-07T19:48:53.0343351Z", replace 'T' with ' '
    pub fn formatted_date(&self) -> String {
        self.date
            .get(0..19)
            .map(|s| s.replace('T', " "))
            .unwrap_or_else(|| self.date.clone())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Ord, PartialOrd)]
#[serde(rename_all = "lowercase", tag = "branch")]
pub enum Branch {
    Stable,
    Dev { author: String, description: String },
    Beta,
}

impl Branch {
    pub fn as_str(&self) -> &'static str {
        match self {
            Branch::Stable => "stable",
            Branch::Dev { .. } => "dev",
            Branch::Beta => "beta",
        }
    }
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

/// Returns the `n` most recent Everest builds .
pub fn get_latest_builds(
    builds: Vec<EverestBuild>,
    n: usize,
) -> BTreeMap<&'static str, Vec<EverestBuild>> {
    let mut groups: BTreeMap<&'static str, Vec<EverestBuild>> = BTreeMap::new();

    for build in builds {
        groups.entry(build.branch.as_str()).or_default().push(build);
    }

    groups
        .into_iter()
        .map(|(branch, mut branch_builds)| {
            branch_builds.sort_by_key(|b| std::cmp::Reverse(b.version));
            branch_builds.truncate(n);
            (branch, branch_builds)
        })
        .collect()
}

/// Returns installed Everest version.
pub fn get_installed_branch(builds: &[EverestBuild], version: u32) -> Option<&Branch> {
    builds
        .iter()
        .find(|b| b.version == version)
        .map(|b| &b.branch)
}

/// Returns latest build on given branch.
pub fn get_latest_build_on_branch<'a>(
    builds: &'a [EverestBuild],
    branch: &Branch,
) -> Option<&'a EverestBuild> {
    builds
        .iter()
        .filter(|b| &b.branch == branch)
        .max_by_key(|b| b.version)
}
