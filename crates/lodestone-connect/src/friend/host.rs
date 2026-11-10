//! A core's friend host (launcher/docs/friends.md, "Finding a friend's core" and
//! "Joining"): publishes one sealed record per friend at the service, punches towards
//! friends who knock, and serves their QUIC connections.
//!
//! - It has its own certificate ([`CoreIdentity`]), socket and QUIC protocol
//!   ([`FRIEND_ALPN`]), apart from every invite host.
//! - It answers QUIC only from addresses in a knock it accepted in the last
//!   [`ALLOW_FOR`], or on a connection now; everyone else gets silence, so a removed
//!   friend who remembers the core's address learns nothing. Their datagrams never
//!   reach QUIC ([`Gate`]), so not even a version negotiation or a stateless reset
//!   answers them.
//! - The first stream of a connection must be the friend hello: the friend's key and a
//!   signature over this connection's TLS exporter. Until then nothing else is read.
//! - Every later stream is checked against the friend list as it is then.

use std::{
    collections::{HashMap, VecDeque},
    fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::{sync::watch, task::JoinHandle};

use super::{
    client::{DeliveredKnock, MeetEvent, MeetSession},
    crypto::{probe_tag, PublicKey, Secret, ID_LEN, SIGNATURE_LEN},
    gate::Gate,
    meet::{self, ReplayGuard},
    protocol::{hello_message, FriendStreamKind, HELLO_CONTEXT, HELLO_LABEL, MAX_INSTANCE_LEN},
};
use crate::{
    addr::{canonical, ConnectionKind},
    gather::{self, Candidate, CandidateKind, GatherOptions},
    hex,
    host::{HostState, MAX_FRIENDS, MAX_SPRAYS, MAX_UNPROVEN},
    portmap, probe,
    service::Service,
    tunnel::{self, TunnelStream, FRIEND_ALPN, HELLO_ANSWER},
    unhex, Error, Result,
};

/// How long an accepted knock lets its addresses connect.
pub const ALLOW_FOR: Duration = Duration::from_secs(120);
/// How long the core punches towards a friend who knocked.
const SPRAY_FOR: Duration = Duration::from_secs(60);
/// Records are renewed at least this often, well within [`meet::RECORD_MAX_AGE_SECS`].
const RENEW_AT_MOST: Duration = Duration::from_secs(6 * 60 * 60);
/// How often the host collects its addresses again (publishing them if they changed)
/// and renews the router's forwarding (which lasts an hour).
const REFRESH_EVERY: Duration = Duration::from_secs(10 * 60);
/// How long a stream has to send its header.
const HEADER_WITHIN: Duration = Duration::from_secs(10);
/// How long a new connection has to finish its handshake and say hello. Until then it
/// holds a place counted against the friend whose knock let its address in.
const HELLO_WITHIN: Duration = Duration::from_secs(10);
/// The most addresses one knock opens (its candidates).
const MAX_ALLOWED_PER_KNOCK: usize = meet::MAX_CANDIDATES;
/// How many of a friend's latest knocks count at once: each lets its addresses connect
/// and gets its own spray, so two joins at once from one friend (two launchers, or two
/// servers on this core) don't cancel each other. An older knock's go when a newer one
/// comes, so one friend never holds more than this.
const KNOCKS_PER_FRIEND: usize = 2;
/// The most connections one friend may hold at once, so no friend can take every place.
/// Connections still in their handshake or hello count too, against the friend whose
/// knock let their address in.
pub const MAX_CONNECTIONS_PER_FRIEND: usize = 4;

// ---------------------------------------------------------------------------
// Identity

/// A core's certificate and key for friends: one per core, kept in a file only the
/// core's user can read. Friends pin its fingerprint, from their friend's state.
#[derive(Clone)]
pub struct CoreIdentity {
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
}

impl fmt::Debug for CoreIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoreIdentity")
            .field("fingerprint", &hex(&self.fingerprint()))
            .finish_non_exhaustive()
    }
}

#[derive(Serialize, Deserialize)]
struct IdentityFile {
    version: u32,
    cert: String,
    key: String,
}

impl CoreIdentity {
    /// A new certificate (ECDSA P-256).
    pub fn generate() -> Result<Self> {
        let certified =
            rcgen::generate_simple_self_signed(vec![tunnel::SERVER_NAME.to_string()])
                .map_err(|err| Error::Other(format!("Couldn't make a certificate: {err}")))?;
        Ok(Self {
            cert_der: certified.cert.der().to_vec(),
            key_der: certified.signing_key.serialize_der(),
        })
    }

    pub fn fingerprint(&self) -> [u8; 32] {
        tunnel::fingerprint(&self.cert_der)
    }

    /// The identity as JSON, with its private key: keep it where only the core's user
    /// can read it.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&IdentityFile {
            version: 1,
            cert: hex(&self.cert_der),
            key: hex(&self.key_der),
        })
        .expect("serializable")
    }

    pub fn from_json(text: &str) -> Result<Self> {
        let bad =
            |what: &str| Error::Other(format!("The saved friend certificate is damaged ({what})"));
        let file: IdentityFile = serde_json::from_str(text).map_err(|_| bad("not JSON"))?;
        if file.version != 1 {
            return Err(bad("unknown version"));
        }
        let identity = Self {
            cert_der: unhex(&file.cert).ok_or_else(|| bad("certificate"))?,
            key_der: unhex(&file.key).ok_or_else(|| bad("key"))?,
        };
        tunnel::server_config_for(&identity.cert_der, &identity.key_der, FRIEND_ALPN)
            .map_err(|_| bad("key"))?;
        Ok(identity)
    }
}

// ---------------------------------------------------------------------------
// Friends and streams

