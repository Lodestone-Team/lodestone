//! Sharing with friends from the friend list (launcher/docs/friends.md): the core's
//! friend certificate, the list of friends its owner's launcher keeps here, and the
//! friend host that publishes records for them and serves their connections.
//!
//! - The certificate is `stores/connect/friends/core.json`; friends pin its fingerprint
//!   from the owner's signed state.
//! - The list is `stores/connect/friends/list.json`: each friend's public key, name, the
//!   core secret this core shares with them (never the pair secret) and the instances
//!   shared with them. The owner's launcher replaces it whole, with a higher version each
//!   time ([`FriendShares::replace`]).
//! - Only friends with something shared get a record, and a friend's streams reach only
//!   the instances shared with them, checked on every stream.

use std::{
    collections::{HashMap, HashSet},
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::{Arc, RwLock},
    time::Duration,
};

use axum::{
    extract::{connect_info::Connected, ConnectInfo, Path as UrlPath, State},
    http::HeaderMap,
    response::Response,
    routing::get,
    serve::IncomingStream,
    Json, Router,
};
use lodestone_connect::{
    friend::{
        crypto::{PublicKey, Secret},
        host::{
            CoreIdentity, FriendAccess, FriendHandler, FriendHost, FriendHostConfig,
            FriendHostStatus, KnockMemory,
        },
        pair::clean_name,
    },
    host::HostState,
    GatherOptions, Service, SharedServer, TunnelStream,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

use super::{game_port_open, shared_server_of, SHARED_SCOPE};
use crate::{
    error::{Error, ErrorKind},
    handlers::{
        public_instances::{pack_response, public_instance},
        util::get_instance,
    },
    public_listing::PublicInstanceList,
    traits::{
        t_configurable::{Game, TConfigurable},
        t_server::TServer,
    },
    types::InstanceUuid,
    AppState,
};
use color_eyre::eyre::eyre;

/// The folder in `stores/connect` with the friend certificate and list.
pub const FRIENDS_DIR: &str = "friends";
/// The most friends a core keeps.
pub const MAX_FRIENDS: usize = 256;
/// The most instances shared with one friend.
pub const MAX_SHARED: usize = 64;
/// The most friends with something shared. Each gets one or two records at the
/// service, which takes at most 256 from a core.
pub const MAX_SHARING_FRIENDS: usize = 120;
/// How often starting the friend host is tried again after it failed (offline).
const RETRY: Duration = Duration::from_secs(60);

fn bad_request(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!("{}", message.into()),
    }
}

fn internal(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::Internal,
        source: eyre!("{}", message.into()),
    }
}

// ---------------------------------------------------------------------------
// The list, as stored and as the API takes it

/// A friend, as the owner's launcher sends it and the file keeps it.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct FriendEntry {
    /// The friend's public key, 64 hex digits.
    pub key: String,
    /// The friend's name, as the owner sees it.
    pub name: String,
    /// The core secret this core shares with the friend, 64 hex digits.
    pub secret: String,
    /// The instances shared with the friend.
    #[serde(default)]
    pub shared: Vec<InstanceUuid>,
    /// Hidden from: the core keeps what's shared with them, but publishes nothing for
    /// them, refuses their connections and serves them nothing.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

impl std::fmt::Debug for FriendEntry {
    /// Leaves out the secret.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FriendEntry")
            .field("key", &self.key)
            .field("name", &self.name)
            .field("shared", &self.shared)
            .field("hidden", &self.hidden)
            .finish_non_exhaustive()
    }
}

/// `PUT /connect/friends`: the whole list.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct FriendList {
    /// Higher than the version the core has, each time.
    pub version: u64,
    pub friends: Vec<FriendEntry>,
}

/// A friend, as `GET /connect/friends` reports it: no secrets.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct FriendView {
    pub key: String,
    pub name: String,
    pub shared: Vec<InstanceUuid>,
    /// Hidden from (see [`FriendEntry::hidden`]).
    pub hidden: bool,
    /// Connected now.
    pub connected: bool,
}

