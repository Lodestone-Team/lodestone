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
//! tunnels do; the Keychain is read and written on the caller's executor. File work
//! (reading, sealing, the lock) is plain blocking code: apps run it off their UI thread.
//!
//! **Who makes the friend list.** Only the Launcher starts one ([`Friends::create`]),
//! when the person first does something that needs it (choosing a name, making or
//! answering a friend link). Everything else opens the list that is there
//! ([`Friends::open_existing`]), which never makes a key or a file. A friend file that
//! goes missing (retired, deleted, or moved with the data folder) stops changes: it
//! never reads as an empty list, and [`Friends::modify`] never writes a new one.

pub mod core_sync;
/// A Minecraft account in a person's state, for friends' whitelists.
pub use lodestone_connect::friend::state::StatePlayer;
pub mod ops;

use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures::channel::oneshot;
use lodestone_connect::{
    Service,
    friend::{
        Secret as FriendSecret,
        crypto::{self, PersonKey, PublicKey, Role},
        pair::{clean_name, name_skeleton},
        state::{MAX_PLAYERS, StateCore},
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

/// The service the friend list uses, read each time it is needed:
/// `LODESTONE_CONNECT_SERVICE`, else the app's developer setting, else
/// connect.lodestone.cc ([`crate::cores::connect_service`]). Invite links carry their
/// own service.
pub fn service() -> Service {
    crate::cores::connect_service::service()
}

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
    /// Their Minecraft accounts, for a server's whitelist (from Launchers that send
    /// them).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub players: Vec<StatePlayer>,
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
    /// Cores of this person's stop publishing for them, and serve them nothing. The
    /// cores keep what's shared with them, for when they're shown again.
    #[serde(default)]
    pub hidden: bool,
    /// Seconds since 1970.
    #[serde(default)]
    pub added_at: u64,
    /// The Minecraft profile their card or answer said they play as, when they became
    /// friends (from launchers that send it). Their states' accounts take over once
    /// one arrives: see [`Friend::minecraft`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player: Option<StatePlayer>,
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
            .field("player", &self.player)
            .finish_non_exhaustive()
    }
}

/// The first Microsoft (not offline) account among `players`: the profile a person
/// shows friends, whose skin is drawn beside their name.
pub fn profile_of(players: &[StatePlayer]) -> Option<&StatePlayer> {
    players.iter().find(|p| !p.offline)
}

impl Friend {
    /// The Minecraft profile they say they play as, for their skin: their latest
    /// state's first Microsoft account once a state arrived, else what they sent when
    /// you became friends. Their launcher's claim: nobody checks that the account is
    /// theirs, so it's never shown as proof of who they are.
    pub fn minecraft(&self) -> Option<&StatePlayer> {
        match &self.state {
            Some(state) => profile_of(&state.players),
            None => self.player.as_ref().filter(|p| !p.offline),
        }
    }

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
    /// Which core this is, as the app that added it names it (Lodestone Host: its
    /// core's data folder). When that core comes back with another certificate, the
    /// old one is dropped. `None` for a core added without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
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
    /// This person's Minecraft accounts, sent to friends so their Hosts can whitelist
    /// them (see [`Friends::set_players`]). The first Microsoft one is their profile
    /// ([`Book::profile`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub players: Vec<StatePlayer>,
    /// `name` is the profile's player name, and follows it: the person never chose
    /// one ([`Friends::set_players`]). False once they do ([`Friends::set_name`]), and
    /// in lists from before profiles, whose names were all chosen.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub name_from_profile: bool,
    /// The number of this person's current state.
    #[serde(default)]
    pub state_seq: u64,
    /// When it was issued.
    #[serde(default)]
    pub state_issued: u64,
    /// Cores this person retired (fingerprints): never listed again, even if a core
    /// with that certificate syncs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub retired_cores: Vec<String>,
    /// This identity was retired: its last state said so, and nothing uses it again.
    /// The Launcher deletes the file when the person starts a new identity.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retired: bool,
    /// Goes up by one each time the file is written, so of two copies of the book the
    /// one with the higher number is newer (a modification time can't tell on file
    /// systems that keep whole seconds).
    #[serde(default)]
    pub generation: u64,
}

impl Book {
    pub fn friend(&self, key: &str) -> Option<&Friend> {
        self.friends.iter().find(|f| f.key == key)
    }

    pub fn friend_mut(&mut self, key: &str) -> Option<&mut Friend> {
        self.friends.iter_mut().find(|f| f.key == key)
    }