/// A friend this core accepts: their key, and the core secret this core shares with
/// them. Give the host only friends with something shared with them.
#[derive(Clone)]
pub struct FriendAccess {
    pub key: PublicKey,
    pub secret: Secret,
}

impl fmt::Debug for FriendAccess {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FriendAccess")
            .field("key", &self.key)
            .finish_non_exhaustive()
    }
}

/// What the core does with friends' streams. Each method gets a stream from a friend on
/// the current list, whose header has been read; it should check what's shared with
/// that friend, spawn a task and return.
pub trait FriendHandler: Send + Sync + 'static {
    /// Minecraft's connection to `instance`.
    fn game(&self, stream: TunnelStream, friend: PublicKey, instance: String);
    /// HTTP/1.1 for the friend info routes.
    fn info(&self, stream: TunnelStream, friend: PublicKey);
}

/// A friend connected now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectedFriend {
    /// The friend's public key, in hex.
    pub key: String,
    pub address: SocketAddr,
    pub kind: ConnectionKind,
    /// Seconds since 1970.
    pub connected_at: u64,
    pub rtt_ms: u64,
}

/// How the friend host is doing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendHostStatus {
    pub state: HostState,
    /// Friends with a record published.
    pub publishing: usize,
    pub candidates: Vec<Candidate>,
    pub symmetric_nat: bool,
    pub connected: Vec<ConnectedFriend>,
    pub problem: Option<String>,
}

/// How to run the friend host.
#[derive(Clone, Debug, Default)]
pub struct FriendHostConfig {
    pub service: Service,
    pub gather: GatherOptions,
}

struct Connected {
    key: PublicKey,
    connection: quinn::Connection,
    connected_at: SystemTime,
}

/// The friend host's connections: those that said hello, and the places held by those
/// that haven't yet. Behind one lock, so a check and the insert after it can't race.
#[derive(Default)]
struct Connections {
    live: HashMap<u64, Connected>,
    /// For each friend, how many connections from addresses their knocks let in are
    /// still in their handshake or hello.
    unproven: HashMap<PublicKey, usize>,
}

impl Connections {
    fn unproven_total(&self) -> usize {
        self.unproven.values().sum()
    }

    /// The places `friend` holds: their connections, and the unproven ones counted
    /// against them.
    fn held_by(&self, friend: &PublicKey) -> usize {
        self.live.values().filter(|c| &c.key == friend).count()
            + self.unproven.get(friend).copied().unwrap_or(0)
    }

    fn release(&mut self, friend: &PublicKey) {
        if let Some(count) = self.unproven.get_mut(friend) {
            *count -= 1;
            if *count == 0 {
                self.unproven.remove(friend);
            }
        }
    }
}

/// Why a new connection isn't let in.
#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    /// No accepted knock lists its address: say nothing.
    Stranger,
    /// Its friend, or the host, has no place left.
    Full,
}

/// A place held by a connection that hasn't said hello yet, counted against the friend
/// whose knock let its address in. Dropping it frees the place.
struct Unproven {
    shared: Arc<Shared>,
    friend: Option<PublicKey>,
}

impl Drop for Unproven {
    fn drop(&mut self) {
        if let Some(friend) = self.friend.take() {
            self.shared
                .connections
                .lock()
                .expect("not poisoned")
                .release(&friend);
        }
    }
}

/// An address on a connection now (or a connection still saying hello): its datagrams
/// pass the [`Gate`] after its knock's [`ALLOW_FOR`] ends. Dropping it lets that go.
struct PeerHold {
    shared: Arc<Shared>,
    ip: IpAddr,
}

impl Drop for PeerHold {
    fn drop(&mut self) {
        let mut peers = self.shared.peers.lock().expect("not poisoned");
        if let Some(count) = peers.get_mut(&self.ip) {
            *count -= 1;
            if *count == 0 {
                peers.remove(&self.ip);
            }
        }
    }
}

struct Shared {
    fingerprint: [u8; 32],
    service: Service,
    friends: Mutex<HashMap<PublicKey, Secret>>,
    /// Bumped whenever the friend list changes, so the records are published again.
    generation: watch::Sender<u64>,
    /// For each friend, the addresses of their latest accepted knocks (at most
    /// [`KNOCKS_PER_FRIEND`]) and until when they may connect. A new knock replaces the
    /// friend's oldest, so one friend can never crowd out another.
    allowed: Mutex<HashMap<PublicKey, VecDeque<(Vec<IpAddr>, Instant)>>>,
    connections: Mutex<Connections>,
    /// The addresses on connections now, and how many connections each has. The
    /// [`Gate`] reads this for every datagram, inside QUIC: never call QUIC while
    /// holding it (nor `allowed`).
    peers: Mutex<HashMap<IpAddr, usize>>,
    next_connection: AtomicU64,
    status: Mutex<FriendHostStatus>,
    changed: watch::Sender<u64>,
    mapping: Mutex<Option<portmap::Mapping>>,
    /// The addresses the records carry. Collected again every [`REFRESH_EVERY`].
    candidates: Mutex<Vec<SocketAddr>>,
}

impl Shared {
    fn update(&self, change: impl FnOnce(&mut FriendHostStatus)) {
        change(&mut self.status.lock().expect("not poisoned"));
        self.changed.send_modify(|n| *n += 1);
    }

    fn is_friend(&self, key: &PublicKey) -> bool {
        self.friends.lock().expect("not poisoned").contains_key(key)
    }

    /// Whether datagrams from `ip` may reach QUIC: an accepted knock lets it connect, or
    /// it's on a connection now. For the [`Gate`], so it allocates nothing.
    fn may_reach(&self, ip: IpAddr) -> bool {
        if self.peers.lock().expect("not poisoned").contains_key(&ip) {
            return true;
        }
        let now = Instant::now();
        self.allowed
            .lock()
            .expect("not poisoned")
            .values()
            .flatten()
            .any(|(ips, until)| *until > now && ips.contains(&ip))
    }

