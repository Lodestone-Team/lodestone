//! The friend list (launcher/docs/friends.md): people this person added once, whose
//! servers they can see and join without links.
//!
//! - [`Book`]: the friends, pending friend links and this person's own state, kept in
//!   `<data>/friend_list.json` sealed with a key from the Keychain, so the friend graph
//!   isn't readable from the disk alone. The person key is in the Keychain too.
//! - [`Friends`]: the book with the person key and the service, and everything that
//!   talks to the service: friend links, states, presence, servers and tunnels
//!   ([`ops`]).
//! - [`core_sync`]: the friend list a core of this person's needs, for the app that has
//!   an owner session to it (Lodestone Host).
//!
//! Network calls run on a thread with a small Tokio runtime ([`on_tokio`]), as invite
//! tunnels do; the Keychain is read and written on the caller's executor.

pub mod core_sync;
pub mod ops;

use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures::channel::oneshot;
use lodestone_connect::{
    Service,
    friend::{
        Secret as FriendSecret,
        crypto::{self, PersonKey, PublicKey, Role},
        pair::{clean_name, name_skeleton},
        state::StateCore,
    },
};
use serde::{Deserialize, Serialize};

use crate::{Secret, SecretStore, paths::DataDir, storage};

/// The Keychain item with the person key (PKCS#8, in hex).
pub const IDENTITY_SECRET: &str = "friend-identity";
/// The Keychain item with the key the friend file is sealed with (hex).
pub const FILE_KEY_SECRET: &str = "friend-list-key";
/// The friend file's name in the data folder.
pub const FILE_NAME: &str = "friend_list.json";
/// The most friends one person keeps.
pub const MAX_FRIENDS: usize = 200;

const FILE_VERSION: u32 = 1;

/// Seconds since 1970.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Where a friendship stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FriendStatus {
    /// This person answered a friend link; waiting for the sender's first state, which
    /// is their confirmation. Nothing is shared with them meanwhile.
    #[default]
    Pending,
    Active,
    /// They retired the key this friendship was with.
    Retired,
}

/// What a friend's latest state said.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendState {
    pub seq: u64,
    pub expires: u64,
    pub cores: Vec<StateCore>,
}

/// A friend.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Friend {
    /// Their public key, 64 hex digits: their id.
    pub key: String,
    /// The name they chose (cleaned).
    pub name: String,
    /// The name this person gave them, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    /// The pair secret, 64 hex digits.
    pub pair: String,
    /// The two compared safety codes.
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub status: FriendStatus,
    /// Their latest state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<FriendState>,
    /// The number of this person's state last written to their slot.
    #[serde(default)]
    pub sent_seq: u64,
    /// Cores of this person's stop publishing for them.
    #[serde(default)]
    pub hidden: bool,
    /// Seconds since 1970.
    #[serde(default)]
    pub added_at: u64,
}

impl std::fmt::Debug for Friend {
    /// Leaves out the pair secret.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Friend")
            .field("key", &self.key)
            .field("name", &self.name)
            .field("nickname", &self.nickname)
            .field("verified", &self.verified)
            .field("status", &self.status)
            .field("state", &self.state)
            .field("sent_seq", &self.sent_seq)
            .field("hidden", &self.hidden)
            .finish_non_exhaustive()
    }
}

impl Friend {
    pub fn public_key(&self) -> Option<PublicKey> {
        PublicKey::from_hex(&self.key)
    }

    pub fn pair_secret(&self) -> Option<FriendSecret> {
        FriendSecret::from_hex(&self.pair)
    }

    /// What the list shows: the nickname, else their name.
    pub fn display_name(&self) -> &str {
        self.nickname.as_deref().unwrap_or(&self.name)
    }

    pub fn safety_code(&self) -> String {
        self.public_key()
            .map(|key| key.safety_code())
            .unwrap_or_default()
    }

    /// The cores of theirs that can be joined now: from a state that hasn't expired.
    pub fn joinable_cores(&self, now: u64) -> &[StateCore] {
        match (&self.status, &self.state) {
            (FriendStatus::Active, Some(state)) if now < state.expires => &state.cores,
            _ => &[],
        }
    }
}

/// A core this person shares from, as their state lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MyCore {
    /// SHA-256 of the core's friend certificate, 64 hex digits.
    pub fingerprint: String,
    pub name: String,
}

/// A friend link this person made, waiting for answers.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingLink {
    /// The mailbox code.
    pub code: String,
    /// [`lodestone_connect::friend::pair::Invitation::to_json`]: holds the link's
    /// secret and the exchange key.
    pub invitation: String,
    pub expires: u64,
}

