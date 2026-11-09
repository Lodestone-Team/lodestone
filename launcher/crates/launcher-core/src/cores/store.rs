//! The cores the launcher knows: `<data>/servers.json`. The file holds addresses,
//! names, who signed in, pinned certificates and whether the user trusts the core for
//! mods; each device key is in the [`SecretStore`] (the Keychain) under
//! [`key_secret_name`], never in the file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::models::{CoreProduct, LaunchedBy};
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
    /// Lodestone Host or a bare core, as `/info` said when it was added or found on
    /// this computer. Left out of the file for a bare core.
    #[serde(default, skip_serializing_if = "CoreProduct::is_core")]
    pub product: CoreProduct,
    /// What started the core, as `/info` said last. Left out of the file while unknown.
    #[serde(default, skip_serializing_if = "LaunchedBy::is_unknown")]
    pub launched_by: LaunchedBy,
    /// Signed in by claiming owner access on this computer (`super::local_claim`): the
    /// key is kept in `local_credentials.json` and only ever sent to the pinned
    /// certificate. Left out of the file otherwise.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub local_claim: bool,
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

    /// The core at this address now says its id is `uuid`: trust given under the old id
    /// ends (as in [`check_identity`](Self::check_identity)) and the entry takes the new
    /// id, so trusting it again lasts. Returns the entry as it was and whether trust
    /// ended, or `None` when the id is the same.
    pub fn adopt_identity(&mut self, uuid: &str) -> Option<(SavedCore, bool)> {
        if uuid.is_empty() || uuid == self.uuid {
            return None;
        }
        let old = self.clone();
        let trust_ended = self.check_identity(uuid);
        self.uuid = uuid.to_string();
        Some((old, trust_ended))
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

/// Whether two entries are for the same core: the same id, or the same address. Only
/// one core answers at an address, so a second entry for it is a stale copy.
fn same_core(a: &SavedCore, b: &SavedCore) -> bool {
    a.uuid == b.uuid || a.url == b.url
}

/// The entry that "Add server" or "Sign in" for the core at `url`, which says its id is
/// `uuid`, updates: the entry with that id, else one at the same address (a signed-in
/// one first). A core makes a new id each time it starts, so after a restart the entry
/// added under the old id is still the one for this address, and signing in from it
/// must not add a second one.
pub fn find_existing(cores: &[SavedCore], uuid: &str, url: &str) -> Option<usize> {
    cores.iter().position(|c| c.uuid == uuid).or_else(|| {
        let at = |signed_in: bool| {
            cores
                .iter()
                .position(|c| c.url == url && c.signed_in() == signed_in)
        };
        at(true).or_else(|| at(false))
    })
}

/// An entry [`dedupe`] took out, and the id of the entry that stays for the same core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Merged {
    pub removed: SavedCore,
    pub into: String,
}