/// `GET /connect/friends`.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct FriendsView {
    /// SHA-256 of this core's friend certificate, in hex: what friends pin.
    pub fingerprint: String,
    pub version: u64,
    pub friends: Vec<FriendView>,
    /// The friend host, while it runs (something is shared with someone), or while
    /// it's being started again after it couldn't start (offline, say): then its state
    /// is `reconnecting` and `problem` says why.
    pub host: Option<FriendHostStatus>,
}

/// One friend, checked.
#[derive(Clone)]
struct Friend {
    key: PublicKey,
    name: String,
    secret: Secret,
    shared: HashSet<InstanceUuid>,
    hidden: bool,
}

impl Friend {
    fn from_entry(entry: &FriendEntry) -> Result<Self, Error> {
        let key = PublicKey::from_hex(&entry.key)
            .ok_or_else(|| bad_request("A friend's key isn't 64 hex digits"))?;
        let secret = Secret::from_hex(&entry.secret)
            .ok_or_else(|| bad_request("A friend's secret isn't 64 hex digits"))?;
        let name = clean_name(&entry.name);
        if name.is_empty() {
            return Err(bad_request("A friend has no name"));
        }
        if entry.shared.len() > MAX_SHARED {
            return Err(bad_request(format!(
                "At most {MAX_SHARED} servers can be shared with one friend"
            )));
        }
        Ok(Self {
            key,
            name,
            secret,
            shared: entry.shared.iter().cloned().collect(),
            hidden: entry.hidden,
        })
    }
}

/// Which instances each friend may reach: what the stream handler checks.
pub(crate) type Access = Arc<RwLock<HashMap<PublicKey, HashSet<InstanceUuid>>>>;

// ---------------------------------------------------------------------------
// The shares

struct Inner {
    version: u64,
    friends: Vec<Friend>,
    loaded: bool,
    host: Option<FriendHost>,
    /// Why the friend host couldn't start, while it's being tried again.
    problem: Option<String>,
    /// Trying to start the friend host again, every [`RETRY`].
    retry: Option<tokio::task::JoinHandle<()>>,
}

/// The friend list and the friend host.
pub struct FriendShares {
    dir: PathBuf,
    service: Service,
    gather: GatherOptions,
    inner: Mutex<Inner>,
    access: Access,
    identity: Mutex<Option<CoreIdentity>>,
    /// Where friends' info streams go: one server for the life of the shares, started
    /// with the first friend host, so restarting the friend host doesn't leave one
    /// behind each time.
    info: std::sync::OnceLock<mpsc::Sender<(TunnelStream, PublicKey)>>,
    /// How long to wait before trying to start the friend host again: [`RETRY`].
    retry: Duration,
    /// Starts to fail on purpose, as if offline.
    #[cfg(test)]
    fail_starts: std::sync::Mutex<usize>,
    /// The knocks the friend host let in, kept while the core runs: the host stops when
    /// nothing is shared and starts again later, and must still refuse old knocks.
    knocks: KnockMemory,
}

impl FriendShares {
    /// Kept in `dir` (`stores/connect/friends`), through `service`.
    pub fn new(dir: PathBuf, service: Service, gather: GatherOptions) -> Self {
        Self {
            dir,
            service,
            gather,
            inner: Mutex::new(Inner {
                version: 0,
                friends: Vec::new(),
                loaded: false,
                host: None,
                problem: None,
                retry: None,
            }),
            access: Arc::default(),
            identity: Mutex::new(None),
            info: std::sync::OnceLock::new(),
            retry: RETRY,
            #[cfg(test)]
            fail_starts: Default::default(),
            knocks: KnockMemory::default(),
        }
    }

    /// Whether this start should fail on purpose (counting it).
    #[cfg(test)]
    fn take_failed_start(&self) -> bool {
        let mut left = self.fail_starts.lock().expect("not poisoned");
        let fail = *left > 0;
        *left = left.saturating_sub(1);
        fail
    }

    /// Fails the next `starts` friend host starts, and tries again after `retry`.
    #[cfg(test)]
    pub(crate) fn failing_for_tests(mut self, starts: usize, retry: Duration) -> Self {
        self.fail_starts = std::sync::Mutex::new(starts);
        self.retry = retry;
        self
    }