impl std::fmt::Debug for PendingLink {
    /// Leaves out the invitation, which holds the link's secret and exchange key.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PendingLink")
            .field("code", &self.code)
            .field("expires", &self.expires)
            .finish_non_exhaustive()
    }
}

/// Everything in the friend file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Book {
    /// The name friends see.
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub friends: Vec<Friend>,
    #[serde(default)]
    pub links: Vec<PendingLink>,
    #[serde(default)]
    pub my_cores: Vec<MyCore>,
    /// The number of this person's current state.
    #[serde(default)]
    pub state_seq: u64,
    /// When it was issued.
    #[serde(default)]
    pub state_issued: u64,
}

impl Book {
    pub fn friend(&self, key: &str) -> Option<&Friend> {
        self.friends.iter().find(|f| f.key == key)
    }

    pub fn friend_mut(&mut self, key: &str) -> Option<&mut Friend> {
        self.friends.iter_mut().find(|f| f.key == key)
    }

    /// Other friends whose name or nickname is `name`: a new friend with the same name
    /// as an old one is flagged, in case someone is pretending.
    pub fn same_name(&self, name: &str, except: &str) -> Vec<&Friend> {
        // compared as they look, so "Аna" in Cyrillic matches "Ana"
        let name = name_skeleton(name);
        self.friends
            .iter()
            .filter(|f| f.key != except)
            .filter(|f| {
                name_skeleton(&f.name) == name
                    || f.nickname
                        .as_deref()
                        .is_some_and(|n| name_skeleton(n) == name)
            })
            .collect()
    }

    /// Starts a new state: a higher number (never lower than the time, so a state made
    /// after the file was lost still counts as newer), issued when next published.
    pub(crate) fn new_state(&mut self) {
        self.state_seq = (self.state_seq + 1).max(now());
        self.state_issued = 0;
    }

    /// This person's cores, as a state lists them.
    pub fn state_cores(&self) -> Vec<StateCore> {
        self.my_cores
            .iter()
            .filter_map(|core| {
                Some(StateCore {
                    fingerprint: crate_unhex32(&core.fingerprint)?,
                    name: clean_name(&core.name),
                })
            })
            .collect()
    }
}

pub(crate) fn crate_unhex32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Serialize, Deserialize)]
struct FileWire {
    v: u32,
    sealed: String,
}

/// Why the friend list couldn't be opened or saved.
#[derive(Debug, thiserror::Error)]
pub enum FriendsError {
    #[error("Couldn't read the Keychain: {0}")]
    Keychain(String),
    #[error("The friend list is damaged: {0}")]
    Damaged(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("{0}")]
    Connect(String),
    #[error("{0}")]
    Refused(String),
}

impl From<lodestone_connect::Error> for FriendsError {
    fn from(err: lodestone_connect::Error) -> Self {
        FriendsError::Connect(err.to_string())
    }
}

/// The friend file at `path`, opened with `key`. `Book::default()` if there's none.
pub fn read_book(path: &Path, key: &FriendSecret) -> Result<Book, FriendsError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Book::default()),
        Err(err) => return Err(err.into()),
    };
    let wire: FileWire =
        serde_json::from_str(&text).map_err(|err| FriendsError::Damaged(err.to_string()))?;
    if wire.v != FILE_VERSION {
        return Err(FriendsError::Damaged("made by a newer launcher".into()));
    }
    let sealed =
        lodestone_unhex(&wire.sealed).ok_or_else(|| FriendsError::Damaged("not hex".into()))?;
    let plain = crypto::open(key, Role::File, FILE_NAME.as_bytes(), &sealed).ok_or_else(|| {
        FriendsError::Damaged("it couldn't be opened with this computer's key".into())
    })?;
    serde_json::from_slice(&plain).map_err(|err| FriendsError::Damaged(err.to_string()))
}