    /// The Minecraft profile this person shows friends: their first Microsoft
    /// account, if they have one.
    pub fn profile(&self) -> Option<&StatePlayer> {
        profile_of(&self.players)
    }

    /// Other friends whose name or nickname is `name`: a new friend with the same name
    /// as an old one is flagged, in case someone is pretending.
    pub fn same_name(&self, name: &str, except: &str) -> Vec<&Friend> {
        self.lookalikes(name, None, except)
    }

    /// Other friends who look like someone called `name` playing as `player`: whose
    /// name, nickname or Minecraft name looks like `name` or the player's name, or who
    /// play as the same Minecraft profile. Anyone can name any profile, and its skin
    /// would make them look like its owner, so it's checked as names are.
    pub fn lookalikes(
        &self,
        name: &str,
        player: Option<&StatePlayer>,
        except: &str,
    ) -> Vec<&Friend> {
        let looks = Looks::new(name, player);
        self.friends
            .iter()
            .filter(|f| f.key != except)
            .filter(|f| looks.matches(&Looks::of(f)))
            .collect()
    }

    /// For each friend whose chosen name (or Minecraft profile) looks like another
    /// friend's: those friends' names, as the list shows them. Checked again whenever
    /// a friend renames themselves, so a friend can't take on another's name
    /// unnoticed.
    pub fn name_clashes(&self) -> BTreeMap<String, Vec<String>> {
        let looks: Vec<Looks> = self.friends.iter().map(Looks::of).collect();
        let mut out = BTreeMap::new();
        for (i, friend) in self.friends.iter().enumerate() {
            let names: Vec<String> = self
                .friends
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i && looks[i].matches(&looks[*j]))
                .map(|(_, f)| f.display_name().to_string())
                .collect();
            if !names.is_empty() {
                out.insert(friend.key.clone(), names);
            }
        }
        out
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
                    fingerprint: unhex32(&core.fingerprint)?,
                    name: clean_name(&core.name),
                })
            })
            .collect()
    }
}

/// How a friend looks in the list, for warnings about look-alikes: the skeletons
/// ([`name_skeleton`]) of their name, their Minecraft name and the nickname given to
/// them, and their Minecraft profile's UUID.
struct Looks {
    /// Their own: the name they chose, and their Minecraft name.
    names: Vec<String>,
    nickname: Option<String>,
    uuid: Option<[u8; 16]>,
}

impl Looks {
    fn new(name: &str, player: Option<&StatePlayer>) -> Self {
        let player = player.filter(|p| !p.offline);
        let mut names = vec![name_skeleton(name)];
        names.extend(player.map(|p| name_skeleton(&p.name)));
        names.retain(|n| !n.is_empty());
        names.dedup();
        Self {
            names,
            nickname: None,
            uuid: player.map(|p| p.uuid),
        }
    }

    fn of(friend: &Friend) -> Self {
        Self {
            nickname: friend
                .nickname
                .as_deref()
                .map(name_skeleton)
                .filter(|n| !n.is_empty()),
            ..Self::new(&friend.name, friend.minecraft())
        }
    }

    /// Whether someone who looks like `self` could be taken for `other`: one of
    /// `self`'s names is one of `other`'s or the nickname `other` was given, or both
    /// play as the same profile.
    fn matches(&self, other: &Looks) -> bool {
        self.uuid.is_some() && self.uuid == other.uuid
            || self
                .names
                .iter()
                .any(|n| other.names.contains(n) || other.nickname.as_ref() == Some(n))
    }
}

/// 64 hex digits as 32 bytes.
pub fn unhex32(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

/// Lower-case hex.
pub fn hex(bytes: &[u8]) -> String {
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
    /// The friend file isn't there any more (retired and started again, deleted, or
    /// moved with the data folder). Stop, and open the list again.
    #[error("The friend list isn't there any more. Open it again.")]
    Missing,
    /// The identity was retired: nothing uses it again.
    #[error("Your friend identity was retired. Start again with a new name.")]
    Retired,
    /// A core's friend certificate was retired: it's never listed again. The core
    /// needs a new certificate ([`core_sync::new_certificate`]) to share.
    #[error(
        "This core's friend certificate was retired, so friends can't be shown it again. Give the core a new friend certificate to share from it."
    )]
    CoreRetired,
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

impl FriendsError {
    /// Whether the list in memory is no longer the one on disk (it went missing, or the
    /// file there needs other keys): drop it and open the list again.
    pub fn is_stale(&self) -> bool {
        matches!(
            self,
            FriendsError::Missing | FriendsError::Retired | FriendsError::Damaged(_)
        )
    }
}

