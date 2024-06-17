use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

use color_eyre::eyre::{eyre, Error};

pub struct GitClient {
    cwd: PathBuf,
}

impl GitClient {
    pub fn cwd(&self) -> PathBuf {
        self.cwd.clone()
    }

    pub async fn clone(
        url: impl AsRef<str>,
        path: impl AsRef<Path>,
        name: impl AsRef<str>,
    ) -> Result<Self, Error> {
        // get output from stderr
        let output = tokio::process::Command::new("git")
            .arg("clone")
            .arg(url.as_ref())
            .arg(name.as_ref())
            .current_dir(path.as_ref())
            .output()
            .await
            .map_err(|e| eyre!("Failed to get output {}", e))?;
        if !output.status.success() {
            return Err(eyre!(
                "git clone failed : {}",
                String::from_utf8(output.stderr)?
            ));
        }
        let output = String::from_utf8(output.stderr)?;
        // dbg!(&output);
        // extract the path from the output
        // ex. Cloning into 'PATH'...;
        let path = output
            .split('\'')
            .nth(1)
            .ok_or_else(|| eyre!("Failed to parse git clone output"))?;

        Ok(Self {
            cwd: PathBuf::from(path),
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
