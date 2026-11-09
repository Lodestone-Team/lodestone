//! API keys: named, revocable credentials that can be limited to some instances and
//! actions, for the CLI, CI and agents.
//!
//! A key looks like `lsk_<id>_<secret>`. The id is public and finds the key. The secret is
//! stored as it is, in `stores/api_keys.json` (readable only by the core's OS user, and
//! never through the file API): anyone who can read that folder can already act as any
//! user with the token secrets in `users.json`, so hashing the keys protected nothing, and
//! a stored key can be shown again. Keys made before this keep only the SHA-256 of their
//! secret, which still works. [`UsersManager::try_auth`]
//! (crate::auth::user::UsersManager::try_auth) turns a key into the [`User`] a request acts
//! as, see [`effective_user`].

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use color_eyre::eyre::{eyre, Context};
use rand::{rngs::SysRng, TryRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tracing::{error, warn};
use ts_rs::TS;

use crate::{
    error::{Error, ErrorKind},
    types::InstanceUuid,
};

use super::{
    permission::UserPermission,
    user::{AuthSource, KeyConsole, User, UserAction},
    user_id::UserId,
};

/// Every key starts with this, so the core and secret scanners can recognise one.
pub const API_KEY_PREFIX: &str = "lsk_";
/// Length of a key's public id, in `[a-z0-9]`.
pub const API_KEY_ID_LEN: usize = 12;
/// Random bytes in a key's secret.
const API_KEY_SECRET_BYTES: usize = 32;
/// Length of the secret once encoded as base64url without padding.
pub const API_KEY_SECRET_LEN: usize = 43;
/// The most keys one user may have.
pub const MAX_API_KEYS_PER_USER: usize = 50;
/// The longest key name, in characters, after trimming.
pub const MAX_API_KEY_NAME_CHARS: usize = 64;
/// The message for routes that API keys may not use, whatever their scope.
pub const SESSION_REQUIRED_MESSAGE: &str = "This action needs a signed-in session, not an API key";

const ID_ALPHABET: &[u8; 36] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// What a key may do.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum ApiKeyScope {
    /// Everything its user can do, including owner or admin powers.
    Full,
    /// Only these permissions, and only while the user still has them. Never owner or
    /// admin.
    Custom { permissions: Box<UserPermission> },
    /// The scope of a Lodestone Launcher device: viewing, starting and stopping the
    /// instances its user can view, start and stop, see [`launcher_user`]. Never owner or
    /// admin, and nothing else, except what the user opted in to when signing in:
    ///
    /// - `console_read`: reading the console of the instances whose console the user can
    ///   access;
    /// - `console_write`: sending commands (and chat) to those consoles.
    ///
    /// Each is left out of the JSON while false, so keys made before these existed, and
    /// keys without either, are `{ "type": "launcher" }` as before.
    Launcher {
        #[serde(default, skip_serializing_if = "is_false")]
        #[ts(optional, as = "Option<bool>")]
        console_read: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        #[ts(optional, as = "Option<bool>")]
        console_write: bool,
    },
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// A stored key. Holds the secret (or, for older keys, its hash), so it is never sent to
/// clients or logged: use [`ApiKeyInfo`] for that.
#[derive(Serialize, Deserialize, Clone)]
pub struct ApiKeyRecord {
    pub id: String,
    pub name: String,
    pub uid: UserId,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub last_used_at: Option<i64>,
    pub scope: ApiKeyScope,
    /// The secret, as it is. Keys made before secrets were kept have only
    /// [`ApiKeyRecord::secret_sha256`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    /// SHA-256 of the secret, as lowercase hex, for keys made before secrets were kept.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_sha256: Option<String>,
}

impl ApiKeyRecord {
    pub fn is_expired(&self, now: i64) -> bool {
        is_expired(self.expires_at, now)
    }

    /// Whether `secret` is this key's secret, compared in constant time: with the stored
    /// secret, or for an older key, with the stored hash.
    pub fn verify_secret(&self, secret: &str) -> bool {
        match (&self.secret, &self.secret_sha256) {
            (Some(stored), _) => bool::from(secret.as_bytes().ct_eq(stored.as_bytes())),
            (None, Some(hash)) => {
                let actual = hash_secret(secret);
                bool::from(actual.as_bytes().ct_eq(hash.as_bytes()))
            }
            (None, None) => false,
        }
    }

    /// The whole key, `lsk_<id>_<secret>`, for keys whose secret is stored.
    pub fn key(&self) -> Option<String> {
        self.secret
            .as_ref()
            .map(|secret| format!("{API_KEY_PREFIX}{}_{secret}", self.id))
    }

    pub fn info(&self) -> ApiKeyInfo {
        ApiKeyInfo {
            id: self.id.clone(),
            name: self.name.clone(),
            display: display_key(&self.id),
            uid: self.uid.clone(),
            created_at: self.created_at,
            expires_at: self.expires_at,
            last_used_at: self.last_used_at,
            scope: self.scope.clone(),
        }
    }
}

/// A key as clients see it: never the secret or its hash.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct ApiKeyInfo {
    pub id: String,
    pub name: String,
    /// `lsk_<id>_…`
    pub display: String,
    pub uid: UserId,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number | null")]
    pub expires_at: Option<i64>,
    #[ts(type = "number | null")]
    pub last_used_at: Option<i64>,
    pub scope: ApiKeyScope,
}

/// The body of `POST /user/api_keys`.
#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub struct CreateApiKeyRequest {
    pub name: String,
    #[ts(type = "number | null")]
    pub expires_at: Option<i64>,
    pub scope: ApiKeyScope,
}

/// The reply to `POST /user/api_keys`: the only time the key itself is sent. No `Debug`,
/// so it cannot end up in a log by accident.
#[derive(Serialize, Deserialize, Clone, TS)]
#[ts(export)]
pub struct CreatedApiKey {
    pub key: String,
    pub info: ApiKeyInfo,
}

