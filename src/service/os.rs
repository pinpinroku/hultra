use std::{io, os::unix::fs::MetadataExt, path::Path};

use crate::core::mod_file::ModIdentityService;

pub struct LocalFileSystemService;

impl ModIdentityService for LocalFileSystemService {
    fn fetch_id(&self, path: &Path) -> io::Result<u64> {
        path.metadata().map(|m| m.ino())
    }
}

pub struct MockFileSystemService;
impl ModIdentityService for MockFileSystemService {
    fn fetch_id(&self, _path: &Path) -> io::Result<u64> {
        Ok(12345)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_fs_service() {
        let mock = MockFileSystemService;
        let result = mock.fetch_id(Path::new("."));
        assert!(result.is_ok_and(|value| value == 12345))
    }
}