    /// Lets `ip`'s datagrams through for as long as the hold lasts.
    fn hold_peer(self: &Arc<Self>, ip: IpAddr) -> PeerHold {
        *self
            .peers
            .lock()
            .expect("not poisoned")
            .entry(ip)
            .or_default() += 1;
        PeerHold {
            shared: self.clone(),
            ip,
        }
    }

    /// The friends whose accepted knocks let `ip` connect now.
    fn allowed_by(&self, ip: IpAddr) -> Vec<PublicKey> {
        let mut allowed = self.allowed.lock().expect("not poisoned");
        forget_expired(&mut allowed);
        allowed
            .iter()
            .filter(|(_, knocks)| knocks.iter().any(|(ips, _)| ips.contains(&ip)))
            .map(|(friend, _)| *friend)
            .collect()
    }

    /// Takes a place for a new connection from `ip`, counted against a friend whose
    /// knock let `ip` in and who has a place left. Anyone at an address a friend's
    /// knock names can connect, so these places are that friend's: they can't fill the
    /// host's.
    fn reserve(self: &Arc<Self>, ip: IpAddr) -> Result<Unproven, Refusal> {
        let friends = self.allowed_by(ip);
        if friends.is_empty() {
            return Err(Refusal::Stranger);
        }
        let mut connections = self.connections.lock().expect("not poisoned");
        if connections.live.len() >= MAX_FRIENDS || connections.unproven_total() >= MAX_UNPROVEN {
            return Err(Refusal::Full);
        }
        let friend = friends
            .into_iter()
            .map(|friend| (connections.held_by(&friend), friend))
            .filter(|(held, _)| *held < MAX_CONNECTIONS_PER_FRIEND)
            .min_by_key(|(held, _)| *held)
            .map(|(_, friend)| friend)
            .ok_or(Refusal::Full)?;
        *connections.unproven.entry(friend).or_default() += 1;
        Ok(Unproven {
            shared: self.clone(),
            friend: Some(friend),
        })
    }

    /// Lets in a connection that said hello as `key`, if `key` has a place left: frees
    /// its unproven place, then checks and inserts under the same lock. Returns its id.
    fn admit(
        &self,
        mut unproven: Unproven,
        key: PublicKey,
        connection: &quinn::Connection,
    ) -> Option<u64> {
        let mut connections = self.connections.lock().expect("not poisoned");
        if let Some(friend) = unproven.friend.take() {
            connections.release(&friend);
        }
        if connections.held_by(&key) >= MAX_CONNECTIONS_PER_FRIEND
            || connections.live.len() >= MAX_FRIENDS
        {
            return None;
        }
        let id = self.next_connection.fetch_add(1, Ordering::Relaxed);
        connections.live.insert(
            id,
            Connected {
                key,
                connection: connection.clone(),
                connected_at: SystemTime::now(),
            },
        );
        Some(id)
    }

    /// Lets `friend` connect from `ips` (a knock's candidates) for [`ALLOW_FOR`],
    /// replacing what their oldest knock allowed once they have [`KNOCKS_PER_FRIEND`].
    /// The address the service says it saw isn't used: the service could name any
    /// address.
    fn allow(&self, friend: PublicKey, ips: impl IntoIterator<Item = IpAddr>) {
        let mut list: Vec<IpAddr> = Vec::new();
        for ip in ips {
            let ip = canonical(SocketAddr::new(ip, 0)).ip();
            if !list.contains(&ip) && list.len() < MAX_ALLOWED_PER_KNOCK {
                list.push(ip);
            }
        }
        let mut allowed = self.allowed.lock().expect("not poisoned");
        forget_expired(&mut allowed);
        let knocks = allowed.entry(friend).or_default();
        while knocks.len() >= KNOCKS_PER_FRIEND {
            knocks.pop_front();
        }
        knocks.push_back((list, Instant::now() + ALLOW_FOR));
    }

    /// Forgets what a friend's knocks allowed (they were removed).
    fn disallow(&self, friend: &PublicKey) {
        self.allowed.lock().expect("not poisoned").remove(friend);
    }

    /// Closes every connection of `key`.
    fn disconnect(&self, key: &PublicKey, why: &'static [u8]) {
        let connections = self.connections.lock().expect("not poisoned");
        for connected in connections.live.values().filter(|c| &c.key == key) {
            connected.connection.close(2u32.into(), why);
        }
    }
}