/// How a key is shown once created: `lsk_<id>_…`.
pub fn display_key(id: &str) -> String {
    format!("{API_KEY_PREFIX}{id}_…")
}

pub fn is_expired(expires_at: Option<i64>, now: i64) -> bool {
    expires_at.is_some_and(|expires_at| now >= expires_at)
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// SHA-256 of a key's secret, as lowercase hex.
pub fn hash_secret(secret: &str) -> String {
    hex::encode(Sha256::digest(secret.as_bytes()))
}

fn os_random_bytes(buf: &mut [u8]) -> Result<(), Error> {
    SysRng.try_fill_bytes(buf).map_err(|e| Error {
        kind: ErrorKind::Internal,
        source: eyre!("Failed to read random bytes from the operating system: {e}"),
    })
}

/// A random id of [`API_KEY_ID_LEN`] characters in `[a-z0-9]`, without modulo bias.
fn generate_id() -> Result<String, Error> {
    let mut id = String::with_capacity(API_KEY_ID_LEN);
    let mut buf = [0u8; 32];
    while id.len() < API_KEY_ID_LEN {
        os_random_bytes(&mut buf)?;
        for byte in buf {
            // 252 is the largest multiple of 36 that fits in a byte
            if byte < 252 && id.len() < API_KEY_ID_LEN {
                id.push(ID_ALPHABET[(byte % 36) as usize] as char);
            }
        }
    }
    Ok(id)
}

/// A new key's id, secret and full text.
pub struct NewKey {
    pub id: String,
    pub secret: String,
    pub key: String,
}

pub fn generate_key() -> Result<NewKey, Error> {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    let id = generate_id()?;
    let mut bytes = [0u8; API_KEY_SECRET_BYTES];
    os_random_bytes(&mut bytes)?;
    let secret = URL_SAFE_NO_PAD.encode(bytes);
    let key = format!("{API_KEY_PREFIX}{id}_{secret}");
    Ok(NewKey { id, secret, key })
}

/// The id and secret of a key.
#[derive(PartialEq, Eq)]
pub struct ParsedKey<'a> {
    pub id: &'a str,
    pub secret: &'a str,
}

impl std::fmt::Debug for ParsedKey<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedKey").field("id", &self.id).finish()
    }
}

/// Splits `lsk_<id>_<secret>`. `None` unless the id and secret have the exact length and
/// alphabet of a generated key. The secret may itself contain `_` (base64url), so the
/// split is by position, not by separator.
pub fn parse_key(token: &str) -> Option<ParsedKey<'_>> {
    let rest = token.strip_prefix(API_KEY_PREFIX)?;
    if rest.len() != API_KEY_ID_LEN + 1 + API_KEY_SECRET_LEN || !rest.is_ascii() {
        return None;
    }
    let (id, rest) = rest.split_at(API_KEY_ID_LEN);
    let secret = rest.strip_prefix('_')?;
    let id_ok = id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    let secret_ok = secret
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    (id_ok && secret_ok).then_some(ParsedKey { id, secret })
}

/// The trimmed name, if it has 1 to [`MAX_API_KEY_NAME_CHARS`] characters.
pub fn validate_name(name: &str) -> Result<String, Error> {
    let name = name.trim();
    let chars = name.chars().count();
    if chars == 0 || chars > MAX_API_KEY_NAME_CHARS {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("The key name must have 1 to {MAX_API_KEY_NAME_CHARS} characters"),
        });
    }
    if name.chars().any(char::is_control) {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("The key name must not contain control characters"),
        });
    }
    Ok(name.to_string())
}

/// Every instance a custom scope names.
pub fn scope_instances(scope: &ApiKeyScope) -> Vec<&InstanceUuid> {
    match scope {
        ApiKeyScope::Full | ApiKeyScope::Launcher { .. } => Vec::new(),
        ApiKeyScope::Custom { permissions: p } => p
            .can_view_instance
            .iter()
            .chain(&p.can_start_instance)
            .chain(&p.can_stop_instance)
            .chain(&p.can_access_instance_console)
            .chain(&p.can_access_instance_setting)
            .chain(&p.can_read_instance_resource)
            .chain(&p.can_write_instance_resource)
            .chain(&p.can_access_instance_macro)
            .chain(&p.can_read_instance_file)
            .chain(&p.can_write_instance_file)
            .collect(),
    }
}

/// The permissions a custom key has right now: those it lists that `user` (the stored
/// user, with their current permissions) can use.
///
/// "Can use" is [`User::can_perform_action`], so for the owner, who can do everything,
/// the key's list applies as written; an admin's automatic powers count too. The macro
/// and extension fields grant nothing and come out empty.
pub fn effective_permissions(user: &User, key: &UserPermission) -> UserPermission {
    let keep = |set: &std::collections::HashSet<InstanceUuid>,
                action: fn(InstanceUuid) -> UserAction| {
        set.iter()
            .filter(|uuid| user.can_perform_action(&action((*uuid).clone())))
            .cloned()
            .collect()
    };
    let flag = |granted: bool, action: UserAction| granted && user.can_perform_action(&action);
    UserPermission {
        can_view_instance: keep(&key.can_view_instance, UserAction::ViewInstance),
        can_start_instance: keep(&key.can_start_instance, UserAction::StartInstance),
        can_stop_instance: keep(&key.can_stop_instance, UserAction::StopInstance),
        can_access_instance_console: keep(
            &key.can_access_instance_console,
            UserAction::AccessConsole,
        ),
        can_access_instance_setting: keep(
            &key.can_access_instance_setting,
            UserAction::AccessSetting,
        ),
        can_read_instance_resource: keep(&key.can_read_instance_resource, UserAction::ReadResource),
        can_write_instance_resource: keep(
            &key.can_write_instance_resource,
            UserAction::WriteResource,
        ),
        can_access_instance_macro: Default::default(),
        can_read_instance_file: keep(&key.can_read_instance_file, UserAction::ReadInstanceFile),
        can_write_instance_file: keep(&key.can_write_instance_file, UserAction::WriteInstanceFile),
        can_create_instance: flag(key.can_create_instance, UserAction::CreateInstance),
        can_delete_instance: flag(key.can_delete_instance, UserAction::DeleteInstance),
        can_read_global_file: flag(key.can_read_global_file, UserAction::ReadGlobalFile),
        can_write_global_file: flag(key.can_write_global_file, UserAction::WriteGlobalFile),
        can_manage_permission: flag(key.can_manage_permission, UserAction::ManagePermission),
        can_install_extension: false,
    }
}

