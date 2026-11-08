//! The cores the launcher knows: `<data>/servers.json`. The file holds addresses,
//! names and who signed in; each device key is in the [`SecretStore`] (the Keychain)
//! under [`key_secret_name`], never in the file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    Secret, SecretStore,
    storage::{self, Document, Migration, StorageError},
};

/// A core added in the launcher.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedCore {
    /// The core's id from `/info`.
    pub uuid: String,
    /// The address, as [`parse_address`](super::client::parse_address) made it.
    pub url: String,
    /// The core's name when it was last reached.
    pub name: String,
    /// Who signed in, while this device has a key.
    #[serde(default)]
    pub username: Option<String>,
    /// The device key's public id (`lsk_<id>_…`), for "revoke" in the dashboard.
    #[serde(default)]
    pub key_id: Option<String>,
    #[serde(default)]
    pub key_expires_at: Option<i64>,
}

impl SavedCore {
    pub fn signed_in(&self) -> bool {
        self.key_id.is_some()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServersFile {
    #[serde(default)]
    pub cores: Vec<SavedCore>,
}

impl Document for ServersFile {
    const VERSION: u32 = 1;
    /// Version 0 is a file without a `"version"` key; its fields carry over.
    const MIGRATIONS: &'static [Migration] = &[Ok];
}

/// `servers.json` in the data directory.
pub fn servers_file(data_root: &Path) -> PathBuf {
    data_root.join("servers.json")
}

pub fn load(path: &Path) -> Result<ServersFile, StorageError> {
    storage::load(path).map(|loaded| loaded.document)
}

pub fn save(path: &Path, file: &ServersFile) -> Result<(), StorageError> {
    storage::save(path, file)
}

/// The name of a core's device key in the [`SecretStore`].
pub fn key_secret_name(core_uuid: &str) -> String {
    format!("lodestone-core-key:{core_uuid}")
}

pub async fn read_key(store: &dyn SecretStore, core_uuid: &str) -> anyhow::Result<Option<Secret>> {
    store.read(&key_secret_name(core_uuid)).await
}

pub async fn write_key(
    store: &dyn SecretStore,
    core_uuid: &str,
    key: &Secret,
) -> anyhow::Result<()> {
    store.write(&key_secret_name(core_uuid), key).await
}

pub async fn delete_key(store: &dyn SecretStore, core_uuid: &str) -> anyhow::Result<()> {
    store.delete(&key_secret_name(core_uuid)).await
}

/// The name of a new device key: "Lodestone Launcher on <device>", at most 64
/// characters (the core's limit), without control characters.
pub fn device_key_name(device: &str) -> String {
    let device: String = device
        .trim()
        .trim_end_matches(".local")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    let device = if device.is_empty() {
        "this Mac".to_string()
    } else {
        device
    };
    format!("Lodestone Launcher on {device}")
        .chars()
        .take(64)
        .collect()
}

/// This computer's host name, for the key name.
pub fn host_name() -> String {
    let mut buf = [0u8; 256];
    // SAFETY: the buffer is valid for its length; gethostname writes at most that many
    // bytes and NUL-terminates on success.
    let result = unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) };
    if result != 0 {
        return String::new();
    }
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::*;
    use crate::MemorySecretStore;

    #[test]
    fn the_file_never_holds_a_key() {
        let dir = tempfile::tempdir().unwrap();
        let path = servers_file(dir.path());
        assert_eq!(load(&path).unwrap(), ServersFile::default());
        let file = ServersFile {
            cores: vec![SavedCore {
                uuid: "c1".into(),
                url: "http://localhost:16662".into(),
                name: "Home".into(),
                username: Some("steve".into()),
                key_id: Some("abcdefghijkl".into()),
                key_expires_at: None,
            }],
        };
        save(&path, &file).unwrap();
        assert_eq!(load(&path).unwrap(), file);
        let store = MemorySecretStore::new();
        block_on(write_key(&store, "c1", &Secret::new("lsk_SECRET"))).unwrap();
        assert!(!std::fs::read_to_string(&path).unwrap().contains("SECRET"));
        assert_eq!(store.keys(), ["lodestone-core-key:c1"]);
        assert_eq!(
            block_on(read_key(&store, "c1")).unwrap(),
            Some(Secret::new("lsk_SECRET"))
        );
        block_on(delete_key(&store, "c1")).unwrap();
        assert!(store.keys().is_empty());
    }

    #[test]
    fn key_names_fit_the_core() {
        assert_eq!(
            device_key_name("Peters-MacBook.local"),
            "Lodestone Launcher on Peters-MacBook"
        );
        assert_eq!(device_key_name(""), "Lodestone Launcher on this Mac");
        assert_eq!(device_key_name(&"x".repeat(100)).chars().count(), 64);
        assert_eq!(device_key_name("a\u{7}b"), "Lodestone Launcher on ab");
        assert!(!host_name().contains('\0'));
    }
}