/// Drops the knocks whose time to connect is over.
fn forget_expired(allowed: &mut HashMap<PublicKey, VecDeque<(Vec<IpAddr>, Instant)>>) {
    let now = Instant::now();
    allowed.retain(|_, knocks| {
        knocks.retain(|(_, until)| *until > now);
        !knocks.is_empty()
    });
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// A core sharing with its friends. Dropping it stops, without waiting.
pub struct FriendHost {
    shared: Arc<Shared>,
    stop: watch::Sender<bool>,
    endpoint: quinn::Endpoint,
    tasks: Vec<JoinHandle<()>>,
}

impl FriendHost {
    /// Starts: collects this computer's addresses, publishes a record for each friend
    /// and waits for knocks in the background. Keeps trying the service while it can't
    /// be reached.
    pub async fn start(
        identity: &CoreIdentity,
        friends: Vec<FriendAccess>,
        config: FriendHostConfig,
        handler: impl FriendHandler,
    ) -> Result<FriendHost> {
        let server = tunnel::server_config_for(&identity.cert_der, &identity.key_der, FRIEND_ALPN)?;
        let gathered = gather::gather_async(config.gather).await?;
        let candidates = gathered.addrs();
        let sprayer = gathered.socket.try_clone()?;
        let stunner = gathered.socket.try_clone()?;
        let lan_v4 = gathered.candidates.iter().find_map(|c| match c {
            Candidate {
                kind: CandidateKind::LocalNetwork,
                addr: SocketAddr::V4(addr),
            } => Some(*addr.ip()),
            _ => None,
        });
        let shared = Arc::new(Shared {
            fingerprint: identity.fingerprint(),
            service: config.service,
            friends: Mutex::new(friends.into_iter().map(|f| (f.key, f.secret)).collect()),
            generation: watch::channel(0).0,
            allowed: Mutex::new(HashMap::new()),
            connections: Mutex::new(Connections::default()),
            peers: Mutex::new(HashMap::new()),
            next_connection: AtomicU64::new(1),
            status: Mutex::new(FriendHostStatus {
                state: HostState::Reconnecting,
                publishing: 0,
                candidates: gathered.candidates.clone(),
                symmetric_nat: gathered.symmetric,
                connected: Vec::new(),
                problem: None,
            }),
            changed: watch::channel(0).0,
            mapping: Mutex::new(gathered.mapping),
            candidates: Mutex::new(candidates),
        });
        let admit = {
            let shared = shared.clone();
            Arc::new(move |ip| shared.may_reach(ip))
        };
        let (endpoint, gate) = Gate::endpoint(gathered.socket, server, admit)?;
        let (stop, stop_rx) = watch::channel(false);
        let handler: Arc<dyn FriendHandler> = Arc::new(handler);
        let tasks = vec![
            tokio::spawn(accept_loop(endpoint.clone(), shared.clone(), handler)),
            tokio::spawn(service_loop(shared.clone(), sprayer, stop_rx.clone())),
            tokio::spawn(refresh_loop(
                shared.clone(),
                gate,
                stunner,
                config.gather,
                lan_v4,
                stop_rx,
            )),
        ];
        Ok(FriendHost {
            shared,
            stop,
            endpoint,
            tasks,
        })
    }

    /// The certificate fingerprint friends pin.
    pub fn fingerprint(&self) -> [u8; 32] {
        self.shared.fingerprint
    }

    /// Replaces the friend list. Friends no longer on it are disconnected at once, and
    /// the records are published again.
    pub fn set_friends(&self, friends: Vec<FriendAccess>) {
        let removed: Vec<PublicKey> = {
            let mut current = self.shared.friends.lock().expect("not poisoned");
            let next: HashMap<PublicKey, Secret> =
                friends.into_iter().map(|f| (f.key, f.secret)).collect();
            let removed = current
                .keys()
                .filter(|key| !next.contains_key(key))
                .copied()
                .collect();
            *current = next;
            removed
        };
        for key in &removed {
            self.shared.disallow(key);
            self.shared.disconnect(key, b"removed");
        }
        self.shared.generation.send_modify(|n| *n += 1);
    }

    /// Closes `friend`'s connections (something was unshared with them). They can
    /// connect again for what's still shared.
    pub fn disconnect(&self, friend: &PublicKey) {
        self.shared.disconnect(friend, b"unshared");
    }

    /// How sharing is going, and who is connected.
    pub fn status(&self) -> FriendHostStatus {
        let mut status = self.shared.status.lock().expect("not poisoned").clone();
        let connections = self.shared.connections.lock().expect("not poisoned");
        let mut connected: Vec<ConnectedFriend> = connections
            .live
            .values()
            .map(|c| {
                let address = canonical(c.connection.remote_address());
                ConnectedFriend {
                    key: c.key.to_hex(),
                    address,
                    kind: ConnectionKind::of(address),
                    connected_at: c
                        .connected_at
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    rtt_ms: c.connection.rtt().as_millis() as u64,
                }
            })
            .collect();
        connected.sort_by_key(|c| c.connected_at);
        status.connected = connected;
        status
    }

    /// Changes whenever the status does.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.shared.changed.subscribe()
    }

    /// Stops: ends the session at the service (the records go at once), disconnects
    /// friends and removes the router's forwarding.
    pub async fn stop(mut self) {
        let _ = self.stop.send(true);
        self.endpoint.close(0u32.into(), b"stopped");
        // The service task ends the session when it sees the stop; give it a moment.
        let mut tasks: Vec<JoinHandle<()>> = self.tasks.drain(..).collect();
        let service = tasks.remove(1);
        for task in tasks {
            task.abort();
        }
        let abort = service.abort_handle();
        if tokio::time::timeout(Duration::from_secs(5), service)
            .await
            .is_err()
        {
            abort.abort();
        }
        let mapping = self.shared.mapping.lock().expect("not poisoned").take();
        if let Some(mapping) = mapping {
            let _ = tokio::task::spawn_blocking(move || mapping.remove()).await;
        }
        self.shared
            .connections
            .lock()
            .expect("not poisoned")
            .live
            .clear();
        self.shared
            .update(|status| status.state = HostState::Stopped);
    }
}

impl Drop for FriendHost {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
        self.endpoint.close(0u32.into(), b"stopped");
        for task in self.tasks.drain(..) {
            task.abort();
        }
        if let Some(mapping) = self.shared.mapping.lock().expect("not poisoned").take() {
            std::thread::spawn(move || mapping.remove());
        }
    }
}

// ---------------------------------------------------------------------------
// The service