/// What a launcher key's user (the stored `user`) may do through the key: view, start
/// and stop the instances `user` can view, start and stop right now. With `console`
/// opted in, also read and/or write the consoles `user` can access right now. Nothing
/// else.
///
/// The owner and admins can do that on every instance, including ones created after the
/// key, so for them it is [`User::key_all_instances`] rather than a list.
pub fn launcher_user(user: &User, console: KeyConsole) -> User {
    let mut effective = user.clone();
    let all_instances = user.is_owner || user.is_admin;
    let mut permissions = UserPermission::new();
    if !all_instances {
        permissions.can_view_instance = user.permissions.can_view_instance.clone();
        permissions.can_start_instance = user.permissions.can_start_instance.clone();
        permissions.can_stop_instance = user.permissions.can_stop_instance.clone();
        if console.read || console.write {
            permissions.can_access_instance_console =
                user.permissions.can_access_instance_console.clone();
        }
    }
    effective.permissions = permissions;
    effective.is_owner = false;
    effective.is_admin = false;
    effective.key_all_instances = all_instances;
    effective.key_console = Some(console);
    effective
}

/// The user a request authenticated with `record` acts as. `user` is the key's stored
/// user.
///
/// A full key is the user. A custom key is the user without owner or admin powers and
/// with only [`effective_permissions`]. A launcher key is [`launcher_user`]. All carry
/// [`AuthSource::ApiKey`].
pub fn effective_user(record: &ApiKeyRecord, user: &User) -> User {
    let mut effective = match &record.scope {
        ApiKeyScope::Full => user.clone(),
        ApiKeyScope::Custom { permissions } => {
            let mut effective = user.clone();
            effective.permissions = effective_permissions(user, permissions);
            effective.is_owner = false;
            effective.is_admin = false;
            effective
        }
        ApiKeyScope::Launcher {
            console_read,
            console_write,
        } => launcher_user(
            user,
            KeyConsole {
                read: *console_read,
                write: *console_write,
            },
        ),
    };
    effective.auth_source = AuthSource::ApiKey {
        id: record.id.clone(),
        name: record.name.clone(),
        expires_at: record.expires_at,
    };
    effective
}

/// The keys, as kept in memory and in `stores/api_keys.json`.
///
/// Changes to the set of keys need `&mut self` (the users write lock). Last-used times are
/// recorded with `&self`, during authentication, and saved by [`ApiKeyStore::flush`] at
/// most once a minute and at shutdown.
pub struct ApiKeyStore {
    path: PathBuf,
    keys: HashMap<String, ApiKeyRecord>,
    /// Last-used times recorded since the keys were last saved, by key id.
    last_used: std::sync::Mutex<HashMap<String, i64>>,
    /// Whether `last_used` has times that are not saved yet.
    dirty: AtomicBool,
    /// Held while a snapshot is taken and written, so that writes reach the disk in the
    /// order their snapshots were taken.
    file_write: tokio::sync::Mutex<()>,
    /// The file could not be read or parsed at startup. It is kept as it is until a key is
    /// created, see [`ApiKeyStore::insert`].
    unreadable: bool,
}

