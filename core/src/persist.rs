//! Crash-safe writes for small state files (settings, watch lists, secrets).

use std::io::Write;
use std::path::{Path, PathBuf};

use color_eyre::eyre::{eyre, Context};

use crate::error::Error;

/// Replaces the file at `path` with `contents` so that readers only ever see the old
/// or the new contents, never a partial file.
///
/// The data is written to a temporary file in the same directory, flushed to disk and
/// then renamed over `path`. On Unix, `mode` sets the permission bits of the new file
/// (for example `0o600` for secrets); otherwise the temporary file's default is used
/// (owner read/write only on Unix).
pub async fn write_atomic(
    path: impl AsRef<Path>,
    contents: impl Into<Vec<u8>>,
    mode: Option<u32>,
) -> Result<(), Error> {
    let path = path.as_ref().to_path_buf();
    let contents = contents.into();
    tokio::task::spawn_blocking(move || write_atomic_blocking(&path, &contents, mode))
        .await
        .context("Atomic write task panicked")?
}

/// Blocking version of [`write_atomic`].
pub fn write_atomic_blocking(path: &Path, contents: &[u8], mode: Option<u32>) -> Result<(), Error> {
    let dir: PathBuf = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let mut tmp = tempfile::NamedTempFile::new_in(&dir).context(format!(
        "Failed to create a temporary file in {}",
        dir.display()
    ))?;
    tmp.write_all(contents)
        .context(format!("Failed to write {}", path.display()))?;
    tmp.as_file()
        .sync_all()
        .context(format!("Failed to flush {}", path.display()))?;
    #[cfg(unix)]
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(mode))
            .context(format!("Failed to set permissions of {}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = mode;
    tmp.persist(path)
        .map_err(|e| eyre!("Failed to replace {}: {}", path.display(), e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn replaces_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        std::fs::write(&path, "old contents that are longer").unwrap();
        write_atomic(&path, "new", None).await.unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        // no temporary files are left behind
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn sets_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.toml");
        write_atomic(&path, "secret", Some(0o600)).await.unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