/// Seals and saves `book` at `path`, readable only by this user.
pub fn write_book(path: &Path, key: &FriendSecret, book: &Book) -> Result<(), FriendsError> {
    let plain = serde_json::to_vec(book).expect("serializable");
    let sealed = crypto::seal(key, Role::File, FILE_NAME.as_bytes(), &plain)?;
    let text = serde_json::to_string(&FileWire {
        v: FILE_VERSION,
        sealed: hex(&sealed),
    })
    .expect("serializable");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    storage::write_atomic(path, text.as_bytes())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn lodestone_unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

/// The Keychain item `base` for the friend list in `data`: `base` itself for the data
/// folder this user's launcher really uses (the default one, or wherever they moved it
/// in Settings › Storage), and `base@<8 hex digits of the folder's SHA-256>` for any
/// other (`LODESTONE_LAUNCHER_DATA_DIR`, tests, dev bundles), so those never make or use
/// this person's real friend identity. Moving the data folder keeps the identity.
pub fn secret_name(base: &str, data: &DataDir) -> String {
    let overridden =
        std::env::var_os("LODESTONE_LAUNCHER_DATA_DIR").is_some_and(|dir| !dir.is_empty());
    let real = !overridden
        && crate::paths::locate_data_dir().is_some_and(|location| location.dir == data.root());
    if real {
        return base.to_string();
    }
    let digest = crypto::sha256(data.root().to_string_lossy().as_bytes());
    format!("{base}@{}", hex(&digest[..4]))
}

/// Reads a hex secret from the Keychain, making and storing a new one if it's missing.
async fn keychain_bytes(
    store: &dyn SecretStore,
    name: &str,
    make: impl FnOnce() -> Result<Vec<u8>, FriendsError>,
) -> Result<Vec<u8>, FriendsError> {
    let keychain = |err: anyhow::Error| FriendsError::Keychain(err.to_string());
    if let Some(stored) = store.read(name).await.map_err(keychain)? {
        return lodestone_unhex(stored.expose())
            .ok_or_else(|| FriendsError::Damaged(format!("the Keychain item {name} isn't hex")));
    }
    let made = make()?;
    store
        .write(name, &Secret::new(hex(&made)))
        .await
        .map_err(keychain)?;
    Ok(made)
}

/// The friend list: the book, the person key and the service.
pub struct Friends {
    path: PathBuf,
    file_key: FriendSecret,
    me: PersonKey,
    service: Service,
    pub book: Book,
}

impl std::fmt::Debug for Friends {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Friends")
            .field("me", &self.me)
            .field("friends", &self.book.friends.len())
            .finish_non_exhaustive()
    }
}

impl Friends {
    /// Whether this person has a friend list in `data` yet. Check it before [`Friends::open`]
    /// in an app that should only use one the Launcher made (the Host): opening makes a
    /// person key.
    pub fn exists(data: &DataDir) -> bool {
        data.root().join(FILE_NAME).exists()
    }

    /// Opens the friend list in `data`, making the person key and the file key in the
    /// Keychain on first use.
    pub async fn open(
        data: &DataDir,
        store: &dyn SecretStore,
        service: Service,
    ) -> Result<Self, FriendsError> {
        let pkcs8 = keychain_bytes(store, &secret_name(IDENTITY_SECRET, data), || {
            Ok(PersonKey::generate()?.pkcs8().to_vec())
        })
        .await?;
        let me = PersonKey::from_pkcs8(&pkcs8)?;
        let file_key = keychain_bytes(store, &secret_name(FILE_KEY_SECRET, data), || {
            Ok(FriendSecret::random()?.as_bytes().to_vec())
        })
        .await?;
        let file_key = FriendSecret::from_bytes(
            file_key
                .try_into()
                .map_err(|_| FriendsError::Damaged("the file key isn't 32 bytes".into()))?,
        );
        let path = data.root().join(FILE_NAME);
        let book = read_book(&path, &file_key)?;
        Ok(Self {
            path,
            file_key,
            me,
            service,
            book,
        })
    }

    /// This person's key.
    pub fn me(&self) -> PublicKey {
        self.me.public()
    }

    pub fn person_key(&self) -> &PersonKey {
        &self.me
    }

    pub fn service(&self) -> &Service {
        &self.service
    }

    /// This person's safety code, for friends to compare.
    pub fn safety_code(&self) -> String {
        self.me.public().safety_code()
    }

    /// Reads the file again (the other app on this computer may have changed it).
    pub fn reload(&mut self) -> Result<(), FriendsError> {
        self.book = read_book(&self.path, &self.file_key)?;
        Ok(())
    }

    /// Changes the book: under a lock on the file, reads it fresh from disk, applies
    /// `change` and writes it. The Launcher and the Host share the file, so a change is
    /// never made to a copy the other app has since changed (which could bring back a
    /// friend it removed, or roll a state number back).
    pub(crate) fn modify<R>(
        &mut self,
        change: impl FnOnce(&mut Book) -> Result<R, FriendsError>,
    ) -> Result<R, FriendsError> {
        let _lock = FileLock::acquire(&self.path)?;
        self.book = read_book(&self.path, &self.file_key)?;
        let result = change(&mut self.book)?;
        write_book(&self.path, &self.file_key, &self.book)?;
        Ok(result)
    }