impl ApiKeyStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            keys: HashMap::new(),
            last_used: Default::default(),
            dirty: AtomicBool::new(false),
            file_write: Default::default(),
            unreadable: false,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads the keys. A missing or empty file means no keys. A file that cannot be read or
    /// parsed is logged and also means no keys, but is not overwritten until a key is
    /// created.
    pub async fn load(&mut self) {
        self.keys.clear();
        self.last_used_lock().clear();
        self.unreadable = false;
        let data = match tokio::fs::read(&self.path).await {
            Ok(data) => data,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => {
                error!(
                    "Failed to read the API key file {}: {e}. No API key will work. The file is \
                     left as it is until a new key is created",
                    self.path.display()
                );
                self.unreadable = true;
                return;
            }
        };
        if data.iter().all(u8::is_ascii_whitespace) {
            return;
        }
        match serde_json::from_slice::<Vec<ApiKeyRecord>>(&data) {
            Ok(records) => {
                self.keys = records.into_iter().map(|r| (r.id.clone(), r)).collect();
            }
            Err(e) => {
                error!(
                    "The API key file {} is corrupt ({e}). No API key will work. The file is \
                     left as it is until a new key is created; then it is moved to {} and \
                     replaced",
                    self.path.display(),
                    self.corrupt_backup_path().display()
                );
                self.unreadable = true;
            }
        }
    }

    fn corrupt_backup_path(&self) -> PathBuf {
        let mut name = self
            .path
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_else(|| "api_keys.json".into());
        name.push(".corrupt");
        self.path.with_file_name(name)
    }

    fn last_used_lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, i64>> {
        // the map only holds timestamps, so a panic while it was locked cannot leave it
        // inconsistent
        self.last_used
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn get(&self, id: &str) -> Option<&ApiKeyRecord> {
        self.keys.get(id)
    }

    /// A copy of the record with the latest last-used time.
    fn current(&self, record: &ApiKeyRecord) -> ApiKeyRecord {
        let mut record = record.clone();
        if let Some(t) = self.last_used_lock().get(&record.id) {
            record.last_used_at = Some(record.last_used_at.map_or(*t, |old| old.max(*t)));
        }
        record
    }

    /// Records a successful use of the key `id`, in memory only.
    pub fn record_use(&self, id: &str, now: i64) {
        self.last_used_lock().insert(id.to_string(), now);
        self.dirty.store(true, Ordering::SeqCst);
    }

    /// The keys of `uid`, newest first.
    pub fn list_for_user(&self, uid: &UserId) -> Vec<ApiKeyInfo> {
        let mut keys: Vec<ApiKeyInfo> = self
            .keys
            .values()
            .filter(|r| r.uid == *uid)
            .map(|r| self.current(r).info())
            .collect();
        keys.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        keys
    }

    pub fn count_for_user(&self, uid: &UserId) -> usize {
        self.keys.values().filter(|r| r.uid == *uid).count()
    }

    /// Writes every key to the file, readable only by the current OS user.
    async fn save(&self) -> Result<(), Error> {
        let _guard = self.file_write.lock().await;
        // cleared before the snapshot: a use recorded after it marks the store dirty again
        self.dirty.store(false, Ordering::SeqCst);
        let mut records: Vec<ApiKeyRecord> = self.keys.values().map(|r| self.current(r)).collect();
        records.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        let data = serde_json::to_vec(&records).context("Failed to serialize the API keys")?;
        let result = crate::persist::write_atomic(&self.path, data, Some(0o600))
            .await
            .context(format!(
                "Failed to write the API key file {}",
                self.path.display()
            ));
        if result.is_err() {
            self.dirty.store(true, Ordering::SeqCst);
        }
        Ok(result?)
    }

    /// Saves the last-used times recorded since the last save, if any.
    pub async fn flush(&self) -> Result<(), Error> {
        if self.unreadable || !self.dirty.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.save().await
    }

    /// Adds a key and saves the file. A file that could not be read at startup is moved
    /// aside first, so its contents are not lost.
    pub async fn insert(&mut self, record: ApiKeyRecord) -> Result<(), Error> {
        if self.unreadable {
            let backup = self.corrupt_backup_path();
            match tokio::fs::rename(&self.path, &backup).await {
                Ok(()) => warn!(
                    "Moved the unreadable API key file {} to {} and started a new one",
                    self.path.display(),
                    backup.display()
                ),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(Error {
                        kind: ErrorKind::Internal,
                        source: eyre!(
                            "The API key file {} is unreadable and could not be moved aside: {e}",
                            self.path.display()
                        ),
                    })
                }
            }
            self.unreadable = false;
        }
        let id = record.id.clone();
        self.keys.insert(id.clone(), record);
        if let Err(e) = self.save().await {
            self.keys.remove(&id);
            return Err(e);
        }
        Ok(())
    }

    /// Removes the key `id` of `uid` and saves the file. `None` if `uid` has no such key.
    pub async fn remove(&mut self, uid: &UserId, id: &str) -> Result<Option<ApiKeyRecord>, Error> {
        if !self.keys.get(id).is_some_and(|r| r.uid == *uid) {
            return Ok(None);
        }
        let Some(record) = self.keys.remove(id) else {
            return Ok(None);
        };
        if let Err(e) = self.save().await {
            self.keys.insert(record.id.clone(), record);
            return Err(e);
        }
        self.last_used_lock().remove(id);
        Ok(Some(record))
    }

    /// Removes every key of `uid` from memory at once, then saves the file. The keys stop
    /// working even if the save fails, since they are gone from memory; the error is
    /// returned for logging.
    pub async fn remove_user(&mut self, uid: &UserId) -> Result<usize, Error> {
        let before = self.keys.len();
        self.keys.retain(|_, r| r.uid != *uid);
        let removed = before - self.keys.len();
        if removed == 0 {
            return Ok(0);
        }
        {
            let keys = &self.keys;
            self.last_used
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .retain(|id, _| keys.contains_key(id));
        }
        self.save().await?;
        Ok(removed)
    }

    /// Forgets keys whose user is gone, in memory only.
    pub fn retain_users(&mut self, exists: impl Fn(&UserId) -> bool) {
        self.keys.retain(|_, r| exists(&r.uid));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(uid: &UserId, scope: ApiKeyScope, expires_at: Option<i64>) -> (ApiKeyRecord, String) {
        let new = generate_key().unwrap();
        (
            ApiKeyRecord {
                id: new.id,
                name: "ci".to_string(),
                uid: uid.clone(),
                created_at: now(),
                expires_at,
                last_used_at: None,
                scope,
                secret: Some(new.secret),
                secret_sha256: None,
            },
            new.key,
        )
    }

    #[test]
    fn generated_keys_have_the_documented_format() {
        let mut ids = std::collections::HashSet::new();
        for _ in 0..50 {
            let new = generate_key().unwrap();
            assert_eq!(new.id.len(), API_KEY_ID_LEN);
            assert!(new
                .id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()));
            assert_eq!(new.secret.len(), API_KEY_SECRET_LEN);
            assert!(!new.secret.contains('='));
            assert_eq!(new.key, format!("lsk_{}_{}", new.id, new.secret));
            let parsed = parse_key(&new.key).unwrap();
            assert_eq!(parsed.id, new.id);
            assert_eq!(parsed.secret, new.secret);
            assert!(ids.insert(new.id));
        }
    }

    #[test]
    fn parsing_rejects_anything_else() {
        let good = format!("lsk_abcdefghij12_{}", "A".repeat(API_KEY_SECRET_LEN));
        assert!(parse_key(&good).is_some());
        // a secret with `_` and `-` (base64url) still splits at the right place
        let underscores = format!("lsk_abcdefghij12_{}", "_-".repeat(21) + "a");
        assert_eq!(parse_key(&underscores).unwrap().id, "abcdefghij12");
        for bad in [
            String::new(),
            "lsk_".to_string(),
            good.replacen("lsk_", "lsx_", 1),
            good.replacen("abcdefghij12", "ABCDEFGHIJ12", 1),
            good.replacen("abcdefghij12_", "abcdefghij12-", 1),
            format!("{good}A"),
            good[..good.len() - 1].to_string(),
            format!("lsk_abcdefghij12_{}", "A".repeat(42) + "="),
            format!("lsk_abcdefghij12_{}", "A".repeat(42) + "é"),
            "eyJhbGciOiJIUzUxMiJ9.e30.sig".to_string(),
        ] {
            assert!(parse_key(&bad).is_none(), "{bad}");
        }
        // the Debug output never shows the secret
        let parsed = parse_key(&good).unwrap();
        assert!(!format!("{parsed:?}").contains(&"A".repeat(10)));
    }

    #[test]
    fn secrets_are_stored_and_checked() {
        let uid = UserId::default();
        let (record, key) = record(&uid, ApiKeyScope::Full, None);
        let parsed = parse_key(&key).unwrap();
        assert_eq!(record.secret.as_deref(), Some(parsed.secret));
        assert_eq!(record.key().as_deref(), Some(key.as_str()));
        assert!(record.verify_secret(parsed.secret));
        let other = generate_key().unwrap();
        assert!(!record.verify_secret(&other.secret));
        assert!(!record.verify_secret(""));
        // a prefix of the secret is not the secret
        assert!(!record.verify_secret(&parsed.secret[..10]));
    }

    #[test]
    fn keys_made_before_secrets_were_kept_still_work() {
        let uid = UserId::default();
        let (mut record, key) = record(&uid, ApiKeyScope::Full, None);
        let parsed = parse_key(&key).unwrap();
        let hash = hash_secret(parsed.secret);
        record.secret = None;
        record.secret_sha256 = Some(hash.clone());
        assert!(record.verify_secret(parsed.secret));
        assert!(!record.verify_secret(&generate_key().unwrap().secret));
        // the hash itself is not accepted as the secret
        assert!(!record.verify_secret(&hash));
        assert_eq!(record.key(), None);
        // an older file still parses
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("secret_sha256") && !json.contains("\"secret\":"));
        let back: ApiKeyRecord = serde_json::from_str(&json).unwrap();
        assert!(back.verify_secret(parsed.secret));
        // a record with neither accepts nothing
        record.secret_sha256 = None;
        assert!(!record.verify_secret(parsed.secret));
        assert!(!record.verify_secret(""));
    }

    #[test]
    fn known_hash() {
        // known answer
        assert_eq!(
            hash_secret("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn expiry() {
        assert!(!is_expired(None, i64::MAX));
        assert!(!is_expired(Some(100), 99));
        assert!(is_expired(Some(100), 100));
        assert!(is_expired(Some(100), 101));
    }

    #[test]
    fn names_are_trimmed_and_limited() {
        assert_eq!(validate_name("  ci  ").unwrap(), "ci");
        assert!(validate_name("   ").is_err());
        assert!(validate_name("").is_err());
        assert_eq!(validate_name(&"é".repeat(64)).unwrap().chars().count(), 64);
        assert!(validate_name(&"a".repeat(65)).is_err());
        assert!(validate_name("a\nb").is_err());
    }

    #[test]
    fn info_never_contains_the_secret() {
        let uid = UserId::default();
        let (record, key) = record(&uid, ApiKeyScope::Full, Some(5));
        let info = serde_json::to_string(&record.info()).unwrap();
        assert!(!info.contains(parse_key(&key).unwrap().secret));
        assert!(!info.contains(&hash_secret(parse_key(&key).unwrap().secret)));
        assert!(info.contains(&format!("\"display\":\"lsk_{}_…\"", record.id)));
        let value: serde_json::Value = serde_json::from_str(&info).unwrap();
        assert_eq!(value["scope"], serde_json::json!({ "type": "full" }));
        assert_eq!(value["expires_at"], 5);
        assert_eq!(value["last_used_at"], serde_json::Value::Null);
    }

    #[test]
    fn scope_json_shape() {
        let custom = ApiKeyScope::Custom {
            permissions: Box::default(),
        };
        let value = serde_json::to_value(&custom).unwrap();
        assert_eq!(value["type"], "custom");
        assert!(value["permissions"]["can_view_instance"].is_array());
        let parsed: ApiKeyScope = serde_json::from_value(value).unwrap();
        assert_eq!(parsed, custom);
        let parsed: ApiKeyScope = serde_json::from_str(r#"{"type":"full"}"#).unwrap();
        assert_eq!(parsed, ApiKeyScope::Full);
    }

    #[tokio::test]
    async fn store_file_is_private_and_debounced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api_keys.json");
        let uid = UserId::default();
        let mut store = ApiKeyStore::new(path.clone());
        store.load().await;
        let (record, _) = record(&uid, ApiKeyScope::Full, None);
        let id = record.id.clone();
        store.insert(record).await.unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let saved = std::fs::read(&path).unwrap();

        // a use is kept in memory only
        store.record_use(&id, 1234);
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        assert_eq!(store.list_for_user(&uid)[0].last_used_at, Some(1234));

        // and written by the next flush, once
        store.flush().await.unwrap();
        let flushed = std::fs::read(&path).unwrap();
        assert_ne!(flushed, saved);
        std::fs::remove_file(&path).unwrap();
        store.flush().await.unwrap();
        assert!(!path.exists(), "a flush with nothing new must not write");
        std::fs::write(&path, &flushed).unwrap();

        let mut reloaded = ApiKeyStore::new(path.clone());
        reloaded.load().await;
        assert_eq!(reloaded.list_for_user(&uid)[0].last_used_at, Some(1234));
    }

    #[tokio::test]
    async fn corrupt_file_is_kept_until_a_key_is_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api_keys.json");
        std::fs::write(&path, "{ not json").unwrap();
        let uid = UserId::default();
        let mut store = ApiKeyStore::new(path.clone());
        store.load().await;
        assert!(store.list_for_user(&uid).is_empty());

        store.record_use("whatever", 1);
        store.flush().await.unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");
        assert_eq!(store.remove_user(&uid).await.unwrap(), 0);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{ not json");

        let (record, _) = record(&uid, ApiKeyScope::Full, None);
        store.insert(record).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("api_keys.json.corrupt")).unwrap(),
            "{ not json"
        );
        let mut reloaded = ApiKeyStore::new(path.clone());
        reloaded.load().await;
        assert_eq!(reloaded.list_for_user(&uid).len(), 1);
    }

    #[tokio::test]
    async fn missing_and_empty_files_mean_no_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api_keys.json");
        let mut store = ApiKeyStore::new(path.clone());
        store.load().await;
        assert!(!store.unreadable);
        assert!(!path.exists(), "loading must not create the file");
        std::fs::write(&path, "").unwrap();
        store.load().await;
        assert!(!store.unreadable);
    }
}