/// Each friend's records for now, and which friend each lookup id belongs to.
fn records(
    shared: &Shared,
    now: u64,
) -> (
    Vec<([u8; ID_LEN], Vec<u8>)>,
    HashMap<[u8; ID_LEN], PublicKey>,
    Duration,
) {
    let candidates = shared.candidates.lock().expect("not poisoned").clone();
    let friends = shared.friends.lock().expect("not poisoned");
    let mut records = Vec::new();
    let mut index = HashMap::new();
    let mut renew = RENEW_AT_MOST;
    for (key, secret) in friends.iter() {
        for lookup in meet::publish_lookups(secret, now) {
            match meet::seal_record(secret, &lookup, &candidates, now) {
                Ok(sealed) => {
                    index.insert(lookup, *key);
                    records.push((lookup, sealed));
                }
                Err(err) => tracing::debug!("friends: couldn't seal a record: {err}"),
            }
        }
        renew = renew.min(Duration::from_secs(
            meet::secs_to_republish(secret, now).max(1),
        ));
    }
    (records, index, renew)
}

/// Keeps the records published, hears knocks, and punches towards the friends who sent
/// them.
async fn service_loop(shared: Arc<Shared>, sprayer: UdpSocket, mut stop: watch::Receiver<bool>) {
    let mut generation = shared.generation.subscribe();
    let mut session: Option<MeetSession> = None;
    let mut watch = None;
    let mut guards: HashMap<PublicKey, ReplayGuard> = HashMap::new();
    let mut sprays: VecDeque<(PublicKey, JoinHandle<()>)> = VecDeque::new();
    let mut backoff = Duration::from_secs(2);
    loop {
        if *stop.borrow() {
            break;
        }
        let now = now_secs();
        let (records, index, renew) = records(&shared, now);
        shared.update(|status| {
            status.publishing = index
                .values()
                .collect::<std::collections::HashSet<_>>()
                .len()
        });
        if records.is_empty() {
            if let Some(old) = session.take() {
                let _ = shared.service.meet_close(&old).await;
            }
            watch = None;
            shared.update(|status| {
                status.state = HostState::Sharing;
                status.problem = None;
            });
            tokio::select! {
                _ = stop.changed() => break,
                _ = generation.changed() => continue,
            }
        }
        // publish: update the session, or open one
        let published = match &session {
            Some(open) => match shared.service.meet_update(open, &records).await {
                Ok(()) => Ok(()),
                Err(Error::Service {
                    status: 403 | 404, ..
                }) => {
                    session = None;
                    watch = None;
                    continue;
                }
                Err(err) => Err(err),
            },
            None => match shared.service.meet_open(&records).await {
                Ok(opened) => {
                    session = Some(opened);
                    watch = None;
                    Ok(())
                }
                Err(err) => Err(err),
            },
        };
        match published {
            Ok(()) => {}
            // The service refused these records (too many friends, too large): trying
            // again won't help. Say so, and wait for the friend list to change.
            Err(
                err @ Error::Service {
                    status: 400 | 413, ..
                },
            ) => {
                tracing::warn!("friends: the service refused the records: {err}");
                shared.update(|status| {
                    status.state = HostState::Reconnecting;
                    status.problem = Some(err.to_string());
                });
                tokio::select! {
                    _ = stop.changed() => break,
                    _ = generation.changed() => continue,
                }
            }
            Err(err) => {
                wait_or_stop(&shared, &err, &mut backoff, &mut stop).await;
                continue;
            }
        }
        if watch.is_none() {
            let open = session.as_ref().expect("opened above");
            match shared.service.meet_watch(open).await {
                Ok(Some(opened)) => watch = Some(opened),
                Ok(None) => {
                    session = None;
                    continue;
                }
                Err(err) => {
                    wait_or_stop(&shared, &err, &mut backoff, &mut stop).await;
                    continue;
                }
            }
        }
        shared.update(|status| {
            status.state = HostState::Sharing;
            status.problem = None;
        });
        backoff = Duration::from_secs(2);
        let renew_at = tokio::time::Instant::now() + renew;
        let listening = watch.as_mut().expect("watching");
        loop {
            tokio::select! {
                _ = stop.changed() => {
                    if let Some(open) = session.take() {
                        let _ = shared.service.meet_close(&open).await;
                    }
                    return;
                }
                _ = generation.changed() => break,
                _ = tokio::time::sleep_until(renew_at) => break,
                event = listening.next() => match event {
                    Ok(MeetEvent::Knocks(knocks)) => {
                        for knock in knocks {
                            on_knock(&shared, &index, knock, &mut guards, &mut sprays, &sprayer, &stop);
                        }
                    }
                    Ok(MeetEvent::Ended) => {
                        session = None;
                        watch = None;
                        break;
                    }
                    Err(err) => {
                        watch = None;
                        wait_or_stop(&shared, &err, &mut backoff, &mut stop).await;
                        break;
                    }
                },
            }
        }
    }
    if let Some(open) = session.take() {
        let _ = shared.service.meet_close(&open).await;
    }
}

