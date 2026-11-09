//! `accounts.json`: the signed-in accounts and which one is active.
//!
//! The file holds what the account box shows (the name, the UUID and the skin URL)
//! and nothing secret. A Microsoft account's refresh token is in the
//! [`SecretStore`](crate::SecretStore) under [`Account::secret_key`], and its
//! Minecraft access token only ever lives in memory.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    SecretStore,
    auth::ClientIdentity,
    offline::{self, UsernameError},
    storage::{self, Document, Migration, StorageError},
};

/// How an account signs in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    /// A Microsoft account that owns Minecraft: Java Edition.
    Microsoft,
    /// A local name for single-player and offline-mode servers, in development only.
    Offline,
}

/// An account, as stored in `accounts.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    /// `microsoft:<uuid>` or `offline:<uuid>`, with the UUID in simple form.
    pub id: String,
    pub kind: AccountKind,
    /// The Minecraft profile UUID, or the offline UUID for offline accounts.
    pub uuid: Uuid,
    pub username: String,
    /// The active skin's texture URL, from the Minecraft profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin_url: Option<String>,
    /// Which client issued a Microsoft account's refresh token, and so which one
    /// refreshes it. Files from before the developer option have no `client`; those
    /// accounts were all signed in with Lodestone's app, the default.
    #[serde(default)]
    pub client: ClientIdentity,
}

impl Account {
    /// A Microsoft account for a Minecraft profile.
    pub fn microsoft(uuid: Uuid, username: impl Into<String>, skin_url: Option<String>) -> Self {
        Self {
            id: format!("microsoft:{}", uuid.simple()),
            kind: AccountKind::Microsoft,
            uuid,
            username: username.into(),
            skin_url,
            client: ClientIdentity::Lodestone,
        }
    }

    /// The account, recorded as signed in with `client`.
    pub fn with_client(mut self, client: ClientIdentity) -> Self {
        self.client = client;
        self
    }

    /// Whether the account was signed in as the Minecraft Launcher (the developer
    /// option) rather than with Lodestone's app.
    pub fn uses_launcher_identity(&self) -> bool {
        self.kind == AccountKind::Microsoft && self.client == ClientIdentity::MinecraftLauncher
    }

    /// An offline account, after checking the username.
    pub fn offline(username: &str) -> Result<Self, UsernameError> {
        offline::validate_username(username)?;
        let uuid = offline::offline_uuid(username);
        Ok(Self {
            id: format!("offline:{}", uuid.simple()),
            kind: AccountKind::Offline,
            uuid,
            username: username.to_string(),
            skin_url: None,
            client: ClientIdentity::Lodestone,
        })
    }

    pub fn is_offline(&self) -> bool {
        self.kind == AccountKind::Offline
    }

    /// The [`SecretStore`] key of a Microsoft account's refresh token.
    pub fn secret_key(&self) -> Option<String> {
        (self.kind == AccountKind::Microsoft).then(|| secret_key(&self.id))
    }
}

/// The [`SecretStore`] key for the account with `id`. On macOS this is the
/// Keychain item's "server" field.
pub fn secret_key(account_id: &str) -> String {
    format!("lodestone-launcher:{account_id}")
}

/// The contents of `accounts.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountsFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
    #[serde(default)]
    pub accounts: Vec<Account>,
}

impl Document for AccountsFile {
    const VERSION: u32 = 1;
    /// Version 0 is a file without a `"version"` key; its fields carry over.
    const MIGRATIONS: &'static [Migration] = &[Ok];
}

/// The account list, backed by `accounts.json`.
#[derive(Debug)]
pub struct AccountStore {
    path: PathBuf,
    file: AccountsFile,
    /// False when the file on disk could not be read as this version, so saving
    /// would lose data. The store then works in memory only.
    writable: bool,
}

/// What [`AccountStore::open`] found.
#[derive(Debug)]
pub enum OpenProblem {
    /// The file was written by a newer launcher. It is left alone and changes are
    /// not saved.
    TooNew(StorageError),
    /// The file could not be parsed. It was moved aside to `moved_to`, and the
    /// launcher starts with no accounts.
    Corrupt {
        error: StorageError,
        moved_to: Option<PathBuf>,
    },
    /// The file could not be read (for example, a permissions problem). Changes are
    /// not saved.
    Unreadable(StorageError),
}

