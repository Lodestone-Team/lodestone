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
}
