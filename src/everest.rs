pub mod client;
mod installer;
pub mod version;

use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, File},
    io,
    path::Path,
};

use serde::{Deserialize, Serialize};
use tracing::{info, instrument};
use zip::ZipArchive;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EverestBuild {
    date: String,
    /// Four digits number of version. This value does not follows semantic versiong.
    pub version: u32,
    /// Commit hash.
    commit: String,

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
    fn formatted_date(&self) -> String {
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

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Extracts ZIP archive to the specified directory.
#[instrument]
fn extract_zip_archive(temp_zip: &Path, dest_dir: &Path) -> Result<(), ExtractError> {
    info!("extracting ZIP archive");
    let file = File::open(temp_zip)?;
    let mut archive = ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;

        let raw_path = file.mangled_name();
        let mut components = raw_path.components();
        components.next();

        let relative_path = components.as_path();

        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let outpath = dest_dir.join(relative_path);

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent()
                && !p.exists()
            {
                fs::create_dir_all(p)?;
            }
            let mut outfile = File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }
    Ok(())
}

/// Prints the `n` most recent Everest build vesrions.
pub fn print_builds(builds: Vec<EverestBuild>, n: usize) {
    let groups = get_latest_builds(builds, n);

    println!(
        "{:<10} {:<8} {:<10} {:<20} DETAILS",
        "BRANCH", "VERSION", "COMMIT", "DATE"
    );
    println!("{}", "-".repeat(80));

    for (branch_name, builds) in groups {
        for (i, build) in builds.into_iter().enumerate() {
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

            let date = build.formatted_date();

            println!(
                "{:<10} {:<8} {:<10} {:<20} {}",
                branch_ptr, build.version, short_sha, date, details
            );
        }
    }
}

/// Returns the `n` most recent Everest builds .
fn get_latest_builds(
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;

    #[test]
    fn test_extract_zip_archive_strips_root() -> anyhow::Result<()> {
        let tmp_dir = tempdir()?;
        let zip_path = tmp_dir.path().join("test.zip");
        let dest_dir = tmp_dir.path().join("dest");
        fs::create_dir(&dest_dir)?;

        {
            let file = File::create(&zip_path)?;
            let mut zip = zip::ZipWriter::new(file);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

            // main/
            // ├── root_file.txt
            // └── subdir/
            //     └── inner_file.txt

            zip.add_directory("main/", options)?;

            zip.start_file("main/root_file.txt", options)?;
            zip.write_all(b"root content")?;

            zip.add_directory("main/subdir/", options)?;

            zip.start_file("main/subdir/inner_file.txt", options)?;
            zip.write_all(b"inner content")?;

            zip.finish()?;
        }

        extract_zip_archive(&zip_path, &dest_dir).expect("Extraction failed");

        let extracted_root_file = dest_dir.join("root_file.txt");
        assert!(
            extracted_root_file.exists(),
            "root_file.txt should exist in dest root"
        );
        assert_eq!(fs::read_to_string(extracted_root_file)?, "root content");

        let extracted_inner_file = dest_dir.join("subdir/inner_file.txt");
        assert!(
            extracted_inner_file.exists(),
            "subdir/inner_file.txt should exist and keep its structure"
        );
        assert_eq!(fs::read_to_string(extracted_inner_file)?, "inner content");

        assert!(
            !dest_dir.join("main").exists(),
            "The 'main' directory should not exist in dest"
        );

        Ok(())
    }
}