    /// Where friends' info streams go, starting the server for them the first time.
    fn info_streams(&self, state: &AppState) -> mpsc::Sender<(TunnelStream, PublicKey)> {
        self.info
            .get_or_init(|| {
                let (info, streams) = mpsc::channel(16);
                tokio::spawn(serve_friend_info(
                    state.clone(),
                    self.access.clone(),
                    streams,
                ));
                info
            })
            .clone()
    }

    fn identity_file(&self) -> PathBuf {
        self.dir.join("core.json")
    }

    fn list_file(&self) -> PathBuf {
        self.dir.join("list.json")
    }

    async fn ensure_dir(&self) -> Result<(), Error> {
        let dir = self.dir.clone();
        tokio::task::spawn_blocking(move || {
            // stores/connect first (private too), then its friends folder
            if let Some(parent) = dir.parent() {
                crate::local_claim::ensure_private_dir(parent)?;
            }
            crate::local_claim::ensure_private_dir(&dir)
        })
        .await
        .map_err(|e| internal(format!("Failed to create the friends folder: {e}")))?
    }

    /// The friend certificate, made on first use.
    ///
    /// A new one is made only when there is no file, or when the file is damaged; a
    /// damaged file is kept beside it as `core.json.damaged-<seconds since 1970>`.
    /// Any other read error (permissions, a disk problem) is an error: replacing the
    /// certificate would cut off every friend who pinned it.
    pub async fn identity(&self) -> Result<CoreIdentity, Error> {
        let mut identity = self.identity.lock().await;
        if let Some(identity) = identity.as_ref() {
            return Ok(identity.clone());
        }
        let file = self.identity_file();
        let loaded = match tokio::fs::read_to_string(&file).await {
            Ok(text) => match CoreIdentity::from_json(&text) {
                Ok(loaded) => Some(loaded),
                Err(e) => {
                    let since = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs());
                    let backup = self.dir.join(format!("core.json.damaged-{since}"));
                    tokio::fs::rename(&file, &backup).await.map_err(|err| {
                        internal(format!(
                            "The friend certificate is damaged ({e}), and it couldn't be moved aside: {err}"
                        ))
                    })?;
                    // Friends pinned the old one: they'll need the owner's new state.
                    warn!(
                        "The friend certificate is damaged ({e}); it was moved to {} and a new one is made. Friends see this core again once the owner's launcher sends them its new state.",
                        backup.display()
                    );
                    None
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                return Err(internal(format!(
                    "Couldn't read the friend certificate {}: {e}",
                    file.display()
                )))
            }
        };
        let made = match loaded {
            Some(loaded) => loaded,
            None => {
                let made = CoreIdentity::generate().map_err(|e| internal(e.to_string()))?;
                self.ensure_dir().await?;
                crate::persist::write_atomic(self.identity_file(), made.to_json(), Some(0o600))
                    .await?;
                made
            }
        };
        *identity = Some(made.clone());
        Ok(made)
    }

    /// Reads the saved list once.
    async fn load(&self, inner: &mut Inner) {
        if inner.loaded {
            return;
        }
        inner.loaded = true;
        let Ok(text) = tokio::fs::read_to_string(self.list_file()).await else {
            return;
        };
        let list: FriendList = match serde_json::from_str(&text) {
            Ok(list) => list,
            Err(e) => return warn!("The friend list is damaged; starting with none: {e}"),
        };
        let friends: Result<Vec<Friend>, Error> =
            list.friends.iter().map(Friend::from_entry).collect();
        match friends {
            Ok(friends) => {
                inner.version = list.version;
                inner.friends = friends;
            }
            Err(e) => warn!("The friend list is damaged; starting with none: {e}"),
        }
    }

