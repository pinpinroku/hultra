use std::{
    fs::{self, File},
    io,
    path::Path,
};

use tracing::{info, instrument};
use zip::ZipArchive;

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Extracts ZIP archive to the specified directory.
#[instrument]
pub fn extract_zip_archive(temp_zip: &Path, dest_dir: &Path) -> Result<(), ExtractError> {
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