/// Opens a knock, and if it's a friend's and fresh, lets its addresses connect for a
/// while and punches towards them: a spray for each of a friend's latest
/// [`KNOCKS_PER_FRIEND`] knocks, at most [`MAX_SPRAYS`] in all.
fn on_knock(
    shared: &Shared,
    index: &HashMap<[u8; ID_LEN], PublicKey>,
    delivered: DeliveredKnock,
    guards: &mut HashMap<PublicKey, ReplayGuard>,
    sprays: &mut VecDeque<(PublicKey, JoinHandle<()>)>,
    sprayer: &UdpSocket,
    stop: &watch::Receiver<bool>,
) {
    let Some(key) = index.get(&delivered.lookup) else {
        return;
    };
    let Some(secret) = shared
        .friends
        .lock()
        .expect("not poisoned")
        .get(key)
        .cloned()
    else {
        return;
    };
    let Some(knock) = meet::open_knock(&secret, &delivered.lookup, &delivered.sealed) else {
        return;
    };
    // one guard per friend: a friend filling theirs only stops their own knocks
    guards.retain(|friend, _| shared.is_friend(friend));
    if !guards.entry(*key).or_default().admit(&knock, now_secs()) {
        return;
    }
    shared.allow(*key, knock.candidates.iter().map(|addr| addr.ip()));
    // a few sprays per friend: a new knock replaces their oldest
    sprays.retain(|(_, spray)| !spray.is_finished());
    while sprays.iter().filter(|(friend, _)| friend == key).count() >= KNOCKS_PER_FRIEND {
        let oldest = sprays
            .iter()
            .position(|(friend, _)| friend == key)
            .expect("counted above");
        if let Some((_, spray)) = sprays.remove(oldest) {
            spray.abort();
        }
    }
    while sprays.len() >= MAX_SPRAYS {
        if let Some((_, oldest)) = sprays.pop_front() {
            oldest.abort();
        }
    }
    if let Ok(socket) = sprayer.try_clone() {
        let tag = probe_tag(&secret, &knock.id);
        sprays.push_back((
            *key,
            tokio::spawn(probe::spray(
                socket,
                knock.candidates,
                tag,
                SPRAY_FOR,
                stop.clone(),
            )),
        ));
    }
}

async fn wait_or_stop(
    shared: &Shared,
    err: &Error,
    backoff: &mut Duration,
    stop: &mut watch::Receiver<bool>,
) {
    tracing::debug!("friends: {err}; trying again in {backoff:?}");
    shared.update(|status| {
        status.state = HostState::Reconnecting;
        status.problem = Some(err.to_string());
    });
    tokio::select! {
        _ = tokio::time::sleep(*backoff) => {}
        _ = stop.changed() => {}
    }
    *backoff = (*backoff * 2).min(Duration::from_secs(60));
}

/// Every [`REFRESH_EVERY`], collects this computer's addresses again and renews the
/// router's forwarding ([`refresh`]).
async fn refresh_loop(
    shared: Arc<Shared>,
    gate: Arc<Gate>,
    socket: UdpSocket,
    options: GatherOptions,
    mut lan_v4: Option<Ipv4Addr>,
    mut stop: watch::Receiver<bool>,
) {
    if options.loopback_only {
        return;
    }
    let socket = Arc::new(socket);
    loop {
        tokio::select! {
            _ = tokio::time::sleep(REFRESH_EVERY) => {}
            _ = stop.changed() => return,
        }
        let (shared, gate, socket, stop) =
            (shared.clone(), gate.clone(), socket.clone(), stop.clone());
        match tokio::task::spawn_blocking(move || {
            refresh(&shared, &gate, &socket, options, lan_v4, &stop)
        })
        .await
        {
            Ok(now) => lan_v4 = now,
            Err(_) => return,
        }
    }
}

/// Collects this computer's addresses again: its network or public address may have
/// changed since the host started. Renews the router's forwarding, or asks for it again
/// when renewing fails or this computer is on another network now (`lan_v4` was its
/// address on the local network). Publishes the records again if the addresses changed.
/// Returns the address on the local network now. Blocking.
fn refresh(
    shared: &Shared,
    gate: &Gate,
    socket: &UdpSocket,
    options: GatherOptions,
    lan_v4: Option<Ipv4Addr>,
    stop: &watch::Receiver<bool>,
) -> Option<Ipv4Addr> {
    let Ok(port) = socket.local_addr().map(|addr| addr.port()) else {
        return lan_v4;
    };
    let found = gather::addresses(port, |server, v6| gate.mapped_address(socket, server, v6));
    let old = shared.mapping.lock().expect("not poisoned").take();
    let mapping = match (old, found.lan_v4) {
        (Some(old), Some(lan)) if old.local_ip() == lan => match old.renew() {
            Ok(()) => Some(old),
            Err(err) => {
                tracing::debug!("friends: couldn't renew the router's forwarding: {err}");
                old.remove();
                gather::forward(port, lan)
            }
        },
        // on another network now
        (Some(old), lan) => {
            old.remove();
            lan.and_then(|lan| gather::forward(port, lan))
        }
        (None, Some(lan)) if options.portmap && found.lan_v4 != lan_v4 => {
            gather::forward(port, lan)
        }
        (None, _) => None,
    };
    let mut candidates = found.candidates;
    if let Some(mapping) = &mapping {
        gather::add(
            &mut candidates,
            mapping.external,
            CandidateKind::RouterForwarded,
        );
    }
    {
        // `stop` says so before the host takes the mapping to remove it: if it hasn't,
        // the host will find this one.
        let mut slot = shared.mapping.lock().expect("not poisoned");
        if *stop.borrow() {
            drop(slot);
            if let Some(mapping) = mapping {
                mapping.remove();
            }
            return found.lan_v4;
        }
        *slot = mapping;
    }
    // offline for now: keep the old addresses
    if candidates.is_empty() {
        return found.lan_v4;
    }
    let addrs: Vec<SocketAddr> = candidates.iter().map(|c| c.addr).collect();
    let changed = {
        let mut current = shared.candidates.lock().expect("not poisoned");
        let changed = *current != addrs;
        *current = addrs;
        changed
    };
    shared.update(|status| {
        status.candidates = candidates;
        status.symmetric_nat = found.symmetric;
    });
    if changed {
        tracing::debug!("friends: this computer's addresses changed; publishing them again");
        shared.generation.send_modify(|n| *n += 1);
    }
    found.lan_v4
}

// ---------------------------------------------------------------------------
// Connections