    /// The list without secrets, and how the friend host is doing.
    pub async fn view(&self) -> Result<FriendsView, Error> {
        let fingerprint = lodestone_connect_hex(&self.identity().await?.fingerprint());
        let mut inner = self.inner.lock().await;
        self.load(&mut inner).await;
        let host = match (&inner.host, &inner.problem) {
            (Some(host), _) => Some(host.status()),
            // not started yet: being tried again
            (None, Some(problem)) => Some(FriendHostStatus {
                state: HostState::Reconnecting,
                publishing: 0,
                candidates: Vec::new(),
                symmetric_nat: false,
                connected: Vec::new(),
                problem: Some(problem.clone()),
            }),
            (None, None) => None,
        };
        let connected: HashSet<String> = host
            .iter()
            .flat_map(|status| status.connected.iter().map(|c| c.key.clone()))
            .collect();
        Ok(FriendsView {
            fingerprint,
            version: inner.version,
            friends: inner
                .friends
                .iter()
                .map(|friend| {
                    let key = friend.key.to_hex();
                    let mut shared: Vec<InstanceUuid> = friend.shared.iter().cloned().collect();
                    shared.sort_by_key(|a| a.to_string());
                    FriendView {
                        connected: !friend.hidden && connected.contains(&key),
                        key,
                        name: friend.name.clone(),
                        shared,
                        hidden: friend.hidden,
                    }
                })
                .collect(),
            host,
        })
    }

    /// Replaces the list with `list`, whose version must be higher than the current one,
    /// saves it, and applies it: friends no longer on it, or with less shared, are
    /// disconnected at once. If the friend host can't start (offline, say), the list is
    /// still saved and the host is tried again in the background; the view's host
    /// status says why.
    pub async fn replace(
        self: &Arc<Self>,
        state: &AppState,
        list: FriendList,
    ) -> Result<FriendsView, Error> {
        if list.friends.len() > MAX_FRIENDS {
            return Err(bad_request(format!(
                "A core keeps at most {MAX_FRIENDS} friends"
            )));
        }
        let friends: Vec<Friend> = list
            .friends
            .iter()
            .map(Friend::from_entry)
            .collect::<Result<_, _>>()?;
        let keys: HashSet<PublicKey> = friends.iter().map(|f| f.key).collect();
        if keys.len() != friends.len() {
            return Err(bad_request("A friend is in the list twice"));
        }
        if friends
            .iter()
            .filter(|f| !f.hidden && !f.shared.is_empty())
            .count()
            > MAX_SHARING_FRIENDS
        {
            return Err(bad_request(format!(
                "A core can share with at most {MAX_SHARING_FRIENDS} friends"
            )));
        }
        // the certificate exists before anyone is told about it
        self.identity().await?;
        {
            let mut inner = self.inner.lock().await;
            self.load(&mut inner).await;
            if list.version <= inner.version {
                return Err(Error {
                    kind: ErrorKind::Conflict,
                    source: eyre!(
                        "This core already has version {} of the friend list",
                        inner.version
                    ),
                });
            }
            let stored = FriendList {
                version: list.version,
                friends: friends
                    .iter()
                    .map(|f| FriendEntry {
                        key: f.key.to_hex(),
                        name: f.name.clone(),
                        secret: f.secret.to_hex(),
                        shared: f.shared.iter().cloned().collect(),
                        hidden: f.hidden,
                    })
                    .collect(),
            };
            self.ensure_dir().await?;
            crate::persist::write_atomic(
                self.list_file(),
                serde_json::to_string_pretty(&stored).expect("serializable"),
                Some(0o600),
            )
            .await?;
            inner.version = list.version;
            inner.friends = friends;
            self.apply_or_retry(state, &mut inner).await;
        }
        self.view().await
    }