/// Takes out every entry that is for the same core as another (see [`same_core`]),
/// returning them in the order they were taken out. The entry at `keep` stays; for
/// other pairs the signed-in entry stays, else the one added later. `saved` reads an
/// entry's [`SavedCore`], so this works on the page's entries as on the file's.
pub fn dedupe<T>(
    entries: &mut Vec<T>,
    saved: impl Fn(&T) -> &SavedCore,
    mut keep: Option<usize>,
) -> Vec<Merged> {
    let mut merged = Vec::new();
    loop {
        let pair = (0..entries.len())
            .flat_map(|i| (i + 1..entries.len()).map(move |j| (i, j)))
            .find(|&(i, j)| same_core(saved(&entries[i]), saved(&entries[j])));
        let Some((i, j)) = pair else {
            break;
        };
        let (stay, gone) = if keep == Some(i) {
            (i, j)
        } else if keep == Some(j) {
            (j, i)
        } else if saved(&entries[i]).signed_in() && !saved(&entries[j]).signed_in() {
            (i, j)
        } else {
            (j, i)
        };
        let into = saved(&entries[stay]).uuid.clone();
        let removed = saved(&entries.remove(gone)).clone();
        if let Some(k) = keep.as_mut()
            && *k > gone
        {
            *k -= 1;
        }
        merged.push(Merged { removed, into });
    }
    merged
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
        format!("this {}", crate::wording::COMPUTER)
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
    fn a_new_id_ends_trust_and_trusting_again_lasts() {
        let mut core = SavedCore {
            uuid: "old".into(),
            url: "http://127.0.0.1:16662".into(),
            ..SavedCore::default()
        };
        core.set_trusted(true);
        assert!(core.adopt_identity("old").is_none());
        assert!(core.is_trusted());

        let (before, trust_ended) = core.adopt_identity("new").unwrap();
        assert_eq!(before.uuid, "old");
        assert!(trust_ended);
        assert_eq!(core.uuid, "new");
        assert!(!core.is_trusted());

        // trusting the core under its new id survives the next refresh
        core.set_trusted(true);
        assert!(core.adopt_identity("new").is_none());
        assert!(core.is_trusted());
        assert!(!core.check_identity("new"));
        assert!(core.is_trusted());
    }

    #[test]
    fn an_untrusted_entry_takes_the_new_id_quietly() {
        let mut core = SavedCore {
            uuid: "old".into(),
            ..SavedCore::default()
        };
        let (_, trust_ended) = core.adopt_identity("new").unwrap();
        assert!(!trust_ended);
        assert_eq!(core.uuid, "new");
        assert!(core.adopt_identity("").is_none());
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

    fn core(uuid: &str, url: &str, signed_in: bool) -> SavedCore {
        SavedCore {
            uuid: uuid.into(),
            url: url.into(),
            name: "Home".into(),
            username: signed_in.then(|| "steve".into()),
            key_id: signed_in.then(|| format!("key-{uuid}")),
            ..SavedCore::default()
        }
    }

    const HOME: &str = "http://localhost:16662";

    #[test]
    fn signing_in_after_a_core_restart_updates_the_same_entry() {
        // added anonymously; the core restarted since and has a new id
        let cores = vec![
            core("friend", "https://10.0.0.2:16662", true),
            core("before-restart", HOME, false),
        ];
        assert_eq!(find_existing(&cores, "after-restart", HOME), Some(1));
        // the id still wins over the address
        assert_eq!(
            find_existing(&cores, "friend", "https://friend.example:16662"),
            Some(0)
        );
        assert_eq!(
            find_existing(&cores, "after-restart", "http://127.0.0.1:16662"),
            None
        );
        // of two entries at the address, the signed-in one
        let cores = vec![core("a", HOME, false), core("b", HOME, true)];
        assert_eq!(find_existing(&cores, "c", HOME), Some(1));
    }

    #[test]
    fn duplicates_merge_into_the_signed_in_entry() {
        // what signing in used to do: a second, signed-in entry for the same address
        let mut cores = vec![
            core("anonymous", HOME, false),
            core("friend", "https://10.0.0.2:16662", false),
            core("signed-in", HOME, true),
        ];
        let merged = dedupe(&mut cores, |c| c, None);
        assert_eq!(
            merged,
            [Merged {
                removed: core("anonymous", HOME, false),
                into: "signed-in".into(),
            }]
        );
        assert_eq!(
            cores,
            [
                core("friend", "https://10.0.0.2:16662", false),
                core("signed-in", HOME, true)
            ]
        );
        assert!(dedupe(&mut cores, |c| c, None).is_empty());
    }

    #[test]
    fn dedupe_keeps_the_entry_asked_for() {
        // the entry just signed in to stays, even before an older signed-in copy
        let mut cores = vec![
            core("old", HOME, true),
            core("same-id", "http://127.0.0.1:16662", false),
            core("new", "http://127.0.0.1:16662", false),
            core("new", HOME, true),
        ];
        let merged = dedupe(&mut cores, |c| c, Some(3));
        assert_eq!(cores, [core("new", HOME, true)]);
        let removed: Vec<&str> = merged.iter().map(|m| m.removed.uuid.as_str()).collect();
        assert_eq!(removed, ["old", "same-id", "new"]);
        assert!(merged.iter().all(|m| m.into == "new"));

        // without a preference, the later of two anonymous entries stays
        let mut cores = vec![core("a", HOME, false), core("b", HOME, false)];
        dedupe(&mut cores, |c| c, None);
        assert_eq!(cores, [core("b", HOME, false)]);

        // works on any entry that holds a SavedCore
        let mut entries = vec![(1, core("a", HOME, false)), (2, core("b", HOME, true))];
        dedupe(&mut entries, |(_, c)| c, Some(0));
        assert_eq!(entries, [(1, core("a", HOME, false))]);
    }

    #[test]
    fn key_names_fit_the_core() {
        assert_eq!(
            device_key_name("Peters-MacBook.local"),
            "Lodestone Launcher on Peters-MacBook"
        );
        assert_eq!(
            device_key_name(""),
            format!("Lodestone Launcher on this {}", crate::wording::COMPUTER)
        );
        assert_eq!(device_key_name(&"x".repeat(100)).chars().count(), 64);
        assert_eq!(device_key_name("a\u{7}b"), "Lodestone Launcher on ab");
        assert!(!host_name().contains('\0'));
    }
}