/// [`UsersManager`](crate::auth::user::UsersManager) with keys: how a key authenticates and
/// what it may do.
#[cfg(test)]
mod manager_tests {
    use std::collections::{HashMap, HashSet};

    use super::*;
    use crate::{auth::user::UsersManager, event_broadcaster::EventBroadcaster, events::CausedBy};

    fn uuid(s: &str) -> InstanceUuid {
        InstanceUuid::from(s.to_string())
    }

    fn set(uuids: &[&InstanceUuid]) -> HashSet<InstanceUuid> {
        uuids.iter().map(|u| (*u).clone()).collect()
    }

    /// Every permission, for `instances`.
    fn everything(instances: &[&InstanceUuid]) -> UserPermission {
        UserPermission {
            can_view_instance: set(instances),
            can_start_instance: set(instances),
            can_stop_instance: set(instances),
            can_access_instance_console: set(instances),
            can_access_instance_setting: set(instances),
            can_read_instance_resource: set(instances),
            can_write_instance_resource: set(instances),
            can_access_instance_macro: set(instances),
            can_read_instance_file: set(instances),
            can_write_instance_file: set(instances),
            can_create_instance: true,
            can_delete_instance: true,
            can_read_global_file: true,
            can_write_global_file: true,
            can_manage_permission: true,
            can_install_extension: true,
        }
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        manager: UsersManager,
    }