    /// Makes the friend host match the list: started while anyone has something shared,
    /// stopped otherwise.
    async fn apply(self: &Arc<Self>, state: &AppState, inner: &mut Inner) -> Result<(), Error> {
        // only instances that exist and are Minecraft Java servers, and nothing for
        // hidden friends: no records, no connections, nothing served
        let mut access: HashMap<PublicKey, HashSet<InstanceUuid>> = HashMap::new();
        for friend in inner.friends.iter().filter(|f| !f.hidden) {
            let mut shared = HashSet::new();
            for uuid in &friend.shared {
                if let Ok(instance) = get_instance(state, uuid) {
                    if matches!(instance.game_type().await, Game::MinecraftJava { .. }) {
                        shared.insert(uuid.clone());
                    }
                }
            }
            if !shared.is_empty() {
                access.insert(friend.key, shared);
            }
        }
        // who lost something: disconnect them (they reconnect for what's left)
        let lost: Vec<PublicKey> = {
            let before = self.access.read().expect("not poisoned");
            before
                .iter()
                .filter(|(key, had)| access.get(key).is_none_or(|now| !had.is_subset(now)))
                .map(|(key, _)| *key)
                .collect()
        };
        *self.access.write().expect("not poisoned") = access.clone();
        let friends: Vec<FriendAccess> = inner
            .friends
            .iter()
            .filter(|f| access.contains_key(&f.key))
            .map(|f| FriendAccess {
                key: f.key,
                secret: f.secret.clone(),
            })
            .collect();
        if friends.is_empty() {
            if let Some(host) = inner.host.take() {
                host.stop().await;
                info!("Stopped sharing with friends: nothing is shared");
            }
            return Ok(());
        }
        match &inner.host {
            Some(host) => {
                host.set_friends(friends);
                for key in &lost {
                    host.disconnect(key);
                }
            }
            None => {
                #[cfg(test)]
                if self.take_failed_start() {
                    return Err(Error {
                        kind: ErrorKind::External,
                        source: eyre!("Couldn't start sharing with friends: offline (test)"),
                    });
                }
                let identity = self.identity().await?;
                let handler = CoreFriendStreams {
                    state: state.clone(),
                    access: self.access.clone(),
                    info: self.info_streams(state),
                };
                let host = FriendHost::start(
                    &identity,
                    friends,
                    FriendHostConfig {
                        service: self.service.clone(),
                        gather: self.gather,
                        knocks: self.knocks.clone(),
                    },
                    handler,
                )
                .await
                .map_err(|e| Error {
                    kind: ErrorKind::External,
                    source: eyre!("Couldn't start sharing with friends: {e}"),
                })?;
                inner.host = Some(host);
                info!("Sharing with friends");
            }
        }
        Ok(())
    }

    /// [`Self::apply`], and if that fails (the friend host can't start while this
    /// computer is offline, say), notes why for the host status and tries again every
    /// [`RETRY`] in the background until it works. A later apply that works stops
    /// the retries.
    async fn apply_or_retry(self: &Arc<Self>, state: &AppState, inner: &mut Inner) {
        match self.apply(state, inner).await {
            Ok(()) => {
                inner.problem = None;
                if let Some(retry) = inner.retry.take() {
                    // it waits for the lock this holds, so it isn't in the middle of a start
                    retry.abort();
                }
            }
            Err(e) => {
                warn!(
                    "Couldn't start sharing with friends, trying again soon: {}",
                    e.source
                );
                inner.problem = Some(e.source.to_string());
                if inner.retry.as_ref().is_none_or(|retry| retry.is_finished()) {
                    let shares = self.clone();
                    let state = state.clone();
                    inner.retry = Some(tokio::spawn(async move { shares.retry(state).await }));
                }
            }
        }
    }

    async fn retry(self: Arc<Self>, state: AppState) {
        loop {
            tokio::time::sleep(self.retry).await;
            let mut inner = self.inner.lock().await;
            match self.apply(&state, &mut inner).await {
                Ok(()) => {
                    inner.problem = None;
                    inner.retry = None;
                    return;
                }
                Err(e) => {
                    warn!(
                        "Couldn't start sharing with friends, trying again soon: {}",
                        e.source
                    );
                    inner.problem = Some(e.source.to_string());
                }
            }
        }
    }

    /// Starts sharing with friends as the core starts, if anything is shared, trying
    /// again in the background while this computer is offline.
    pub async fn resume(self: &Arc<Self>, state: &AppState) {
        let mut inner = self.inner.lock().await;
        self.load(&mut inner).await;
        self.apply_or_retry(state, &mut inner).await;
    }

