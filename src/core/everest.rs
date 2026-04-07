use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};

use crate::utils;

pub mod installer;
pub mod network;
pub mod version;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EverestBuild {
    /// ISO 8601 format date string.
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
            utils::format_date(&self.date)
        )
    }
}

impl EverestBuild {
    pub fn date(&self) -> &str {
        &self.date
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Ord, PartialOrd)]
#[serde(rename_all = "lowercase", tag = "branch")]
pub enum Branch {
    Dev { author: String, description: String },
    Beta,
    Stable,
}

impl Branch {
    pub fn as_str(&self) -> &'static str {
        match self {
            Branch::Dev { .. } => "dev",
            Branch::Beta => "beta",
            Branch::Stable => "stable",
        }
    }
}

impl fmt::Display for Branch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Branch::Dev { .. } => write!(f, "dev"),
            Branch::Beta => write!(f, "beta"),
            Branch::Stable => write!(f, "stable"),
        }
    }
}

pub trait EverestBuildExt {
    fn get_latest_builds(&self, n: usize) -> BTreeMap<&'static str, Vec<EverestBuild>>;
    fn get_installed_branch(&self, version: u32) -> Option<&Branch>;
    fn get_latest_build_for_branch<'a>(&'a self, branch: &Branch) -> Option<&'a EverestBuild>;
    fn get_build_for_version(&self, version: u32) -> Option<&EverestBuild>;
}

impl EverestBuildExt for [EverestBuild] {
    /// Returns the `n` most recent Everest builds .
    fn get_latest_builds(&self, n: usize) -> BTreeMap<&'static str, Vec<EverestBuild>> {
        let mut groups: BTreeMap<&'static str, Vec<EverestBuild>> = BTreeMap::new();

        for build in self {
            groups
                .entry(build.branch.as_str())
                .or_default()
                .push(build.clone());
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
    fn get_installed_branch(&self, version: u32) -> Option<&Branch> {
        self.iter()
            .find(|b| b.version == version)
            .map(|b| &b.branch)
    }

    /// Returns latest build on given branch.
    fn get_latest_build_for_branch<'a>(&'a self, branch: &Branch) -> Option<&'a EverestBuild> {
        self.iter()
            .filter(|b| &b.branch == branch)
            .max_by_key(|b| b.version)
    }

    /// Returns a build that matches given version, otherwise returns None.
    fn get_build_for_version(&self, version: u32) -> Option<&EverestBuild> {
        self.iter().find(|b| b.version == version)
    }
}
