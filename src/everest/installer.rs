use std::{
    fs,
    io::{self, BufRead, BufReader},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::config::AppConfig;

/// Runs MiniInstaller.
pub fn run(config: &AppConfig) -> io::Result<()> {
    let installer = MiniInstaller::new(config.root_dir());
    installer.grant_execute_permission()?;
    installer.execute()
}

/// Installer for Everest.
struct MiniInstaller {
    path: PathBuf,
}

impl MiniInstaller {
    fn new(root_dir: &Path) -> Self {
        Self {
            path: root_dir.join("MiniInstaller-linux"),
        }
    }

    /// Grants execute permission to the installer.
    fn grant_execute_permission(&self) -> io::Result<()> {
        let user_exec_bit = 0o100;

        let metadata = fs::metadata(&self.path)?;
        let mut perms = metadata.permissions();
        let current_mode = perms.mode();

        if (current_mode & user_exec_bit) != 0 {
            return Ok(());
        }

        perms.set_mode(current_mode | user_exec_bit);
        fs::set_permissions(&self.path, perms)
    }

    /// Executes the installer.
    fn execute(&self) -> io::Result<()> {
        let mut child = Command::new(&self.path).stdout(Stdio::piped()).spawn()?;

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                println!("{}", line);
            }
        }

        child.wait()?;

        Ok(())
    }
}
