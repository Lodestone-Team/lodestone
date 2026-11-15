//! Where the launcher's data directory and the control socket are.
//!
//! The data directory is the folder the person moved it to (Settings › Storage),
//! which the default directory's `data-location.json` names, or else the default
//! directory itself: on macOS `~/Library/Application Support/Lodestone Launcher/`,
//! on Windows `%APPDATA%\Lodestone Launcher\`. `LODESTONE_LAUNCHER_DATA_DIR`
//! replaces the default directory (for development and tests), so a data folder
//! moved from it is followed too.
//!
//! The launcher (`launcher_core::paths::DataDir`), the CLI and the uninstaller all
//! find it this way, so a moved folder is found by every one of them.
//!
//! The socket is `<data dir>/control.sock`. A Unix socket's path is limited to about
//! 100 bytes, on Windows too. When the data directory is deeper than that, the socket
//! goes to `<fallback>/<hash>.sock` instead, in a folder only this user may use:
//! `/tmp/lodestone-launcher-<uid>` on Unix, the user's own temporary folder on
//! Windows. The hash is of the data directory, so the launcher and the CLI find the
//! same one.

use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// The folder name under the platform's data directory, as the launcher's.
pub const APP_DIR_NAME: &str = "Lodestone Launcher";

/// The socket's file name in the data directory.
pub const SOCKET_FILE: &str = "control.sock";

/// The variable that replaces the data directory, as for the launcher.
pub const DATA_DIR_ENV: &str = "LODESTONE_LAUNCHER_DATA_DIR";

/// The variable that names the socket directly, for the CLI.
pub const SOCKET_ENV: &str = "LODESTONE_LAUNCHER_SOCKET";

/// The longest socket path used as is. `sun_path` holds 104 bytes on macOS and 108 on
/// Linux and Windows, with the terminating zero.
pub const MAX_SOCKET_PATH: usize = 100;

/// The file in the default data directory that names the folder the data was moved
/// to. After a move it is all the default directory keeps.
pub const LOCATION_FILE: &str = "data-location.json";

/// Why the data directory is where it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataDirSource {
    /// The person moved it; [`LOCATION_FILE`] names it.
    Moved,
    /// The default directory.
    Default,
}

/// Where the data directory is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataDirLocation {
    /// The data directory.
    pub dir: PathBuf,
    pub source: DataDirSource,
    /// The default directory, which holds [`LOCATION_FILE`]: the platform's, or
    /// `LODESTONE_LAUNCHER_DATA_DIR`.
    pub default: PathBuf,
}

/// [`LOCATION_FILE`]'s contents.
#[derive(Debug, Serialize, Deserialize)]
struct LocationFile {
    version: u32,
    path: PathBuf,
}

/// The launcher's data directory for this user, as the launcher finds it.
pub fn data_dir() -> Option<PathBuf> {
    locate().map(|location| location.dir)
}

/// Where the launcher's data directory is for this user, and why.
pub fn locate() -> Option<DataDirLocation> {
    locate_with(std::env::var_os(DATA_DIR_ENV), dirs::data_dir())
}

fn locate_with(
    env_override: Option<OsString>,
    platform_dir: Option<PathBuf>,
) -> Option<DataDirLocation> {
    let default = match env_override.filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => platform_dir?.join(APP_DIR_NAME),
    };
    Some(match read_location(&default) {
        Some(dir) => DataDirLocation {
            dir,
            source: DataDirSource::Moved,
            default,
        },
        None => DataDirLocation {
            dir: default.clone(),
            source: DataDirSource::Default,
            default,
        },
    })
}

/// The folder `default`'s [`LOCATION_FILE`] names, if it names one. A path that
/// isn't absolute, or a file that can't be read, is ignored.
pub fn read_location(default: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(default.join(LOCATION_FILE)).ok()?;
    let file: LocationFile = serde_json::from_str(&text).ok()?;
    file.path.is_absolute().then_some(file.path)
}

/// Makes `default`'s [`LOCATION_FILE`] name `dir`, or removes it (`None`: the data is
/// in `default` again). Written to a temporary file first and renamed into place, so
/// a reader sees the old file or the new one.
pub fn write_location(default: &Path, dir: Option<&Path>) -> io::Result<()> {
    let file = default.join(LOCATION_FILE);
    let Some(dir) = dir else {
        return match std::fs::remove_file(&file) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        };
    };
    std::fs::create_dir_all(default)?;
    let text = serde_json::to_string_pretty(&LocationFile {
        version: 1,
        path: dir.to_path_buf(),
    })
    .map_err(io::Error::other)?;
    let partial = default.join(format!("{LOCATION_FILE}.part"));
    std::fs::write(&partial, text)?;
    std::fs::rename(&partial, &file)
}

