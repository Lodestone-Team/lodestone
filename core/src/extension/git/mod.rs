use std::path::{Path, PathBuf};

use color_eyre::eyre::{eyre, Error};

pub struct GitClient {
    cwd: PathBuf,
}

impl GitClient {
    pub fn cwd(&self) -> PathBuf {
        self.cwd.clone()
    }

    /// Clones `url` into the folder `name` inside `path`.
    pub async fn clone(
        url: impl AsRef<str>,
        path: impl AsRef<Path>,
        name: impl AsRef<str>,
    ) -> Result<Self, Error> {
        let output = tokio::process::Command::new("git")
            .arg("clone")
            // everything after `--` is a positional argument, never an option
            .arg("--")
            .arg(url.as_ref())
            .arg(name.as_ref())
            .current_dir(path.as_ref())
            .output()
            .await
            .map_err(|e| eyre!("Failed to get output {}", e))?;
        if !output.status.success() {
            return Err(eyre!(
                "git clone failed : {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(Self {
            cwd: path.as_ref().join(name.as_ref()),
        })
    }

    pub async fn get_current_commit(&self) -> Result<String, Error> {
        let output = tokio::process::Command::new("git")
            .arg("rev-parse")
            .arg("HEAD")
            .current_dir(&self.cwd)
            .output()
            .await
            .map_err(|e| eyre!("Failed to get output {}", e))?;
        if !output.status.success() {
            return Err(eyre!("git rev-parse HEAD failed"));
        }
        let output = String::from_utf8(output.stdout)?;
        Ok(output.trim().to_string())
    }

    pub async fn fetch(&self) -> Result<(), Error> {
        let status = tokio::process::Command::new("git")
            .arg("fetch")
            .current_dir(&self.cwd)
            .status()
            .await
            .map_err(|e| eyre!("Failed to get status {}", e))?;
        if !status.success() {
            return Err(eyre!("git fetch failed"));
        }
        Ok(())
    }

    pub async fn get_latest_commit(&self) -> Result<String, Error> {
        let output = tokio::process::Command::new("git")
            .arg("rev-parse")
            .arg("origin/main")
            .current_dir(&self.cwd)
            .output()
            .await
            .map_err(|e| eyre!("Failed to get output {}", e))?;
        if !output.status.success() {
            return Err(eyre!("git rev-parse origin/main failed"));
        }
        let output = String::from_utf8(output.stdout)?;
        Ok(output.trim().to_string())
    }
}
