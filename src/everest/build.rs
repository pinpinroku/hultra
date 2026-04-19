use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};

use crate::utils;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EverestBuild {
    /// ISO 8601 format date string.
    date: String,
    /// Four digits number of version. This value does not follows semantic version.
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

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Ord, PartialOrd, Default)]
#[serde(rename_all = "lowercase", tag = "branch")]
pub enum Branch {
    Dev {
        author: String,
        description: String,
    },
    Beta,
    #[default]
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
    fn get_latest_builds(&self, n: u8) -> BTreeMap<&'static str, Vec<EverestBuild>>;
    fn get_installed_branch(&self, version: u32) -> Option<&Branch>;
    fn get_latest_build_for_branch<'a>(&'a self, branch: &Branch) -> Option<&'a EverestBuild>;
    fn get_build_for_version(&self, version: u32) -> Option<&EverestBuild>;
}

impl EverestBuildExt for [EverestBuild] {
    /// Returns the `n` most recent Everest builds .
    fn get_latest_builds(&self, n: u8) -> BTreeMap<&'static str, Vec<EverestBuild>> {
        let mut groups: BTreeMap<&'static str, Vec<EverestBuild>> = BTreeMap::new();

        if n == 0 {
            return BTreeMap::new();
        }

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
                branch_builds.truncate(n.into());
                (branch, branch_builds)
            })
            .filter(|(_, builds)| !builds.is_empty())
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

#[cfg(test)]
mod test {
    use super::*;

    fn setup_builds() -> [EverestBuild; 1] {
        [EverestBuild {
            version: 6412,
            ..Default::default()
        }]
    }

    #[test]
    fn test_get_latest_builds() {
        let builds = setup_builds();
        assert_eq!(builds.get_latest_builds(1).len(), 1);
    }

    #[test]
    fn test_get_latest_builds_boundary() {
        let builds = setup_builds();
        assert_eq!(
            builds
                .get_latest_builds(3)
                .values()
                .map(|v| v.len())
                .sum::<usize>(),
            1
        );
        assert_eq!(builds.get_latest_builds(0).len(), 0);
    }

    #[test]
    fn test_get_build_for_version() {
        let builds = setup_builds();
        assert!(builds.get_build_for_version(6412).is_some());
        assert!(builds.get_build_for_version(9999).is_none());
    }

    #[test]
    fn get_latest_build_for_branch() {
        let builds = setup_builds();
        assert!(
            builds
                .get_latest_build_for_branch(&Branch::Stable)
                .is_some()
        )
    }

    #[test]
    fn test_get_installed_branch() {
        let builds = setup_builds();
        assert!(builds.get_installed_branch(6412).is_some())
    }

    #[test]
    fn test_empty_list() {
        let builds: [EverestBuild; 0] = [];
        assert!(builds.get_build_for_version(6412).is_none());
        assert!(builds.get_latest_builds(1).is_empty());
    }

    #[test]
    fn test_get_latest_order() {
        let builds = [
            EverestBuild {
                version: 100,
                ..Default::default()
            },
            EverestBuild {
                version: 200,
                ..Default::default()
            },
        ];
        let mut result = builds.get_latest_builds(1);
        assert!(
            result
                .pop_first()
                .is_some_and(|(_, b)| b.first().is_some_and(|v| v.version == 200))
        )
    }
}