/// The friend file at `path`, opened with `key`. [`FriendsError::Missing`] if there's
/// none: a missing file is never an empty friend list.
pub fn read_book(path: &Path, key: &FriendSecret) -> Result<Book, FriendsError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Err(FriendsError::Missing),
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

/// SHA-256 of the friend file's bytes as they are on disk, `None` without one. Every
/// write seals with a new nonce, so this changes whenever the file is written, even
/// within the same second. For an app without the keys to tell whether the file
/// changed (to open it again only then).
pub fn file_stamp(data: &DataDir) -> Option<[u8; 32]> {
    let bytes = std::fs::read(data.root().join(FILE_NAME)).ok()?;
    Some(crypto::sha256(&bytes))
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
    if is_real_data_dir(data) {
        return base.to_string();
    }
    let digest = crypto::sha256(data.root().to_string_lossy().as_bytes());
    format!("{base}@{}", hex(&digest[..4]))
}

/// Whether `data` is the data folder this user's launcher really uses (not one named
/// by `LODESTONE_LAUNCHER_DATA_DIR`, a test's or a dev bundle's).
pub fn is_real_data_dir(data: &DataDir) -> bool {
    let overridden =
        std::env::var_os("LODESTONE_LAUNCHER_DATA_DIR").is_some_and(|dir| !dir.is_empty());
    !overridden
        && crate::paths::locate_data_dir().is_some_and(|location| location.dir == data.root())
}

/// The friend list's two Keychain items, as read: either may be missing.
#[derive(Default)]
pub struct StoredKeys {
    identity: Option<Vec<u8>>,
    file_key: Option<Vec<u8>>,
}

impl std::fmt::Debug for StoredKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoredKeys")
            .field("identity", &self.identity.is_some())
            .field("file_key", &self.file_key.is_some())
            .finish()
    }
}

fn keychain_error(err: anyhow::Error) -> FriendsError {
    FriendsError::Keychain(err.to_string())
}

async fn read_hex(store: &dyn SecretStore, name: &str) -> Result<Option<Vec<u8>>, FriendsError> {
    match store.read(name).await.map_err(keychain_error)? {
        Some(stored) => lodestone_unhex(stored.expose())
            .map(Some)
            .ok_or_else(|| FriendsError::Damaged(format!("the Keychain item {name} isn't hex"))),
        None => Ok(None),
    }
}

/// Reads the friend list's Keychain items for `data`. Makes nothing.
pub async fn read_keys(
    data: &DataDir,
    store: &dyn SecretStore,
) -> Result<StoredKeys, FriendsError> {
    Ok(StoredKeys {
        identity: read_hex(store, &secret_name(IDENTITY_SECRET, data)).await?,
        file_key: read_hex(store, &secret_name(FILE_KEY_SECRET, data)).await?,
    })
}

/// Makes the Keychain items missing from `keys` and stores them: a new person key, a
/// new file key. Only for starting a friend list ([`Friends::create`]).
pub async fn make_keys(
    data: &DataDir,
    store: &dyn SecretStore,
    mut keys: StoredKeys,
) -> Result<StoredKeys, FriendsError> {
    if keys.identity.is_none() {
        let made = PersonKey::generate()?.pkcs8().to_vec();
        store
            .write(
                &secret_name(IDENTITY_SECRET, data),
                &Secret::new(hex(&made)),
            )
            .await
            .map_err(keychain_error)?;
        keys.identity = Some(made);
    }
    if keys.file_key.is_none() {
        let made = FriendSecret::random()?.as_bytes().to_vec();
        store
            .write(
                &secret_name(FILE_KEY_SECRET, data),
                &Secret::new(hex(&made)),
            )
            .await
            .map_err(keychain_error)?;
        keys.file_key = Some(made);
    }
    Ok(keys)
}

/// Deletes the person key from the Keychain: the last step of retiring it.
pub async fn forget_identity(data: &DataDir, store: &dyn SecretStore) -> Result<(), FriendsError> {
    store
        .delete(&secret_name(IDENTITY_SECRET, data))
        .await
        .map_err(keychain_error)
}

/// Deletes both Keychain items (a retired list's, before starting a new one).
pub async fn forget_keys(data: &DataDir, store: &dyn SecretStore) -> Result<(), FriendsError> {
    forget_identity(data, store).await?;
    store
        .delete(&secret_name(FILE_KEY_SECRET, data))
        .await
        .map_err(keychain_error)
}