    async fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let (tx, _rx) = EventBroadcaster::new(64);
        let mut manager = UsersManager::new(tx, HashMap::new(), dir.path().join("users.json"));
        manager.load_users().await.unwrap();
        Fixture { _dir: dir, manager }
    }

    async fn add(
        manager: &mut UsersManager,
        name: &str,
        is_owner: bool,
        is_admin: bool,
        permissions: UserPermission,
    ) -> User {
        let user = User::new(name.to_string(), "pw", is_owner, is_admin, permissions);
        manager
            .add_user(user.clone(), CausedBy::System)
            .await
            .unwrap();
        user
    }

    async fn key(manager: &mut UsersManager, user: &User, scope: ApiKeyScope) -> String {
        manager
            .create_api_key(&user.uid, "ci".to_string(), None, scope, CausedBy::System)
            .await
            .unwrap()
            .key
    }

    fn custom(permissions: UserPermission) -> ApiKeyScope {
        ApiKeyScope::Custom {
            permissions: Box::new(permissions),
        }
    }

    #[tokio::test]
    async fn full_key_is_the_user_and_custom_key_is_never_owner() {
        let mut f = fixture().await;
        let a = uuid("a");
        let owner = add(&mut f.manager, "owner", true, false, Default::default()).await;
        let full = key(&mut f.manager, &owner, ApiKeyScope::Full).await;
        let narrow = key(
            &mut f.manager,
            &owner,
            custom(UserPermission {
                can_view_instance: set(&[&a]),
                ..Default::default()
            }),
        )
        .await;

        let user = f.manager.try_auth(&full).unwrap();
        assert!(user.is_owner);
        assert!(user.is_api_key());
        assert!(user
            .try_action(&UserAction::AccessMacro(None), true)
            .is_ok());
        assert!(user.try_action(&UserAction::InstallExtension, true).is_ok());

        let user = f.manager.try_auth(&narrow).unwrap();
        assert!(!user.is_owner && !user.is_admin);
        assert!(user
            .try_action(&UserAction::ViewInstance(a.clone()), true)
            .is_ok());
        assert!(user
            .try_action(&UserAction::StartInstance(a), false)
            .is_err());
        // owner-only features: macros, extensions, atoms, script writes, managing users
        assert!(user
            .try_action(&UserAction::AccessMacro(None), false)
            .is_err());
        assert!(user
            .try_action(&UserAction::InstallExtension, false)
            .is_err());
        assert!(user.owner_only("create atom servers").is_err());
        assert!(crate::handlers::util::check_code_write(
            user.is_owner,
            &crate::global_settings::CodeExecutionSwitch::new(true)
        )
        .is_err());
        assert!(user.try_action(&UserAction::ManageUser, false).is_err());
    }

    #[tokio::test]
    async fn owner_custom_key_gets_its_list_as_written() {
        let mut f = fixture().await;
        let a = uuid("a");
        let owner = add(&mut f.manager, "owner", true, false, Default::default()).await;
        let k = key(&mut f.manager, &owner, custom(everything(&[&a]))).await;
        let user = f.manager.try_auth(&k).unwrap();
        let mut expected = everything(&[&a]);
        // they grant nothing, so they come out empty
        expected.can_access_instance_macro.clear();
        expected.can_install_extension = false;
        assert_eq!(user.permissions, expected);
    }

    #[tokio::test]
    async fn admin_custom_key_cannot_exceed_the_admin() {
        let mut f = fixture().await;
        let a = uuid("a");
        // an admin with no explicit permissions: views, starts, creates... but may not
        // write files or resources, use global files or manage permissions
        let admin = add(&mut f.manager, "admin", false, true, Default::default()).await;
        let k = key(&mut f.manager, &admin, custom(everything(&[&a]))).await;
        let user = f.manager.try_auth(&k).unwrap();
        assert!(!user.is_admin);
        let p = &user.permissions;
        assert_eq!(p.can_view_instance, set(&[&a]));
        assert_eq!(p.can_start_instance, set(&[&a]));
        assert_eq!(p.can_read_instance_file, set(&[&a]));
        assert!(p.can_create_instance && p.can_delete_instance);
        assert!(p.can_write_instance_resource.is_empty());
        assert!(p.can_write_instance_file.is_empty());
        assert!(!p.can_read_global_file);
        assert!(!p.can_write_global_file);
        assert!(!p.can_manage_permission);
        for action in [
            UserAction::WriteResource(a.clone()),
            UserAction::WriteInstanceFile(a.clone()),
            UserAction::ReadGlobalFile,
            UserAction::WriteGlobalFile,
            UserAction::ManagePermission,
        ] {
            assert!(!admin.can_perform_action(&action));
            assert!(!user.can_perform_action(&action));
        }
        // an instance the key does not name is out of reach, even though the admin can
        // view every instance
        assert!(!user.can_perform_action(&UserAction::ViewInstance(uuid("b"))));
    }

    #[tokio::test]
    async fn member_custom_key_cannot_exceed_the_member() {
        let mut f = fixture().await;
        let (a, b) = (uuid("a"), uuid("b"));
        let member = add(
            &mut f.manager,
            "member",
            false,
            false,
            UserPermission {
                can_view_instance: set(&[&a]),
                can_start_instance: set(&[&a]),
                can_create_instance: true,
                ..Default::default()
            },
        )
        .await;
        let k = key(&mut f.manager, &member, custom(everything(&[&a, &b]))).await;
        let user = f.manager.try_auth(&k).unwrap();
        let expected = UserPermission {
            can_view_instance: set(&[&a]),
            can_start_instance: set(&[&a]),
            can_create_instance: true,
            ..Default::default()
        };
        assert_eq!(user.permissions, expected);
    }

    #[tokio::test]
    async fn losing_a_permission_applies_to_keys_at_once() {
        let mut f = fixture().await;
        let a = uuid("a");
        let member = add(
            &mut f.manager,
            "member",
            false,
            false,
            UserPermission {
                can_view_instance: set(&[&a]),
                can_stop_instance: set(&[&a]),
                ..Default::default()
            },
        )
        .await;
        let k = key(
            &mut f.manager,
            &member,
            custom(UserPermission {
                can_view_instance: set(&[&a]),
                can_stop_instance: set(&[&a]),
                ..Default::default()
            }),
        )
        .await;
        let user = f.manager.try_auth(&k).unwrap();
        assert!(user.can_perform_action(&UserAction::StopInstance(a.clone())));

        f.manager
            .modify_permissions(
                &member.uid,
                |p| {
                    p.can_stop_instance.clear();
                },
                CausedBy::System,
            )
            .await
            .unwrap();
        let after = f.manager.try_auth(&k).unwrap();
        assert!(!after.can_perform_action(&UserAction::StopInstance(a.clone())));
        assert!(after.can_perform_action(&UserAction::ViewInstance(a.clone())));
        // a copy kept from before (a stream, a download link) is re-derived the same way
        let refreshed = f.manager.refresh_user(&user).unwrap();
        assert!(!refreshed.can_perform_action(&UserAction::StopInstance(a.clone())));

        // and getting the permission back gives it back to the key, as the key lists it
        f.manager
            .modify_permissions(
                &member.uid,
                |p| {
                    p.can_stop_instance.insert(a.clone());
                },
                CausedBy::System,
            )
            .await
            .unwrap();
        let back = f.manager.try_auth(&k).unwrap();
        assert!(back.can_perform_action(&UserAction::StopInstance(a)));
    }

    #[tokio::test]
    async fn safe_mode_blocks_unsafe_permissions_of_custom_keys() {
        let mut f = fixture().await;
        let a = uuid("a");
        let owner = add(&mut f.manager, "owner", true, false, Default::default()).await;
        let k = key(&mut f.manager, &owner, custom(everything(&[&a]))).await;
        let user = f.manager.try_auth(&k).unwrap();
        for action in [
            UserAction::WriteInstanceFile(a.clone()),
            UserAction::WriteResource(a.clone()),
            UserAction::ReadGlobalFile,
            UserAction::WriteGlobalFile,
        ] {
            assert!(!action.is_safe());
            assert!(user.try_action(&action, false).is_ok());
            assert!(user.try_action(&action, true).is_err());
        }
        assert!(user
            .try_action(&UserAction::ViewInstance(a.clone()), true)
            .is_ok());
        // the owner's full key is the owner, whom safe mode does not restrict
        let full = key(&mut f.manager, &owner, ApiKeyScope::Full).await;
        let user = f.manager.try_auth(&full).unwrap();
        assert!(user.try_action(&UserAction::WriteGlobalFile, true).is_ok());
    }

    #[tokio::test]
    async fn authentication_fails_for_bad_expired_revoked_and_orphaned_keys() {
        let mut f = fixture().await;
        let owner = add(&mut f.manager, "owner", true, false, Default::default()).await;
        let member = add(&mut f.manager, "member", false, false, Default::default()).await;
        let good = key(&mut f.manager, &owner, ApiKeyScope::Full).await;
        assert!(f.manager.try_auth(&good).is_some());

        // wrong secret for an existing id
        let parsed = parse_key(&good).unwrap();
        let other = generate_key().unwrap();
        let forged = format!("lsk_{}_{}", parsed.id, other.secret);
        assert!(f.manager.try_auth(&forged).is_none());
        // unknown id, malformed, and the bare prefix
        assert!(f.manager.try_auth(&other.key).is_none());
        assert!(f.manager.try_auth(&good[..good.len() - 1]).is_none());
        assert!(f.manager.try_auth("lsk_").is_none());

        // expired
        let expired = f
            .manager
            .create_api_key(
                &owner.uid,
                "old".to_string(),
                Some(now() - 1),
                ApiKeyScope::Full,
                CausedBy::System,
            )
            .await
            .unwrap();
        assert!(f.manager.try_auth(&expired.key).is_none());
        let expiring = f
            .manager
            .create_api_key(
                &owner.uid,
                "soon".to_string(),
                Some(now() + 3600),
                ApiKeyScope::Full,
                CausedBy::System,
            )
            .await
            .unwrap();
        let user = f.manager.try_auth(&expiring.key).unwrap();
        assert!(!user.api_key_expired(now()));
        assert!(user.api_key_expired(now() + 3600));

        // revoked: at once, and only that key
        f.manager
            .revoke_api_key(&owner.uid, &expiring.info.id, CausedBy::System)
            .await
            .unwrap();
        assert!(f.manager.try_auth(&expiring.key).is_none());
        assert!(f.manager.refresh_user(&user).is_none());
        assert!(f.manager.try_auth(&good).is_some());
        // someone else's key id is not found, and stays valid
        let theirs = parse_key(&good).unwrap().id.to_string();
        let err = f
            .manager
            .revoke_api_key(&member.uid, &theirs, CausedBy::System)
            .await
            .unwrap_err();
        assert!(matches!(err.kind, ErrorKind::NotFound));
        assert!(f.manager.try_auth(&good).is_some());

        // the user is deleted: their keys are deleted too
        let member_key = key(&mut f.manager, &member, ApiKeyScope::Full).await;
        f.manager
            .delete_user(&member.uid, CausedBy::System)
            .await
            .unwrap();
        assert!(f.manager.try_auth(&member_key).is_none());
        assert!(f.manager.list_api_keys(&member.uid).is_empty());
        let file = std::fs::read_to_string(f._dir.path().join("api_keys.json")).unwrap();
        assert!(!file.contains(&member.uid.to_string()));
    }

    #[tokio::test]
    async fn sessions_still_work_and_are_marked_as_sessions() {
        let mut f = fixture().await;
        let owner = add(&mut f.manager, "owner", true, false, Default::default()).await;
        let jwt = owner.create_jwt().unwrap();
        let user = f.manager.try_auth(jwt.as_ref()).unwrap();
        assert!(!user.is_api_key());
        assert!(user.require_session().is_ok());
        assert_eq!(
            user.caused_by(),
            CausedBy::User {
                user_id: owner.uid.clone(),
                user_name: "owner".to_string()
            }
        );
        let k = key(&mut f.manager, &owner, ApiKeyScope::Full).await;
        let user = f.manager.try_auth(&k).unwrap();
        let err = user.require_session().unwrap_err();
        assert!(matches!(err.kind, ErrorKind::PermissionDenied));
        assert_eq!(err.source.to_string(), SESSION_REQUIRED_MESSAGE);
        assert_eq!(
            user.caused_by(),
            CausedBy::User {
                user_id: owner.uid.clone(),
                user_name: "owner (API key: ci)".to_string()
            }
        );
    }

    #[tokio::test]
    async fn the_limit_is_fifty_keys_per_user() {
        let mut f = fixture().await;
        let owner = add(&mut f.manager, "owner", true, false, Default::default()).await;
        let member = add(&mut f.manager, "member", false, false, Default::default()).await;
        for _ in 0..MAX_API_KEYS_PER_USER {
            key(&mut f.manager, &owner, ApiKeyScope::Full).await;
        }
        let err = f
            .manager
            .create_api_key(
                &owner.uid,
                "one too many".to_string(),
                None,
                ApiKeyScope::Full,
                CausedBy::System,
            )
            .await
            .err()
            .unwrap();
        assert!(matches!(err.kind, ErrorKind::BadRequest));
        // the limit is per user
        key(&mut f.manager, &member, ApiKeyScope::Full).await;
    }

    #[tokio::test]
    async fn last_used_is_recorded_in_memory_and_saved_by_flush() {
        let mut f = fixture().await;
        let owner = add(&mut f.manager, "owner", true, false, Default::default()).await;
        let k = key(&mut f.manager, &owner, ApiKeyScope::Full).await;
        let path = f._dir.path().join("api_keys.json");
        let saved = std::fs::read(&path).unwrap();
        assert_eq!(f.manager.list_api_keys(&owner.uid)[0].last_used_at, None);

        for _ in 0..5 {
            f.manager.try_auth(&k).unwrap();
        }
        assert!(f.manager.list_api_keys(&owner.uid)[0]
            .last_used_at
            .is_some());
        assert_eq!(std::fs::read(&path).unwrap(), saved, "no write per request");

        f.manager.flush_api_keys().await.unwrap();
        let mut reloaded = UsersManager::new(
            EventBroadcaster::new(4).0,
            HashMap::new(),
            f._dir.path().join("users.json"),
        );
        reloaded.load_users().await.unwrap();
        assert!(reloaded.list_api_keys(&owner.uid)[0].last_used_at.is_some());
        // the reloaded key still works
        assert!(reloaded.try_auth(&k).is_some());
    }

    #[tokio::test]
    async fn events_name_the_key_but_never_carry_it() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, mut rx) = EventBroadcaster::new(64);
        let mut manager = UsersManager::new(tx, HashMap::new(), dir.path().join("users.json"));
        let owner = add(&mut manager, "owner", true, false, Default::default()).await;
        let created = manager
            .create_api_key(
                &owner.uid,
                "deploy bot".to_string(),
                None,
                ApiKeyScope::Full,
                owner.caused_by(),
            )
            .await
            .unwrap();
        manager
            .revoke_api_key(&owner.uid, &created.info.id, owner.caused_by())
            .await
            .unwrap();
        let secret = parse_key(&created.key).unwrap().secret.to_string();
        let hash = hash_secret(&secret);
        let mut details = Vec::new();
        while let Ok(event) = rx.try_recv() {
            let json = serde_json::to_string(&event).unwrap();
            assert!(!json.contains(&secret) && !json.contains(&hash), "{json}");
            if let crate::events::EventInner::UserEvent(_) = &event.event_inner {
                details.push(event.details.clone());
            }
        }
        assert!(details.contains(&"API key `deploy bot` created for owner".to_string()));
        assert!(details.contains(&"API key `deploy bot` revoked for owner".to_string()));
    }
}
