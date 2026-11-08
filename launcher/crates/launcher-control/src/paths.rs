//! Where the control socket is.
//!
//! The socket is `<data dir>/control.sock`, in the launcher's data directory: on macOS
//! `~/Library/Application Support/Lodestone Launcher/`, or `LODESTONE_LAUNCHER_DATA_DIR`
//! when it is set (as the launcher reads it, see `launcher_core::paths::DataDir`).
//!
//! A Unix socket's path is limited to about 100 bytes. When the data directory is
//! deeper than that, the socket goes to `/tmp/lodestone-launcher-<uid>/<hash>.sock`
//! instead, in a folder only this user may use; the hash is of the data directory, so
//! the launcher and the CLI find the same one.

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// The folder name under the platform's data directory, as the launcher's.
pub const APP_DIR_NAME: &str = "Lodestone Launcher";

/// The socket's file name in the data directory.
pub const SOCKET_FILE: &str = "control.sock";

/// The variable that replaces the data directory, as for the launcher.
pub const DATA_DIR_ENV: &str = "LODESTONE_LAUNCHER_DATA_DIR";

/// The variable that names the socket directly, for the CLI.
pub const SOCKET_ENV: &str = "LODESTONE_LAUNCHER_SOCKET";

/// The longest socket path used as is. `sun_path` holds 104 bytes on macOS and 108 on
/// Linux, with the terminating zero.
pub const MAX_SOCKET_PATH: usize = 100;

/// The launcher's data directory for this user, as the launcher finds it.
pub fn data_dir() -> Option<PathBuf> {
    data_dir_with(std::env::var_os(DATA_DIR_ENV), dirs::data_dir())
}

fn data_dir_with(env_override: Option<OsString>, platform_dir: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(dir) = env_override.filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    platform_dir.map(|dir| dir.join(APP_DIR_NAME))
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
pub fn fallback_dir() -> PathBuf {
    #[cfg(unix)]
    let user = crate::current_uid().to_string();
    #[cfg(not(unix))]
    let user = "user".to_string();
    PathBuf::from("/tmp").join(format!("lodestone-launcher-{user}"))
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
        let dir = data_dir_with(None, Some("/Users/me/Library/Application Support".into()));
        assert_eq!(
            dir.unwrap(),
            Path::new("/Users/me/Library/Application Support/Lodestone Launcher")
        );
        let dir = data_dir_with(Some("/tmp/dev".into()), Some("/ignored".into()));
        assert_eq!(dir.unwrap(), Path::new("/tmp/dev"));
        let dir = data_dir_with(Some("".into()), Some("/platform".into()));
        assert_eq!(dir.unwrap(), Path::new("/platform/Lodestone Launcher"));
        assert_eq!(data_dir_with(None, None), None);
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
        assert!(path.as_os_str().len() <= MAX_SOCKET_PATH, "{}", path.display());
        assert!(path.starts_with(fallback_dir()));
        // the same data dir gives the same socket, another dir another one
        assert_eq!(path, socket_path(&deep));
        assert_ne!(path, socket_path(&deep.join("other")));
    }
}