async fn accept_loop(
    endpoint: quinn::Endpoint,
    shared: Arc<Shared>,
    handler: Arc<dyn FriendHandler>,
) {
    while let Some(incoming) = endpoint.accept().await {
        let ip = canonical(incoming.remote_address()).ip();
        if !incoming.remote_address_validated() {
            // Silence for anyone without an accepted knock: not even a retry, which
            // would show a server is here.
            if shared.allowed_by(ip).is_empty() {
                incoming.ignore();
            } else if let Err(err) = incoming.retry() {
                err.into_incoming().ignore();
            }
            continue;
        }
        let unproven = match shared.reserve(ip) {
            Ok(unproven) => unproven,
            Err(Refusal::Stranger) => {
                incoming.ignore();
                continue;
            }
            Err(Refusal::Full) => {
                incoming.refuse();
                continue;
            }
        };
        let peer = shared.hold_peer(ip);
        let shared = shared.clone();
        let handler = handler.clone();
        tokio::spawn(async move {
            // kept for as long as the connection is
            let _peer = peer;
            // The handshake and the hello together, so an unproven place is held only
            // briefly.
            let proven = tokio::time::timeout(HELLO_WITHIN, async {
                let connection = match incoming.await {
                    Ok(connection) => connection,
                    Err(err) => {
                        tracing::debug!("friends: a connection attempt failed: {err}");
                        return None;
                    }
                };
                let key = hello(&connection, &shared).await;
                Some((connection, key))
            })
            .await;
            let (connection, key) = match proven {
                Ok(Some((connection, Some(key)))) => (connection, key),
                Ok(Some((connection, None))) => {
                    connection.close(1u32.into(), b"not a friend");
                    return;
                }
                // A connection still in its handshake or hello closes as it drops.
                Ok(None) | Err(_) => return,
            };
            serve_connection(connection, key, unproven, shared, handler).await;
        });
    }
}

/// Reads the friend hello from the connection's first stream. Returns the friend's key
/// if it's a friend's and the signature is good.
async fn hello(connection: &quinn::Connection, shared: &Shared) -> Option<PublicKey> {
    let (mut send, mut recv) = connection.accept_bi().await.ok()?;
    let mut message = [0u8; 1 + 32 + SIGNATURE_LEN];
    recv.read_exact(&mut message).await.ok()?;
    if message[0] != FriendStreamKind::Hello as u8 {
        return None;
    }
    let key = PublicKey(message[1..33].try_into().ok()?);
    let mut exporter = [0u8; 32];
    connection
        .export_keying_material(&mut exporter, HELLO_LABEL, b"")
        .ok()?;
    if !key.verify(
        HELLO_CONTEXT,
        &hello_message(&exporter, &shared.fingerprint, &key),
        &message[33..],
    ) || !shared.is_friend(&key)
    {
        return None;
    }
    send.write_all(&[HELLO_ANSWER]).await.ok()?;
    let _ = send.finish();
    Some(key)
}

