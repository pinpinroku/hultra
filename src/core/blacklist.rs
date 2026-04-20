use std::{collections::HashSet, fs, io, path::PathBuf, str::FromStr};

use tracing::instrument;

#[instrument(skip_all)]
pub fn fetch(source: &impl UpdaterBlacklistSource) -> io::Result<UpdaterBlacklist> {
    let content = source.fetch_content()?;
    let blacklist: UpdaterBlacklist = content
        .parse()
        .expect("should be parsed since this is an infallible operation");
    Ok(blacklist)
}

/// Represents `updaterblacklist.txt` which is used to ignore specific mods from updates.
#[derive(Debug, Clone, Default)]
pub struct UpdaterBlacklist {
    /// A list of unique mod filenames.
    filenames: HashSet<String>,
}

impl UpdaterBlacklist {
    pub fn filenames(&self) -> &HashSet<String> {
        &self.filenames
    }
}

impl FromStr for UpdaterBlacklist {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let files = s
            .lines()
            .map(|l| l.trim())
            // NOTE "SomeMod.zip" is an example entry, so we should ignore it
            .filter(|l| !l.is_empty() && !l.starts_with('#') && *l != "SomeMod.zip")
            .map(String::from)
            .collect();
        Ok(UpdaterBlacklist { filenames: files })
    }
}

pub trait UpdaterBlacklistSource {
    fn fetch_content(&self) -> io::Result<String>;
}

#[derive(Debug, Clone)]
pub struct LocalUpdaterBlacklistSource {
    base_directory: PathBuf,
}

impl LocalUpdaterBlacklistSource {
    pub fn new(base_directory: PathBuf) -> Self {
        Self { base_directory }
    }
}

impl UpdaterBlacklistSource for LocalUpdaterBlacklistSource {
    fn fetch_content(&self) -> io::Result<String> {
        let path = self.base_directory.join("updaterblacklist.txt");
        let content = fs::read_to_string(&path).or_else(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                Ok(String::new())
            } else {
                Err(e)
            }
        })?;
        Ok(content)
    }
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    #[test]
    fn test_parse_default_blacklist() {
        let default = r#"
# This is the Updater Blacklist. Lines starting with # are ignored.
# If you put the name of a mod zip in this file, it won't be auto-updated and it won't show update notifications on the title screen.
SomeMod.zip
"#;

        let blacklist: UpdaterBlacklist = default.parse().expect("should be parsed");
        assert!(blacklist.filenames().is_empty())
    }

    #[test]
    fn test_parse_blacklist_with_actual_names() {
        let content = r#"
SpeedrunTool.zip
Another Farewell Map CC-Side.zip
GravityHelper.zip
"#;

        let blacklist: UpdaterBlacklist = content.parse().expect("should be parsed");
        assert_eq!(blacklist.filenames().len(), 3)
    }
}

#[cfg(test)]
mod fetch_tests {
    use super::*;

    /// Mock source for controlled input
    struct MockSource {
        content: Option<String>,
        error: Option<io::ErrorKind>,
    }

    impl UpdaterBlacklistSource for MockSource {
        fn fetch_content(&self) -> io::Result<String> {
            if let Some(kind) = self.error {
                Err(io::Error::new(kind, "mock error"))
            } else {
                Ok(self.content.clone().unwrap_or_default())
            }
        }
    }

    #[test]
    fn test_fetch_empty_content() {
        // Empty content should result in empty blacklist
        let source = MockSource {
            content: Some(String::new()),
            error: None,
        };

        let blacklist = fetch(&source).expect("fetch should succeed");

        assert!(blacklist.filenames().is_empty());
    }

    #[test]
    fn test_fetch_valid_content() {
        // Valid blacklist entries
        let source = MockSource {
            content: Some(
                r#"
SpeedrunTool.zip
Another Farewell Map CC-Side.zip
GravityHelper.zip
"#
                .to_string(),
            ),
            error: None,
        };

        let blacklist = fetch(&source).expect("fetch should succeed");

        assert_eq!(blacklist.filenames().len(), 3);
        assert!(blacklist.filenames().contains("SpeedrunTool.zip"));
        assert!(
            blacklist
                .filenames()
                .contains("Another Farewell Map CC-Side.zip")
        );
        assert!(blacklist.filenames().contains("GravityHelper.zip"));
    }

    #[test]
    fn test_fetch_ignores_comments_and_default_entry() {
        // Should ignore comments and "SomeMod.zip"
        let source = MockSource {
            content: Some(
                r#"
# comment
SomeMod.zip
ValidMod.zip
"#
                .to_string(),
            ),
            error: None,
        };

        let blacklist = fetch(&source).expect("fetch should succeed");

        assert_eq!(blacklist.filenames().len(), 1);
        assert!(blacklist.filenames().contains("ValidMod.zip"));
    }

    #[test]
    fn test_fetch_propagates_error() {
        // Simulate IO error
        let source = MockSource {
            content: None,
            error: Some(io::ErrorKind::Other),
        };

        let result = fetch(&source);

        assert!(result.is_err());
    }
}