    /// Sets the name friends see. It reaches them with the next state.
    pub fn set_name(&mut self, name: &str) -> Result<(), FriendsError> {
        let name = clean_name(name);
        if name.is_empty() {
            return Err(FriendsError::Refused(
                "Choose a name your friends will see.".into(),
            ));
        }
        self.modify(|book| {
            if name != book.name {
                book.name = name;
                book.new_state();
            }
            Ok(())
        })
    }

    /// Ticks or unticks "Codes match" for a friend.
    pub fn set_verified(&mut self, key: &str, verified: bool) -> Result<(), FriendsError> {
        self.modify(|book| {
            if let Some(friend) = book.friend_mut(key) {
                friend.verified = verified;
            }
            Ok(())
        })
    }

    pub fn set_nickname(&mut self, key: &str, nickname: Option<&str>) -> Result<(), FriendsError> {
        let nickname = nickname.map(clean_name).filter(|n| !n.is_empty());
        self.modify(|book| {
            if let Some(friend) = book.friend_mut(key) {
                friend.nickname = nickname;
            }
            Ok(())
        })
    }

    /// Hides from a friend: this person's cores stop publishing for them (Host applies
    /// it). They stay friends.
    pub fn set_hidden(&mut self, key: &str, hidden: bool) -> Result<(), FriendsError> {
        self.modify(|book| {
            if let Some(friend) = book.friend_mut(key) {
                friend.hidden = hidden;
            }
            Ok(())
        })
    }

    /// Adds or renames a core of this person's in their state. Returns whether the state
    /// changed (publish it with [`Friends::publish_states`]).
    pub fn add_my_core(&mut self, fingerprint: &str, name: &str) -> Result<bool, FriendsError> {
        if crate_unhex32(fingerprint).is_none() {
            return Err(FriendsError::Damaged("a core's fingerprint".into()));
        }
        let name = clean_name(name);
        self.modify(|book| {
            let changed = match book
                .my_cores
                .iter_mut()
                .find(|c| c.fingerprint == fingerprint)
            {
                Some(core) if core.name == name => false,
                Some(core) => {
                    core.name = name;
                    true
                }
                None => {
                    book.my_cores.push(MyCore {
                        fingerprint: fingerprint.to_string(),
                        name,
                    });
                    true
                }
            };
            if changed {
                book.new_state();
            }
            Ok(changed)
        })
    }

    /// Retires a core of this person's: friends drop it and stop trusting its mods.
    pub fn remove_my_core(&mut self, fingerprint: &str) -> Result<bool, FriendsError> {
        self.modify(|book| {
            let before = book.my_cores.len();
            book.my_cores.retain(|c| c.fingerprint != fingerprint);
            let changed = book.my_cores.len() != before;
            if changed {
                book.new_state();
            }
            Ok(changed)
        })
    }
}

/// A lock on the friend file while one app changes it: a `.lock` file next to it, made
/// with create-new. A lock older than [`STALE_LOCK`] is from an app that died, and is
/// taken over.
struct FileLock {
    path: PathBuf,
}

const STALE_LOCK: Duration = Duration::from_secs(30);

impl FileLock {
    fn acquire(file: &Path) -> Result<Self, FriendsError> {
        let path = file.with_extension("lock");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(_) => return Ok(Self { path }),
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                    let stale = std::fs::metadata(&path)
                        .and_then(|meta| meta.modified())
                        .ok()
                        .and_then(|at| at.elapsed().ok())
                        .is_some_and(|age| age > STALE_LOCK);
                    if stale {
                        let _ = std::fs::remove_file(&path);
                        continue;
                    }
                    if std::time::Instant::now() >= deadline {
                        return Err(FriendsError::Refused(
                            "The friend list is busy in Lodestone's other app. Try again.".into(),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(err) => return Err(err.into()),
            }
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Runs `make()`'s future on a thread of its own with a small Tokio runtime, and waits
/// for it on the caller's executor. lodestone_connect needs Tokio; the apps don't run it.
pub async fn on_tokio<T, F, Fut>(make: F) -> Result<T, FriendsError>
where
    T: Send + 'static,
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = T>,
{
    let (tx, rx) = oneshot::channel();
    std::thread::Builder::new()
        .name("lodestone-friends".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(_) => return,
            };
            let _ = tx.send(runtime.block_on(make()));
        })
        .map_err(FriendsError::Io)?;
    rx.await
        .map_err(|_| FriendsError::Connect("the friend list's network thread stopped".into()))
}

/// How long a single service call may take, overall.
pub const CALL_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(test)]
mod tests;