/// The socket of the launcher whose data directory is `data_dir`.
pub fn socket_path(data_dir: &Path) -> PathBuf {
    let path = data_dir.join(SOCKET_FILE);
    if path.as_os_str().len() <= MAX_SOCKET_PATH {
        return path;
    }
    fallback_dir().join(format!("{:016x}.sock", fnv1a(data_dir)))
}

/// The folder for sockets whose data directory is too deep: `/tmp/lodestone-launcher-<uid>`.
#[cfg(unix)]
pub fn fallback_dir() -> PathBuf {
    PathBuf::from("/tmp").join(format!("lodestone-launcher-{}", crate::current_uid()))
}

/// The folder for sockets whose data directory is too deep: `lodestone-launcher` in the
/// user's temporary folder (`%LOCALAPPDATA%\Temp`), which only they may use.
#[cfg(windows)]
pub fn fallback_dir() -> PathBuf {
    std::env::temp_dir().join("lodestone-launcher")
}

/// The socket the CLI uses: `LODESTONE_LAUNCHER_SOCKET`, or the default data directory's.
pub fn default_socket() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(SOCKET_ENV).filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(path));
    }
    data_dir().map(|dir| socket_path(&dir))
}

/// FNV-1a over the path's bytes: stable across runs and builds, which `std`'s hasher
/// isn't.
fn fnv1a(path: &Path) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.as_os_str().as_encoded_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_data_dir_like_the_launcher() {
        let platform = tempfile::tempdir().unwrap();
        let location = locate_with(None, Some(platform.path().into())).unwrap();
        assert_eq!(location.dir, platform.path().join("Lodestone Launcher"));
        assert_eq!(location.source, DataDirSource::Default);
        let dev = tempfile::tempdir().unwrap();
        let location = locate_with(Some(dev.path().into()), Some("/ignored".into())).unwrap();
        assert_eq!(location.dir, dev.path());
        assert_eq!(location.default, dev.path());
        let location = locate_with(Some("".into()), Some(platform.path().into())).unwrap();
        assert_eq!(location.dir, platform.path().join("Lodestone Launcher"));
        assert_eq!(locate_with(None, None), None);
    }

    #[test]
    fn follows_a_moved_data_dir() {
        let platform = tempfile::tempdir().unwrap();
        let moved = tempfile::tempdir().unwrap();
        let default = platform.path().join("Lodestone Launcher");
        write_location(&default, Some(moved.path())).unwrap();
        let location = locate_with(None, Some(platform.path().into())).unwrap();
        assert_eq!(location.dir, moved.path());
        assert_eq!(location.source, DataDirSource::Moved);
        assert_eq!(location.default, default);
        // The variable replaces the default directory, so a folder moved from it is
        // followed too.
        let dev = tempfile::tempdir().unwrap();
        let location = locate_with(Some(dev.path().into()), Some(platform.path().into())).unwrap();
        assert_eq!(location.dir, dev.path());
        write_location(dev.path(), Some(moved.path())).unwrap();
        let location = locate_with(Some(dev.path().into()), Some(platform.path().into())).unwrap();
        assert_eq!(location.dir, moved.path());
        assert_eq!(location.default, dev.path());
        // Moved back; removing a missing file is fine.
        write_location(&default, None).unwrap();
        write_location(&default, None).unwrap();
        let location = locate_with(None, Some(platform.path().into())).unwrap();
        assert_eq!(location.dir, default);
        assert_eq!(location.source, DataDirSource::Default);
    }

    #[test]
    fn ignores_a_bad_location_file() {
        let platform = tempfile::tempdir().unwrap();
        let default = platform.path().join("Lodestone Launcher");
        std::fs::create_dir_all(&default).unwrap();
        for text in ["not json", r#"{"version":1,"path":"relative/dir"}"#, "{}"] {
            std::fs::write(default.join(LOCATION_FILE), text).unwrap();
            let location = locate_with(None, Some(platform.path().into())).unwrap();
            assert_eq!(location.dir, default, "{text}");
        }
    }

    #[test]
    fn short_paths_stay_in_the_data_dir() {
        let dir = Path::new("/Users/me/Library/Application Support/Lodestone Launcher");
        assert_eq!(socket_path(dir), dir.join("control.sock"));
    }

    #[test]
    fn deep_paths_move_to_a_private_folder() {
        let deep = PathBuf::from("/private/tmp").join("x".repeat(120));
        let path = socket_path(&deep);
        assert!(
            path.as_os_str().len() <= MAX_SOCKET_PATH,
            "{}",
            path.display()
        );
        assert!(path.starts_with(fallback_dir()));
        // the same data dir gives the same socket, another dir another one
        assert_eq!(path, socket_path(&deep));
        assert_ne!(path, socket_path(&deep.join("other")));
    }
}