    /// Replaces the friend certificate with a new one, for an owner who retired this core
    /// in their state (friends refuse a retired fingerprint for good). The old
    /// `core.json` is kept as `core.json.retired-<seconds since 1970>`. A running friend
    /// host is restarted with the new certificate, which drops every friend connection;
    /// the friend list and what's shared stay. Friends reach this core again once the
    /// owner's launcher lists the new fingerprint in its state.
    pub async fn rotate_certificate(
        self: &Arc<Self>,
        state: &AppState,
    ) -> Result<FriendsView, Error> {
        let mut inner = self.inner.lock().await;
        self.load(&mut inner).await;
        {
            // the old one is read (or made) first, so a damaged file is set aside as such
            let old = self.identity().await?;
            let made = CoreIdentity::generate().map_err(|e| internal(e.to_string()))?;
            let mut identity = self.identity.lock().await;
            self.ensure_dir().await?;
            let file = self.identity_file();
            if tokio::fs::try_exists(&file).await.unwrap_or(false) {
                let since = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let mut backup = self.dir.join(format!("core.json.retired-{since}"));
                for n in 1.. {
                    if !tokio::fs::try_exists(&backup).await.unwrap_or(false) {
                        break;
                    }
                    backup = self.dir.join(format!("core.json.retired-{since}-{n}"));
                }
                tokio::fs::rename(&file, &backup).await.map_err(|e| {
                    internal(format!(
                        "Couldn't keep the old friend certificate aside: {e}"
                    ))
                })?;
            }
            crate::persist::write_atomic(&file, made.to_json(), Some(0o600)).await?;
            *identity = Some(made.clone());
            info!(
                "Made a new friend certificate {} (was {})",
                lodestone_connect_hex(&made.fingerprint()),
                lodestone_connect_hex(&old.fingerprint())
            );
        }
        // the friend host serves the old certificate: start it again with the new one
        if let Some(host) = inner.host.take() {
            host.stop().await;
        }
        self.access.write().expect("not poisoned").clear();
        self.apply_or_retry(state, &mut inner).await;
        drop(inner);
        self.view().await
    }

    /// The running friend host's certificate fingerprint.
    #[cfg(test)]
    pub(crate) async fn host_fingerprint_for_tests(&self) -> Option<[u8; 32]> {
        self.inner
            .lock()
            .await
            .host
            .as_ref()
            .map(FriendHost::fingerprint)
    }

    /// Applies the list again: after an instance was deleted, say.
    pub async fn refresh(self: &Arc<Self>, state: &AppState) {
        let mut inner = self.inner.lock().await;
        self.load(&mut inner).await;
        self.apply_or_retry(state, &mut inner).await;
    }

    /// Stops the friend host as the core shuts down.
    pub async fn stop(&self) {
        let mut inner = self.inner.lock().await;
        if let Some(retry) = inner.retry.take() {
            retry.abort();
        }
        inner.problem = None;
        if let Some(host) = inner.host.take() {
            host.stop().await;
        }
    }

    /// The info streams' channel, once its server started.
    #[cfg(test)]
    pub(crate) fn info_for_tests(&self) -> Option<mpsc::Sender<(TunnelStream, PublicKey)>> {
        self.info.get().cloned()
    }

    /// Which instances each friend may reach now.
    #[cfg(test)]
    pub(crate) fn access_for_tests(&self) -> Access {
        self.access.clone()
    }
}

fn lodestone_connect_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Streams

/// Whose info stream a connection is: what the friend info routes see as the peer.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FriendPeer(PublicKey);

/// Friends' info streams, as connections for axum, each with whose it is.
struct FriendListener<Io> {
    streams: mpsc::Receiver<(Io, PublicKey)>,
}

impl<Io> axum::serve::Listener for FriendListener<Io>
where
    Io: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    type Io = Io;
    type Addr = FriendPeer;

    async fn accept(&mut self) -> (Io, FriendPeer) {
        match self.streams.recv().await {
            Some((stream, key)) => (stream, FriendPeer(key)),
            // the friend host stopped: nothing more comes
            None => std::future::pending().await,
        }
    }

    fn local_addr(&self) -> std::io::Result<FriendPeer> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
}

impl<Io> Connected<IncomingStream<'_, FriendListener<Io>>> for FriendPeer
where
    Io: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    fn connect_info(stream: IncomingStream<'_, FriendListener<Io>>) -> Self {
        *stream.remote_addr()
    }
}

/// Answers HTTP on each friend's stream from `streams` with [`friend_router`].
pub(crate) async fn serve_friend_info<Io>(
    state: AppState,
    access: Access,
    streams: mpsc::Receiver<(Io, PublicKey)>,
) where
    Io: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let _ = axum::serve(
        FriendListener { streams },
        friend_router(state, access).into_make_service_with_connect_info::<FriendPeer>(),
    )
    .await;
}