async fn serve_connection(
    connection: quinn::Connection,
    key: PublicKey,
    unproven: Unproven,
    shared: Arc<Shared>,
    handler: Arc<dyn FriendHandler>,
) {
    let Some(id) = shared.admit(unproven, key, &connection) else {
        connection.close(3u32.into(), b"too many connections");
        return;
    };
    shared.changed.send_modify(|n| *n += 1);
    // removed between the hello and now
    if !shared.is_friend(&key) {
        connection.close(2u32.into(), b"removed");
    }
    loop {
        let (send, mut recv) = match connection.accept_bi().await {
            Ok(streams) => streams,
            Err(_) => break,
        };
        let connection = connection.clone();
        let shared = shared.clone();
        let handler = handler.clone();
        tokio::spawn(async move {
            let close = |why: &'static [u8]| connection.close(1u32.into(), why);
            let mut kind = [0u8; 1];
            if !matches!(
                tokio::time::timeout(HEADER_WITHIN, recv.read_exact(&mut kind)).await,
                Ok(Ok(()))
            ) {
                return close(b"no header");
            }
            if !shared.is_friend(&key) {
                return close(b"removed");
            }
            match kind[0] {
                k if k == FriendStreamKind::Info as u8 => {
                    handler.info(tokio::io::join(recv, send), key);
                }
                k if k == FriendStreamKind::Game as u8 => {
                    let mut len = [0u8; 1];
                    if recv.read_exact(&mut len).await.is_err()
                        || len[0] == 0
                        || usize::from(len[0]) > MAX_INSTANCE_LEN
                    {
                        return close(b"bad header");
                    }
                    let mut name = vec![0u8; usize::from(len[0])];
                    if recv.read_exact(&mut name).await.is_err() {
                        return close(b"bad header");
                    }
                    let Ok(instance) = String::from_utf8(name) else {
                        return close(b"bad header");
                    };
                    handler.game(tokio::io::join(recv, send), key, instance);
                }
                // invite kinds (0-2), a second hello, or anything else
                _ => close(b"bad kind"),
            }
        });
    }
    if shared
        .connections
        .lock()
        .expect("not poisoned")
        .live
        .remove(&id)
        .is_some()
    {
        shared.changed.send_modify(|n| *n += 1);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn key(byte: u8) -> PublicKey {
        PublicKey([byte; 32])
    }

    /// A friend host's shared state with these friends, and no sockets.
    fn shared_with(friends: &[PublicKey]) -> Arc<Shared> {
        Arc::new(Shared {
            fingerprint: [0; 32],
            service: Service::default(),
            friends: Mutex::new(
                friends
                    .iter()
                    .map(|key| (*key, Secret::random().unwrap()))
                    .collect(),
            ),
            generation: watch::channel(0).0,
            allowed: Mutex::new(HashMap::new()),
            connections: Mutex::new(Connections::default()),
            peers: Mutex::new(HashMap::new()),
            next_connection: AtomicU64::new(1),
            status: Mutex::new(FriendHostStatus {
                state: HostState::Sharing,
                publishing: 0,
                candidates: Vec::new(),
                symmetric_nat: false,
                connected: Vec::new(),
                problem: None,
            }),
            changed: watch::channel(0).0,
            mapping: Mutex::new(None),
            candidates: Mutex::new(Vec::new()),
        })
    }

    #[test]
    fn a_friends_addresses_take_only_that_friends_places() {
        let (dee, ben) = (key(1), key(2));
        let shared = shared_with(&[dee, ben]);
        let crowd: IpAddr = "198.18.0.7".parse().unwrap();
        let bens: IpAddr = "198.18.0.8".parse().unwrap();
        assert_eq!(shared.reserve(crowd).err(), Some(Refusal::Stranger));
        // Dee's knock names an address anyone there can connect from
        shared.allow(dee, [crowd]);
        let held: Vec<Unproven> = (0..MAX_CONNECTIONS_PER_FRIEND)
            .map(|_| shared.reserve(crowd).unwrap())
            .collect();
        assert_eq!(shared.reserve(crowd).err(), Some(Refusal::Full));
        // ... which leaves Ben's places alone
        shared.allow(ben, [bens]);
        let bens_place = shared.reserve(bens).unwrap();
        assert_eq!(bens_place.friend, Some(ben));
        // an address both knocks name goes to whoever has a place left
        shared.allow(ben, [bens, crowd]);
        assert_eq!(shared.reserve(crowd).unwrap().friend, Some(ben));
        // dropping a place frees it
        drop(held);
        assert_eq!(shared.connections.lock().unwrap().held_by(&dee), 0);
        assert_eq!(shared.reserve(crowd).unwrap().friend, Some(dee));
        drop(bens_place);
        assert_eq!(shared.connections.lock().unwrap().unproven_total(), 0);
    }

    /// A knock from `friend` (on `shared`) with these addresses, as the service delivers
    /// it, and the lookup index it needs.
    fn knock_from(
        shared: &Shared,
        friend: PublicKey,
        candidates: &[SocketAddr],
    ) -> (DeliveredKnock, HashMap<[u8; ID_LEN], PublicKey>) {
        let secret = shared.friends.lock().unwrap()[&friend].clone();
        let lookup = meet::publish_lookups(&secret, now_secs())[0];
        let (_, sealed) = meet::seal_knock(&secret, &lookup, candidates, now_secs()).unwrap();
        let delivered = DeliveredKnock {
            lookup,
            sealed,
            seen: None,
        };
        (delivered, HashMap::from([(lookup, friend)]))
    }

    #[tokio::test]
    async fn a_friends_latest_knocks_each_get_a_spray_and_their_addresses() {
        let dee = key(1);
        let shared = shared_with(&[dee]);
        let sprayer = UdpSocket::bind("127.0.0.1:0").unwrap();
        sprayer.set_nonblocking(true).unwrap();
        let (_stop, stop) = watch::channel(false);
        let mut guards = HashMap::new();
        let mut sprays = VecDeque::new();
        // nothing listens on these ports
        let joins: Vec<SocketAddr> = (1..=3)
            .map(|n| SocketAddr::from(([127, 0, 0, n], 9)))
            .collect();
        for (n, join) in joins.iter().enumerate() {
            let (knock, index) = knock_from(&shared, dee, &[*join]);
            on_knock(
                &shared,
                &index,
                knock,
                &mut guards,
                &mut sprays,
                &sprayer,
                &stop,
            );
            assert_eq!(sprays.len(), (n + 1).min(KNOCKS_PER_FRIEND));
        }
        // the two latest joins both keep their spray and their addresses
        assert!(sprays.iter().all(|(_, spray)| !spray.is_finished()));
        assert!(shared.may_reach(joins[1].ip()));
        assert!(shared.may_reach(joins[2].ip()));
        assert!(!shared.may_reach(joins[0].ip()), "the oldest knock's went");
        for (_, spray) in sprays {
            spray.abort();
        }
    }

    #[test]
    fn only_allowed_addresses_and_connections_reach_quic() {
        let dee = key(1);
        let shared = shared_with(&[dee]);
        let knocked: IpAddr = "198.18.0.7".parse().unwrap();
        let stranger: IpAddr = "198.18.0.9".parse().unwrap();
        assert!(!shared.may_reach(knocked));
        shared.allow(dee, [knocked]);
        assert!(shared.may_reach(knocked));
        assert!(!shared.may_reach(stranger));
        // a connection outlasts its knock
        let hold = shared.hold_peer(knocked);
        let second = shared.hold_peer(knocked);
        shared.disallow(&dee);
        assert!(shared.may_reach(knocked));
        drop(hold);
        assert!(shared.may_reach(knocked));
        drop(second);
        assert!(!shared.may_reach(knocked));
    }

    #[test]
    fn unproven_places_are_capped_for_the_whole_host() {
        let friends: Vec<PublicKey> = (0..10).map(key).collect();
        let shared = shared_with(&friends);
        let mut held = Vec::new();
        for (n, friend) in friends.iter().enumerate() {
            let ip = IpAddr::from([198, 18, 1, n as u8]);
            shared.allow(*friend, [ip]);
            while let Ok(place) = shared.reserve(ip) {
                held.push(place);
            }
        }
        assert_eq!(held.len(), MAX_UNPROVEN);
    }

    #[test]
    fn identities_round_trip() {
        let identity = CoreIdentity::generate().unwrap();
        let back = CoreIdentity::from_json(&identity.to_json()).unwrap();
        assert_eq!(back.fingerprint(), identity.fingerprint());
        assert!(!format!("{identity:?}").contains(&hex(&identity.key_der)));
        assert!(CoreIdentity::from_json("{}").is_err());
    }
}
