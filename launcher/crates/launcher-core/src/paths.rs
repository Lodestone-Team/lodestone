//! The launcher's data directory.
//!
//! On macOS it is `~/Library/Application Support/Lodestone Launcher/`, unless the
//! person moved it to another folder (Settings › Storage, [`crate::data_move`]) or
//! the `LODESTONE_LAUNCHER_DATA_DIR` environment variable replaces it, for
//! development. `launcher_control::paths` finds it, for the CLI too.
//! Nothing in it is secret: tokens live in the [`SecretStore`](crate::SecretStore).

use std::path::{Path, PathBuf};

pub use launcher_control::paths::{
    APP_DIR_NAME, DataDirLocation, DataDirSource, LOCATION_FILE, locate as locate_data_dir,
};

/// The paths of the launcher's files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataDir {
    root: PathBuf,
}

impl DataDir {
    /// The data directory for this user, or `None` if the home directory is unknown.
    pub fn locate() -> Option<Self> {
        locate_data_dir().map(|location| Self::at(location.dir))
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

    /// Downloaded manifests, kept so the launcher works offline.
    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// One folder per instance, each with `instance.json` and the game's `minecraft/`.
    pub fn instances_dir(&self) -> PathBuf {
        self.root.join("instances")
    }

    /// `<id>/<id>.json` and `<id>/<id>.jar` for each installed version.
    pub fn versions_dir(&self) -> PathBuf {
        self.root.join("versions")
    }

    /// Library jars, in Maven layout. Shared by every instance.
    pub fn libraries_dir(&self) -> PathBuf {
        self.root.join("libraries")
    }

    /// Asset indexes, objects, legacy copies and logging configs.
    pub fn assets_dir(&self) -> PathBuf {
        self.root.join("assets")
    }

    /// Java runtimes from Mojang, as `<component>/<platform>/`.
    pub fn runtimes_dir(&self) -> PathBuf {
        self.root.join("runtimes")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_in_the_root() {
        let dir = DataDir::at("/Users/me/Library/Application Support/Lodestone Launcher");
        assert_eq!(
            dir.accounts_file(),
            Path::new("/Users/me/Library/Application Support/Lodestone Launcher/accounts.json")
        );
        assert_eq!(
            dir.settings_file(),
            Path::new("/Users/me/Library/Application Support/Lodestone Launcher/settings.json")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_data_dir_is_application_support() {
        let dir = dirs::data_dir().unwrap();
        assert!(dir.ends_with("Library/Application Support"), "{dir:?}");
    }
}