impl AccountStore {
    /// Opens the store at `path`, migrating an older file and writing it back.
    pub fn open(path: impl Into<PathBuf>) -> (Self, Option<OpenProblem>) {
        let path = path.into();
        match storage::load::<AccountsFile>(&path) {
            Ok(loaded) => {
                let migrated = loaded.migrated(AccountsFile::VERSION);
                let mut store = Self {
                    path,
                    file: loaded.document,
                    writable: true,
                };
                store.normalize();
                if migrated {
                    // Best effort: the migrated data is in memory either way.
                    let _ = store.save();
                }
                (store, None)
            }
            Err(error) => {
                let (writable, problem) = match error {
                    StorageError::TooNew { .. } => (false, OpenProblem::TooNew(error)),
                    StorageError::Corrupt { .. } => {
                        let moved_to = move_aside(&path);
                        (moved_to.is_some(), OpenProblem::Corrupt { error, moved_to })
                    }
                    StorageError::Io { .. } => (false, OpenProblem::Unreadable(error)),
                };
                let store = Self {
                    path,
                    file: AccountsFile::default(),
                    writable,
                };
                (store, Some(problem))
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn is_writable(&self) -> bool {
        self.writable
    }

    pub fn accounts(&self) -> &[Account] {
        &self.file.accounts
    }

    /// The accounts signed in as the Minecraft Launcher.
    pub fn launcher_identity_accounts(&self) -> Vec<&Account> {
        self.file
            .accounts
            .iter()
            .filter(|account| account.uses_launcher_identity())
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<&Account> {
        self.file.accounts.iter().find(|account| account.id == id)
    }

    pub fn active(&self) -> Option<&Account> {
        self.file.active.as_deref().and_then(|id| self.get(id))
    }

    /// Adds the account, or updates the one with the same id, and makes it active.
    pub fn upsert_and_activate(&mut self, account: Account) -> Result<(), StorageError> {
        let id = account.id.clone();
        match self.file.accounts.iter_mut().find(|a| a.id == account.id) {
            Some(existing) => *existing = account,
            None => self.file.accounts.push(account),
        }
        self.file.active = Some(id);
        self.save()
    }

    /// Updates the stored name and skin of an existing account, for example after a
    /// refresh found that the player renamed. Does nothing for unknown ids.
    pub fn update_profile(
        &mut self,
        id: &str,
        username: &str,
        skin_url: Option<String>,
    ) -> Result<(), StorageError> {
        let Some(account) = self.file.accounts.iter_mut().find(|a| a.id == id) else {
            return Ok(());
        };
        if account.username == username && account.skin_url == skin_url {
            return Ok(());
        }
        account.username = username.to_string();
        account.skin_url = skin_url;
        self.save()
    }

    /// Makes `id` the active account. Returns false if there is no such account.
    pub fn set_active(&mut self, id: &str) -> Result<bool, StorageError> {
        if self.get(id).is_none() {
            return Ok(false);
        }
        self.file.active = Some(id.to_string());
        self.save()?;
        Ok(true)
    }

    /// Removes the account and deletes its refresh token from `secrets`. If it was
    /// active, the first remaining account becomes active.
    ///
    /// The secret is deleted first: if that fails, the account stays listed so the
    /// user can try again, rather than leaving an orphaned Keychain item.
    pub async fn remove(
        &mut self,
        id: &str,
        secrets: &dyn SecretStore,
    ) -> Result<Option<Account>, RemoveError> {
        let Some(account) = self.get(id) else {
            return Ok(None);
        };
        if let Some(key) = account.secret_key() {
            secrets.delete(&key).await.map_err(RemoveError::Secret)?;
        }
        self.forget(id).map_err(RemoveError::Storage)
    }

    /// Removes the account from the list without touching the [`SecretStore`]. Use
    /// it after deleting the account's secret, when the store cannot be borrowed
    /// across the `await` (as in the app).
    pub fn forget(&mut self, id: &str) -> Result<Option<Account>, StorageError> {
        let Some(index) = self.file.accounts.iter().position(|a| a.id == id) else {
            return Ok(None);
        };
        let account = self.file.accounts.remove(index);
        self.normalize();
        self.save()?;
        Ok(Some(account))
    }

    /// Writes the file, unless the store is in memory only.
    pub fn save(&self) -> Result<(), StorageError> {
        if !self.writable {
            return Ok(());
        }
        storage::save(&self.path, &self.file)
    }

    /// Keeps `active` pointing at a listed account.
    fn normalize(&mut self) {
        if self.active().is_none() {
            self.file.active = self.file.accounts.first().map(|a| a.id.clone());
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RemoveError {
    #[error("could not delete the saved sign-in: {0:#}")]
    Secret(anyhow::Error),
    #[error(transparent)]
    Storage(StorageError),
}

/// Renames an unreadable file to `<name>.corrupt-<n>` so a fresh one can be written.
fn move_aside(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    (1..100)
        .map(|n| path.with_file_name(format!("{name}.corrupt-{n}")))
        .find(|candidate| !candidate.exists())
        .and_then(|candidate| {
            crate::fsx::rename(path, &candidate)
                .ok()
                .map(|()| candidate)
        })
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use serde_json::{Value, json};

    use super::*;
    use crate::{MemorySecretStore, Secret};

    fn steve() -> Account {
        Account::microsoft(
            Uuid::parse_str("069a79f4-44e9-4726-a5be-fca90e38aaf5").unwrap(),
            "Notch",
            Some("https://textures.minecraft.net/texture/abc123".into()),
        )
    }

    #[test]
    fn ids_and_secret_keys() {
        let account = steve();
        assert_eq!(account.id, "microsoft:069a79f444e94726a5befca90e38aaf5");
        assert_eq!(
            account.secret_key().as_deref(),
            Some("lodestone-launcher:microsoft:069a79f444e94726a5befca90e38aaf5")
        );
        let offline = Account::offline("Notch").unwrap();
        assert_eq!(offline.id, "offline:b50ad385829d3141a2167e7d7539ba7f");
        assert_eq!(offline.secret_key(), None);
        assert!(offline.is_offline());
        assert_eq!(
            Account::offline("no"),
            Err(crate::offline::UsernameError::TooShort)
        );
    }

    #[test]
    fn add_switch_and_persist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        let (mut store, problem) = AccountStore::open(&path);
        assert!(problem.is_none());
        assert!(store.active().is_none());

        store.upsert_and_activate(steve()).unwrap();
        let offline = Account::offline("Dev_1").unwrap();
        store.upsert_and_activate(offline.clone()).unwrap();
        assert_eq!(store.active(), Some(&offline));
        assert!(store.set_active(&steve().id).unwrap());
        assert!(!store.set_active("microsoft:missing").unwrap());

        let (reopened, _) = AccountStore::open(&path);
        assert_eq!(reopened.accounts().len(), 2);
        assert_eq!(reopened.active(), Some(&steve()));

        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["version"], json!(1));
        assert_eq!(raw["active"], json!(steve().id));
        assert_eq!(raw["accounts"][1]["kind"], json!("offline"));
    }

    #[test]
    fn upsert_replaces_an_existing_account() {
        let dir = tempfile::tempdir().unwrap();
        let (mut store, _) = AccountStore::open(dir.path().join("accounts.json"));
        store.upsert_and_activate(steve()).unwrap();
        let mut renamed = steve();
        renamed.username = "Notch2".into();
        store.upsert_and_activate(renamed).unwrap();
        assert_eq!(store.accounts().len(), 1);
        assert_eq!(store.accounts()[0].username, "Notch2");
        store.update_profile(&steve().id, "Notch3", None).unwrap();
        assert_eq!(store.accounts()[0].username, "Notch3");
        assert_eq!(store.accounts()[0].skin_url, None);
    }

    #[test]
    fn remove_deletes_the_secret_and_picks_a_new_active_account() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = MemorySecretStore::new();
        let (mut store, _) = AccountStore::open(dir.path().join("accounts.json"));
        let offline = Account::offline("Dev_1").unwrap();
        store.upsert_and_activate(offline.clone()).unwrap();
        store.upsert_and_activate(steve()).unwrap();
        let key = steve().secret_key().unwrap();
        block_on(secrets.write(&key, &Secret::new("refresh"))).unwrap();

        let removed = block_on(store.remove(&steve().id, &secrets)).unwrap();
        assert_eq!(removed, Some(steve()));
        assert_eq!(secrets.get(&key), None);
        assert_eq!(store.active(), Some(&offline));

        assert_eq!(block_on(store.remove("unknown", &secrets)).unwrap(), None);
        block_on(store.remove(&offline.id, &secrets)).unwrap();
        assert!(store.active().is_none());
        let (reopened, _) = AccountStore::open(store.path());
        assert!(reopened.accounts().is_empty());
    }

    #[test]
    fn a_failed_secret_delete_keeps_the_account() {
        struct FailingStore;
        impl SecretStore for FailingStore {
            fn read(
                &self,
                _: &str,
            ) -> futures::future::LocalBoxFuture<'_, anyhow::Result<Option<Secret>>> {
                Box::pin(async { Ok(None) })
            }
            fn write(
                &self,
                _: &str,
                _: &Secret,
            ) -> futures::future::LocalBoxFuture<'_, anyhow::Result<()>> {
                Box::pin(async { Ok(()) })
            }
            fn delete(&self, _: &str) -> futures::future::LocalBoxFuture<'_, anyhow::Result<()>> {
                Box::pin(async { Err(anyhow::anyhow!("keychain locked")) })
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let (mut store, _) = AccountStore::open(dir.path().join("accounts.json"));
        store.upsert_and_activate(steve()).unwrap();
        let err = block_on(store.remove(&steve().id, &FailingStore)).unwrap_err();
        assert!(matches!(err, RemoveError::Secret(_)));
        assert_eq!(store.accounts().len(), 1);
    }

    #[test]
    fn migrates_an_unversioned_file_and_writes_it_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        std::fs::write(
            &path,
            json!({
                "accounts": [{
                    "id": "offline:b50ad385829d3141a2167e7d7539ba7f",
                    "kind": "offline",
                    "uuid": "b50ad385-829d-3141-a216-7e7d7539ba7f",
                    "username": "Notch"
                }]
            })
            .to_string(),
        )
        .unwrap();
        let (store, problem) = AccountStore::open(&path);
        assert!(problem.is_none());
        // The active account is filled in, and the file now has a version.
        assert_eq!(store.active().unwrap().username, "Notch");
        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["version"], json!(1));
        assert_eq!(
            raw["active"],
            json!("offline:b50ad385829d3141a2167e7d7539ba7f")
        );
    }

    #[test]
    fn records_the_client_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        let (mut store, _) = AccountStore::open(&path);
        let launcher = Account::microsoft(Uuid::from_u128(7), "Jeb", None)
            .with_client(ClientIdentity::MinecraftLauncher);
        store.upsert_and_activate(steve()).unwrap();
        store.upsert_and_activate(launcher.clone()).unwrap();
        store
            .upsert_and_activate(Account::offline("Dev_1").unwrap())
            .unwrap();

        let raw: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["version"], json!(1));
        assert_eq!(raw["accounts"][0]["client"], json!("lodestone"));
        assert_eq!(raw["accounts"][1]["client"], json!("minecraft_launcher"));