/// Passes friends' game streams to the instance's game port, if it's shared with them
/// and running, and their info streams to [`friend_router`].
struct CoreFriendStreams {
    state: AppState,
    access: Access,
    info: mpsc::Sender<(TunnelStream, PublicKey)>,
}

fn may_reach(access: &Access, friend: &PublicKey, uuid: &InstanceUuid) -> bool {
    access
        .read()
        .expect("not poisoned")
        .get(friend)
        .is_some_and(|shared| shared.contains(uuid))
}

impl FriendHandler for CoreFriendStreams {
    fn game(&self, stream: TunnelStream, friend: PublicKey, instance: String) {
        let uuid = InstanceUuid::from(instance);
        if !may_reach(&self.access, &friend, &uuid) {
            return;
        }
        let state = self.state.clone();
        tokio::spawn(async move {
            let Ok(instance) = get_instance(&state, &uuid) else {
                return;
            };
            if !game_port_open(instance.state().await) {
                return;
            }
            let Ok(port) = u16::try_from(instance.port().await) else {
                return;
            };
            lodestone_connect::host::pipe(stream, SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
                .await;
        });
    }

    fn info(&self, stream: TunnelStream, friend: PublicKey) {
        // dropped (closed) when too many are waiting
        let _ = self.info.try_send((stream, friend));
    }
}

#[derive(Clone)]
struct FriendRoutes {
    state: AppState,
    access: Access,
}

/// The routes friends reach through info streams, for the instances shared with them:
///
/// - `GET /api/v1/friend/servers`: their [`SharedServer`]s;
/// - `GET /api/v1/public/instances`: those instances, as the public list shows them;
/// - `GET /api/v1/public/instances/{uuid}/client-pack`: a shared instance's client pack.
///
/// Everything else is 404.
pub(crate) fn friend_router(state: AppState, access: Access) -> Router {
    Router::new()
        .route("/api/v1/friend/servers", get(get_friend_servers))
        .route("/api/v1/public/instances", get(get_friend_list))
        .route(
            "/api/v1/public/instances/{uuid}/client-pack",
            get(get_friend_pack),
        )
        .with_state(FriendRoutes { state, access })
}

fn not_found() -> Error {
    Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Server not found"),
    }
}

/// The instances shared with `friend` that still exist, in a stable order.
async fn shared_with(
    routes: &FriendRoutes,
    friend: &PublicKey,
) -> Vec<crate::public_listing::PublicInstance> {
    let mut uuids: Vec<InstanceUuid> = routes
        .access
        .read()
        .expect("not poisoned")
        .get(friend)
        .map(|shared| shared.iter().cloned().collect())
        .unwrap_or_default();
    uuids.sort_by_key(|a| a.to_string());
    let mut out = Vec::new();
    for uuid in uuids {
        if let Ok(instance) = get_instance(&routes.state, &uuid) {
            if let Some(public) = public_instance(&instance, SHARED_SCOPE).await {
                out.push(public);
            }
        }
    }
    out
}

async fn get_friend_servers(
    State(routes): State<FriendRoutes>,
    ConnectInfo(FriendPeer(friend)): ConnectInfo<FriendPeer>,
) -> Json<Vec<SharedServer>> {
    let mut servers = Vec::new();
    for instance in shared_with(&routes, &friend).await {
        servers.push(shared_server_of(&routes.state, instance).await);
    }
    Json(servers)
}

async fn get_friend_list(
    State(routes): State<FriendRoutes>,
    ConnectInfo(FriendPeer(friend)): ConnectInfo<FriendPeer>,
) -> Json<PublicInstanceList> {
    Json(PublicInstanceList {
        core_name: routes.state.global_settings.lock().await.core_name(),
        core_uuid: routes.state.uuid.clone(),
        listing_enabled: true,
        instances: shared_with(&routes, &friend).await,
    })
}

async fn get_friend_pack(
    State(routes): State<FriendRoutes>,
    ConnectInfo(FriendPeer(friend)): ConnectInfo<FriendPeer>,
    UrlPath(uuid): UrlPath<InstanceUuid>,
    headers: HeaderMap,
) -> Result<Response, Error> {
    if !may_reach(&routes.access, &friend, &uuid) {
        return Err(not_found());
    }
    get_instance(&routes.state, &uuid)?;
    pack_response(&routes.state, &uuid, &headers).await
}
