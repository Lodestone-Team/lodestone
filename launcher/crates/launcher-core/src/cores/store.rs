//! The cores the launcher knows: `<data>/servers.json`. The file holds addresses,
//! names, who signed in, pinned certificates and whether the user trusts the core for
//! mods; each device key is in the [`SecretStore`] (the Keychain) under
//! [`key_secret_name`], never in the file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::tls::{CertificatePin, same_fingerprint};
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
    /// The certificate the user trusted, for a core macOS doesn't trust (see
    /// [`super::tls`]). `None` for HTTP and for certificates macOS trusts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate: Option<CertificatePin>,
    /// Whether the user trusts this core for mods and other files that run as code. Off
    /// by default, and for files from before trust existed. Only counts while
    /// [`trusted_as`](Self::trusted_as) still describes the core: see
    /// [`is_trusted`](Self::is_trusted).
    #[serde(default)]
    pub trusted: bool,
    /// The core the trust was given to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trusted_as: Option<TrustKey>,
}

/// What trust in a core is tied to: its id, its address and, for HTTPS cores with a
/// pinned certificate, that certificate.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustKey {
    pub uuid: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate: Option<String>,
}

impl TrustKey {
    fn matches(&self, other: &TrustKey) -> bool {
        self.uuid == other.uuid
            && self.url == other.url
            && match (&self.certificate, &other.certificate) {
                (None, None) => true,
                (Some(a), Some(b)) => same_fingerprint(a, b),
                _ => false,
            }
    }
}

impl SavedCore {
    pub fn signed_in(&self) -> bool {
        self.key_id.is_some()
    }

    /// The core as it is now, to tie trust to.
    pub fn trust_key(&self) -> TrustKey {
        TrustKey {
            uuid: self.uuid.clone(),
            url: self.url.clone(),
            certificate: self.certificate.as_ref().map(|pin| pin.sha256.clone()),
        }
    }

    /// Whether the user trusts this core for mods: they said so, for this core at this
    /// address with this certificate.
    pub fn is_trusted(&self) -> bool {
        self.trusted
            && self
                .trusted_as
                .as_ref()
                .is_some_and(|key| key.matches(&self.trust_key()))
    }

    pub fn set_trusted(&mut self, trusted: bool) {
        self.trusted = trusted;
        self.trusted_as = trusted.then(|| self.trust_key());
    }

    /// Pins a certificate the user accepted. Trust given under another certificate (or
    /// none) ends; returns whether the core was trusted until now.
    pub fn pin_certificate(&mut self, pin: CertificatePin) -> bool {
        let was_trusted = self.is_trusted();
        let same = self
            .certificate
            .as_ref()
            .is_some_and(|old| old.host == pin.host && same_fingerprint(&old.sha256, &pin.sha256));
        self.certificate = Some(pin);
        if !same {
            self.set_trusted(false);
        }
        was_trusted && !same
    }

    /// The core at this address said its id is `uuid`. Another core than the one the
    /// user trusted ends the trust; returns whether it did.
    pub fn check_identity(&mut self, uuid: &str) -> bool {
        if uuid == self.uuid || !self.trusted {
            return false;
        }
        let was_trusted = self.is_trusted();
        self.set_trusted(false);
        was_trusted
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
#[cfg(not(windows))]
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

/// This computer's host name, for the key name. Windows has no `gethostname` in
/// libc; this asks for the same DNS host name `hostname` prints.
#[cfg(windows)]
pub fn host_name() -> String {
    use windows_sys::Win32::System::SystemInformation::{
        ComputerNamePhysicalDnsHostname, GetComputerNameExW,
    };
    let mut buf = [0u16; 256];
    let mut len = buf.len() as u32;
    // SAFETY: the buffer is valid for `len` UTF-16 units; on success the call writes at
    // most that many and sets `len` to the number written, without the NUL.
    let ok =
        unsafe { GetComputerNameExW(ComputerNamePhysicalDnsHostname, buf.as_mut_ptr(), &mut len) };
    if ok == 0 {
        return String::new();
    }
    String::from_utf16_lossy(&buf[..(len as usize).min(buf.len())])
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
                ..SavedCore::default()
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
    fn old_files_load_untrusted() {
        let dir = tempfile::tempdir().unwrap();
        let path = servers_file(dir.path());
        std::fs::write(
            &path,
            r#"{"version":1,"cores":[{"uuid":"c1","url":"https://192.168.1.5:16662","name":"Home","username":null,"key_id":null,"key_expires_at":null}]}"#,
        )
        .unwrap();
        let file = load(&path).unwrap();
        assert!(!file.cores[0].trusted);
        assert!(!file.cores[0].is_trusted());
        assert_eq!(file.cores[0].certificate, None);
        // trust and pins round-trip
        let mut core = file.cores[0].clone();
        core.pin_certificate(CertificatePin::new("192.168.1.5", &"AB".repeat(32)));
        core.set_trusted(true);
        save(
            &path,
            &ServersFile {
                cores: vec![core.clone()],
            },
        )
        .unwrap();
        let loaded = load(&path).unwrap().cores[0].clone();
        assert_eq!(loaded, core);
        assert!(loaded.is_trusted());
    }

    #[test]
    fn trust_ends_with_another_core_or_certificate() {
        let pin = |fp: &str| CertificatePin::new("192.168.1.5", &fp.repeat(32));
        let mut core = SavedCore {
            uuid: "c1".into(),
            url: "https://192.168.1.5:16662".into(),
            ..SavedCore::default()
        };
        assert!(!core.pin_certificate(pin("AB")));
        core.set_trusted(true);
        assert!(core.is_trusted());
        // the same certificate again keeps the trust
        assert!(!core.pin_certificate(pin("ab")));
        assert!(core.is_trusted());
        // a new certificate ends it
        assert!(core.pin_certificate(pin("CD")));
        assert!(!core.is_trusted());
        assert!(!core.trusted);

        core.set_trusted(true);
        assert!(!core.check_identity("c1"));
        assert!(core.is_trusted());
        // another core at this address
        assert!(core.check_identity("c2"));
        assert!(!core.is_trusted());

        // trust given to the core at another address doesn't count here
        core.set_trusted(true);
        core.url = "https://10.0.0.2:16662".into();
        assert!(!core.is_trusted());
        // nor a file edited to say trusted without saying for what
        let mut edited = core.clone();
        edited.trusted_as = None;
        assert!(!edited.is_trusted());
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
