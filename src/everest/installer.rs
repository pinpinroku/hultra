use std::{
    fs,
    io::{self, BufRead, BufReader},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub struct MiniInstaller {
    path: PathBuf,
}

impl MiniInstaller {
    pub fn new(root_dir: &Path) -> Self {
        Self {
            path: root_dir.join(Self::PATH_MINI_INSTALLER),
        }
    }

    const PATH_MINI_INSTALLER: &str = "MiniInstaller-linux";

    pub fn grant_execute_permission(&self) -> io::Result<()> {
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

    pub fn execute(&self) -> io::Result<()> {
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