/// Deletes the friend file in `data` under its lock, so no app is in the middle of
/// changing it (a change that read the file before would otherwise write it back).
pub fn remove_file(data: &DataDir) -> Result<(), FriendsError> {
    let path = data.root().join(FILE_NAME);
    let _lock = FileLock::acquire(&path)?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

fn file_key_of(keys: &StoredKeys) -> Result<FriendSecret, FriendsError> {
    let bytes = keys.file_key.clone().ok_or_else(|| {
        FriendsError::Damaged("the Keychain has no key for it on this computer".into())
    })?;
    Ok(FriendSecret::from_bytes(bytes.try_into().map_err(
        |_| FriendsError::Damaged("the file key isn't 32 bytes".into()),
    )?))
}

fn person_key_of(keys: &StoredKeys) -> Result<PersonKey, FriendsError> {
    let pkcs8 = keys.identity.as_ref().ok_or_else(|| {
        FriendsError::Damaged("the Keychain has no friend identity for it on this computer".into())
    })?;
    Ok(PersonKey::from_pkcs8(pkcs8)?)
}

/// What [`Friends::open_existing`] found.
#[derive(Debug)]
pub enum Opened {
    /// There's no friend list in this data folder (yet).
    None,
    /// The list's identity was retired. Nothing uses it; the Launcher starts a new one.
    Retired,
    Ready(Box<Friends>),
}

/// The friend list: the book, the person key and the service. Cloning it gives another
/// handle on the same file: every change reloads the file under its lock first, so
/// copies (and the other app) never undo each other's changes.
#[derive(Clone)]
pub struct Friends {
    path: PathBuf,
    file_key: FriendSecret,
    me: Arc<PersonKey>,
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
    /// Whether this person has a friend list in `data` yet.
    pub fn exists(data: &DataDir) -> bool {
        data.root().join(FILE_NAME).exists()
    }

    /// Opens the friend list in `data` with `keys` (from [`read_keys`]): blocking file
    /// work, which makes nothing.
    pub fn open_with(
        data: &DataDir,
        keys: &StoredKeys,
        service: Service,
    ) -> Result<Opened, FriendsError> {
        let path = data.root().join(FILE_NAME);
        if !path.exists() {
            return Ok(Opened::None);
        }
        let file_key = file_key_of(keys)?;
        let book = match read_book(&path, &file_key) {
            Ok(book) => book,
            // deleted since `exists`
            Err(FriendsError::Missing) => return Ok(Opened::None),
            Err(err) => return Err(err),
        };
        if book.retired {
            return Ok(Opened::Retired);
        }
        Ok(Opened::Ready(Box::new(Self {
            path,
            file_key,
            me: Arc::new(person_key_of(keys)?),
            service,
            book,
        })))
    }

    /// Opens the friend list in `data`, if there is one. Never makes a key or a file:
    /// for Lodestone Host, and for anything that only looks.
    pub async fn open_existing(
        data: &DataDir,
        store: &dyn SecretStore,
        service: Service,
    ) -> Result<Opened, FriendsError> {
        if !Self::exists(data) {
            return Ok(Opened::None);
        }
        let keys = read_keys(data, store).await?;
        Self::open_with(data, &keys, service)
    }

    /// Starts the friend list in `data` with `keys` (complete, from [`make_keys`]):
    /// under the file's lock, writes an empty book unless there is a file by now (then
    /// opens that one). Blocking file work.
    pub fn start_with(
        data: &DataDir,
        keys: &StoredKeys,
        service: Service,
    ) -> Result<Self, FriendsError> {
        let path = data.root().join(FILE_NAME);
        let file_key = file_key_of(keys)?;
        let me = Arc::new(person_key_of(keys)?);
        let _lock = FileLock::acquire(&path)?;
        let book = match read_book(&path, &file_key) {
            Ok(book) if book.retired => return Err(FriendsError::Retired),
            Ok(book) => book,
            Err(FriendsError::Missing) => {
                let book = Book {
                    generation: 1,
                    ..Book::default()
                };
                write_book(&path, &file_key, &book)?;
                book
            }
            Err(err) => return Err(err),
        };
        Ok(Self {
            path,
            file_key,
            me,
            service,
            book,
        })
    }

    /// Opens the friend list in `data`, or starts one: the Launcher, when the person
    /// does something that needs it. A retired list is deleted first (the file, then
    /// its Keychain items), and a new identity made. A damaged one is left alone.
    pub async fn create(
        data: &DataDir,
        store: &dyn SecretStore,
        service: Service,
    ) -> Result<Self, FriendsError> {
        let keys = read_keys(data, store).await?;
        let keys = match Self::open_with(data, &keys, service.clone())? {
            Opened::Ready(friends) => return Ok(*friends),
            Opened::None => keys,
            Opened::Retired => {
                remove_file(data)?;
                forget_keys(data, store).await?;
                StoredKeys::default()
            }
        };
        let keys = make_keys(data, store, keys).await?;
        Self::start_with(data, &keys, service)
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
    /// [`FriendsError::Missing`] when it's gone, [`FriendsError::Retired`] when the
    /// identity was retired meanwhile.
    pub fn reload(&mut self) -> Result<(), FriendsError> {
        let book = read_book(&self.path, &self.file_key)?;
        let retired = book.retired;
        self.book = book;
        if retired {
            return Err(FriendsError::Retired);
        }
        Ok(())
    }

    /// Changes the book: under a lock on the file, reads it fresh from disk, applies
    /// `change` and writes it, if it changed. The Launcher and the Host share the file,
    /// so a change is never made to a copy the other app has since changed (which could
    /// bring back a friend it removed, or roll a state number back).
    ///
    /// The file must be there: a missing one is [`FriendsError::Missing`], never
    /// written anew (that would seal a list with keys the Keychain may no longer
    /// have). A retired identity's book is [`FriendsError::Retired`].
    pub(crate) fn modify<R>(
        &mut self,
        change: impl FnOnce(&mut Book) -> Result<R, FriendsError>,
    ) -> Result<R, FriendsError> {
        self.modify_book(false, change)
    }

    /// [`Friends::modify`], also on a retired identity's book when `retired_too`.
    pub(crate) fn modify_book<R>(
        &mut self,
        retired_too: bool,
        change: impl FnOnce(&mut Book) -> Result<R, FriendsError>,
    ) -> Result<R, FriendsError> {
        let _lock = FileLock::acquire(&self.path)?;
        let book = read_book(&self.path, &self.file_key)?;
        let retired = book.retired;
        self.book = book;
        if retired && !retired_too {
            return Err(FriendsError::Retired);
        }
        let mut next = self.book.clone();
        let result = change(&mut next)?;
        if next != self.book {
            next.generation = self.book.generation + 1;
            write_book(&self.path, &self.file_key, &next)?;
            self.book = next;
        }
        Ok(result)
    }

    /// Sets the name friends see, chosen by the person: it no longer follows their
    /// Minecraft profile's. It reaches friends with the next state.
    pub fn set_name(&mut self, name: &str) -> Result<(), FriendsError> {
        let name = clean_name(name);
        if name.is_empty() {
            return Err(FriendsError::Refused(
                "Choose a name your friends will see.".into(),
            ));
        }
        self.modify(|book| {
            book.name_from_profile = false;
            if name != book.name {
                book.name = name;
                book.new_state();
            }
            Ok(())
        })
    }

    /// Goes back to the Minecraft profile's player name, and follows it from now on.
    pub fn use_profile_name(&mut self) -> Result<(), FriendsError> {
        let Some(profile) = self.book.profile().cloned() else {
            return Err(FriendsError::Refused(
                "Sign in with a Microsoft account to use your Minecraft name.".into(),
            ));
        };
        self.modify(|book| {
            book.name_from_profile = true;
            if profile.name != book.name {
                book.name = profile.name;
                book.new_state();
            }
            Ok(())
        })
    }

    /// Sets the Minecraft accounts friends see (for their servers' whitelists, and the
    /// first Microsoft one for the skin beside this person's name): the first
    /// [`MAX_PLAYERS`] with a name a player can have. Until the person chooses a name
    /// of their own, their name is that profile's player name. Returns whether anything
    /// changed; a change makes a new state, which goes out at the next publish.
    pub fn set_players(&mut self, players: Vec<StatePlayer>) -> Result<bool, FriendsError> {
        let players: Vec<StatePlayer> = players
            .into_iter()
            .filter(|p| StatePlayer::is_name(&p.name))
            .take(MAX_PLAYERS)
            .collect();
        let apply = |book: &mut Book| {
            let mut changed = false;
            if players != book.players {
                book.players = players.clone();
                changed = true;
            }
            if book.name.is_empty() || book.name_from_profile {
                // the Minecraft name, until they choose another; one that goes (signed
                // out) leaves the name as it was
                if let Some(profile) = profile_of(&players) {
                    if book.name != profile.name {
                        book.name = profile.name.clone();
                        changed = true;
                    }
                    book.name_from_profile = true;
                }
            }
            if changed {
                book.new_state();
            }
            changed
        };
        if !apply(&mut self.book.clone()) {
            return Ok(false);
        }
        self.modify(|book| Ok(apply(book)))
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

    /// Hides from a friend: this person's cores stop publishing for them and serve them
    /// nothing, but keep what's shared with them (Host applies it). They stay friends.
    pub fn set_hidden(&mut self, key: &str, hidden: bool) -> Result<(), FriendsError> {
        self.modify(|book| {
            if let Some(friend) = book.friend_mut(key) {
                friend.hidden = hidden;
            }
            Ok(())
        })
    }

    /// Adds or renames a core of this person's in their state. `source` names which
    /// core it is (see [`MyCore::source`]): another certificate from the same core
    /// replaces the old one, which friends then drop. A retired certificate is refused.
    /// Returns whether the state changed (publish it with [`Friends::publish_states`]).
    pub fn add_my_core(
        &mut self,
        fingerprint: &str,
        name: &str,
        source: Option<&str>,
    ) -> Result<bool, FriendsError> {
        if unhex32(fingerprint).is_none() {
            return Err(FriendsError::Damaged("a core's fingerprint".into()));
        }
        let name = clean_name(name);
        self.modify(|book| {
            if book.retired_cores.iter().any(|c| c == fingerprint) {
                return Err(FriendsError::CoreRetired);
            }
            let before = book.my_cores.len();
            // the same core with another certificate (its data was reset, say): the old
            // one goes
            if let Some(source) = source {
                book.my_cores.retain(|c| {
                    c.fingerprint == fingerprint || c.source.as_deref() != Some(source)
                });
            }
            let mut changed = book.my_cores.len() != before;
            match book
                .my_cores
                .iter_mut()
                .find(|c| c.fingerprint == fingerprint)
            {
                Some(core) => {
                    if core.name != name {
                        core.name = name;
                        changed = true;
                    }
                    if source.is_some() && core.source.as_deref() != source {
                        core.source = source.map(str::to_string);
                    }
                }
                None => {
                    book.my_cores.push(MyCore {
                        fingerprint: fingerprint.to_string(),
                        name,
                        source: source.map(str::to_string),
                    });
                    changed = true;
                }
            }
            if changed {
                book.new_state();
            }
            Ok(changed)
        })
    }

    /// Retires a core of this person's: it leaves their state (friends drop it and stop
    /// trusting its servers' mods), and is never listed again, even if it syncs. Returns
    /// whether the state changed (publish it with [`Friends::publish_states`]).
    pub fn retire_my_core(&mut self, fingerprint: &str) -> Result<bool, FriendsError> {
        let fingerprint = fingerprint.to_ascii_lowercase();
        self.modify(|book| {
            let before = book.my_cores.len();
            book.my_cores.retain(|c| c.fingerprint != fingerprint);
            let changed = book.my_cores.len() != before;
            if !book.retired_cores.contains(&fingerprint) {
                book.retired_cores.push(fingerprint.clone());
            }
            if changed {
                book.new_state();
            }
            Ok(changed)
        })
    }
}

/// A lock on the friend file while one app changes it: an operating system lock on
/// `friend_list.lock` next to it. The system lets go when the app holding it exits,
/// so a crashed app never leaves it locked; the file itself stays.
struct FileLock {
    _file: std::fs::File,
}

/// How long [`FileLock::acquire`] waits for the other app.
const LOCK_WAIT: Duration = Duration::from_secs(5);

impl FileLock {
    fn acquire(file: &Path) -> Result<Self, FriendsError> {
        let path = file.with_extension("lock");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let handle = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        let deadline = std::time::Instant::now() + LOCK_WAIT;
        loop {
            match handle.try_lock() {
                Ok(()) => return Ok(Self { _file: handle }),
                Err(std::fs::TryLockError::WouldBlock) => {
                    if std::time::Instant::now() >= deadline {
                        return Err(FriendsError::Refused(
                            "The friend list is busy in Lodestone's other app. Try again.".into(),
                        ));
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(std::fs::TryLockError::Error(err)) => return Err(err.into()),
            }
        }
    }
}

// Closing the file lets go of the lock.

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

#[cfg(test)]
mod tests;
