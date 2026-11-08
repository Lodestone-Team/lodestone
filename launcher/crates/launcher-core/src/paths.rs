//! The launcher's data directory.
//!
//! On macOS it is `~/Library/Application Support/Lodestone Launcher/`. The
//! `LODESTONE_LAUNCHER_DATA_DIR` environment variable replaces it, for development.
//! Nothing in it is secret: tokens live in the [`SecretStore`](crate::SecretStore).

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// The folder name under the platform's data directory.
pub const APP_DIR_NAME: &str = "Lodestone Launcher";

/// The paths of the launcher's files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    /// The data directory for this user, or `None` if the home directory is unknown.
    pub fn locate() -> Option<Self> {
        Self::locate_with(
            std::env::var_os("LODESTONE_LAUNCHER_DATA_DIR"),
            dirs::data_dir(),
        )
    }

    fn locate_with(env_override: Option<OsString>, platform_dir: Option<PathBuf>) -> Option<Self> {
        if let Some(dir) = env_override.filter(|dir| !dir.is_empty()) {
            return Some(Self::at(PathBuf::from(dir)));
        }
        platform_dir.map(|dir| Self::at(dir.join(APP_DIR_NAME)))
    }

    /// A data directory at `root`, for tests and the offscreen screenshots.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The account list and the active account. No tokens.
    pub fn accounts_file(&self) -> PathBuf {
        self.root.join("accounts.json")
    }

    /// The launcher's settings.
    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    /// Rendered skin heads for the account box, by texture hash.
    pub fn avatar_cache_dir(&self) -> PathBuf {
        self.root.join("cache").join("avatars")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_the_platform_data_dir() {
        let dir = DataDir::locate_with(None, Some("/Users/me/Library/Application Support".into()))
            .unwrap();
        assert_eq!(
            dir.root(),
            Path::new("/Users/me/Library/Application Support/Lodestone Launcher")
        );
        assert_eq!(
            dir.accounts_file(),
            Path::new("/Users/me/Library/Application Support/Lodestone Launcher/accounts.json")
        );
        assert_eq!(
            dir.settings_file(),
            Path::new("/Users/me/Library/Application Support/Lodestone Launcher/settings.json")
        );
    }

    #[test]
    fn the_env_var_overrides_it() {
        let dir = DataDir::locate_with(Some("/tmp/dev".into()), Some("/ignored".into())).unwrap();
        assert_eq!(dir.root(), Path::new("/tmp/dev"));
        let dir = DataDir::locate_with(Some("".into()), Some("/platform".into())).unwrap();
        assert_eq!(dir.root(), Path::new("/platform/Lodestone Launcher"));
        assert_eq!(DataDir::locate_with(None, None), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_data_dir_is_application_support() {
        let dir = dirs::data_dir().unwrap();
        assert!(dir.ends_with("Library/Application Support"), "{dir:?}");
    }
}