        let (reopened, problem) = AccountStore::open(&path);
        assert!(problem.is_none());
        assert_eq!(reopened.get(&launcher.id), Some(&launcher));
        assert!(reopened.get(&launcher.id).unwrap().uses_launcher_identity());
        assert!(!reopened.get(&steve().id).unwrap().uses_launcher_identity());
        assert_eq!(reopened.launcher_identity_accounts(), vec![&launcher]);
        // The secret key does not depend on the identity.
        assert_eq!(launcher.secret_key(), Some(secret_key(&launcher.id)));
    }

    #[test]
    fn accounts_from_before_the_identity_use_lodestones_app() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        // A version 1 file as milestone 2 wrote it, and an unversioned one.
        for file in [
            json!({
                "version": 1,
                "active": "microsoft:069a79f444e94726a5befca90e38aaf5",
                "accounts": [{
                    "id": "microsoft:069a79f444e94726a5befca90e38aaf5",
                    "kind": "microsoft",
                    "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5",
                    "username": "Notch",
                    "skin_url": "https://textures.minecraft.net/texture/abc123"
                }]
            }),
            json!({
                "accounts": [{
                    "id": "microsoft:069a79f444e94726a5befca90e38aaf5",
                    "kind": "microsoft",
                    "uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5",
                    "username": "Notch",
                    "skin_url": "https://textures.minecraft.net/texture/abc123"
                }]
            }),
        ] {
            std::fs::write(&path, file.to_string()).unwrap();
            let (store, problem) = AccountStore::open(&path);
            assert!(problem.is_none());
            assert_eq!(store.accounts(), [steve()]);
            assert_eq!(store.accounts()[0].client, ClientIdentity::Lodestone);
            assert!(store.launcher_identity_accounts().is_empty());
        }
    }

    #[test]
    fn a_newer_file_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        let newer = r#"{"version": 9, "accounts": "a new shape"}"#;
        std::fs::write(&path, newer).unwrap();
        let (mut store, problem) = AccountStore::open(&path);
        assert!(matches!(problem, Some(OpenProblem::TooNew(_))));
        assert!(!store.is_writable());
        store.upsert_and_activate(steve()).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), newer);
    }

    #[test]
    fn a_corrupt_file_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        std::fs::write(&path, "{ not json").unwrap();
        let (mut store, problem) = AccountStore::open(&path);
        let Some(OpenProblem::Corrupt { moved_to, .. }) = problem else {
            panic!("expected a corrupt file, got {problem:?}");
        };
        let moved_to = moved_to.unwrap();
        assert_eq!(moved_to, dir.path().join("accounts.json.corrupt-1"));
        assert_eq!(std::fs::read_to_string(moved_to).unwrap(), "{ not json");
        store.upsert_and_activate(steve()).unwrap();
        assert!(path.exists());
    }
}
