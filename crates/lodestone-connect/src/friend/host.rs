//! A core's friend host (launcher/docs/friends.md, "Finding a friend's core" and
//! "Joining"): publishes one sealed record per friend at the service, punches towards
//! friends who knock, and serves their QUIC connections.
//!
//! - It has its own certificate ([`CoreIdentity`]), socket and QUIC protocol
//!   ([`FRIEND_ALPN`]), apart from every invite host.
//! - It answers QUIC only from addresses in a knock it accepted in the last
//!   [`ALLOW_FOR`]; everyone else gets silence, so a removed friend who remembers the
//!   core's address learns nothing.
//! - The first stream of a connection must be the friend hello: the friend's key and a
//!   signature over this connection's TLS exporter. Until then nothing else is read.
//! - Every later stream is checked against the friend list as it is then.

use std::{
    collections::{HashMap, VecDeque},
    fmt,
    net::{IpAddr, SocketAddr, UdpSocket},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::{sync::watch, task::JoinHandle};

use super::{
    client::{DeliveredKnock, MeetEvent, MeetSession},
    crypto::{probe_tag, PublicKey, Secret, ID_LEN, SIGNATURE_LEN},
    meet::{self, ReplayGuard},
    protocol::{hello_message, FriendStreamKind, HELLO_CONTEXT, HELLO_LABEL, MAX_INSTANCE_LEN},
};
use crate::{
    addr::{canonical, ConnectionKind},
    gather::{self, Candidate, GatherOptions},
    hex,
    host::{HostState, UnprovenSlot, MAX_FRIENDS, MAX_SPRAYS},
    portmap, probe,
    service::Service,
    tunnel::{self, TunnelStream, FRIEND_ALPN, HELLO_ANSWER, PROVE_WITHIN},
    unhex, Error, Result,
};

/// How long an accepted knock lets its addresses connect.
pub const ALLOW_FOR: Duration = Duration::from_secs(120);
/// How long the core punches towards a friend who knocked.
const SPRAY_FOR: Duration = Duration::from_secs(60);
/// Records are renewed at least this often, well within [`meet::RECORD_MAX_AGE_SECS`].
const RENEW_AT_MOST: Duration = Duration::from_secs(6 * 60 * 60);
/// How often the router's forwarding is renewed (it lasts an hour).
const PORTMAP_EVERY: Duration = Duration::from_secs(20 * 60);
/// How long a stream has to send its header.
const HEADER_WITHIN: Duration = Duration::from_secs(10);
/// The most addresses one friend's latest knock opens (its candidates).
const MAX_ALLOWED_PER_FRIEND: usize = meet::MAX_CANDIDATES;
/// The most connections one friend may hold at once, so no friend can take every place.
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

struct Shared {
    fingerprint: [u8; 32],
    service: Service,
    friends: Mutex<HashMap<PublicKey, Secret>>,
    /// Bumped whenever the friend list changes, so the records are published again.
    generation: watch::Sender<u64>,
    /// For each friend, the addresses of their latest accepted knock and until when
    /// they may connect. A new knock replaces the friend's addresses, so one friend can
    /// never crowd out another.
    allowed: Mutex<HashMap<PublicKey, (Vec<IpAddr>, Instant)>>,
    connections: Mutex<HashMap<u64, Connected>>,
    next_connection: AtomicU64,
    status: Mutex<FriendHostStatus>,
    changed: watch::Sender<u64>,
    mapping: Mutex<Option<portmap::Mapping>>,
    candidates: Vec<SocketAddr>,
}

impl Shared {
    fn update(&self, change: impl FnOnce(&mut FriendHostStatus)) {
        change(&mut self.status.lock().expect("not poisoned"));
        self.changed.send_modify(|n| *n += 1);
    }

    fn is_friend(&self, key: &PublicKey) -> bool {
        self.friends.lock().expect("not poisoned").contains_key(key)
    }

    fn is_allowed(&self, ip: IpAddr) -> bool {
        let mut allowed = self.allowed.lock().expect("not poisoned");
        let now = Instant::now();
        allowed.retain(|_, (_, until)| *until > now);
        allowed.values().any(|(ips, _)| ips.contains(&ip))
    }

    /// Lets `friend` connect from `ips` (their knock's candidates) for [`ALLOW_FOR`],
    /// replacing what their previous knock allowed. The address the service says it
    /// saw isn't used: the service could name any address.
    fn allow(&self, friend: PublicKey, ips: impl IntoIterator<Item = IpAddr>) {
        let mut list: Vec<IpAddr> = Vec::new();
        for ip in ips {
            let ip = canonical(SocketAddr::new(ip, 0)).ip();
            if !list.contains(&ip) && list.len() < MAX_ALLOWED_PER_FRIEND {
                list.push(ip);
            }
        }
        let mut allowed = self.allowed.lock().expect("not poisoned");
        let now = Instant::now();
        allowed.retain(|_, (_, until)| *until > now);
        allowed.insert(friend, (list, now + ALLOW_FOR));
    }

    /// Forgets what a friend's knocks allowed (they were removed).
    fn disallow(&self, friend: &PublicKey) {
        self.allowed.lock().expect("not poisoned").remove(friend);
    }

    fn connections_of(&self, friend: &PublicKey) -> usize {
        self.connections
            .lock()
            .expect("not poisoned")
            .values()
            .filter(|c| &c.key == friend)
            .count()
    }

    /// Closes every connection of `key`.
    fn disconnect(&self, key: &PublicKey, why: &'static [u8]) {
        let connections = self.connections.lock().expect("not poisoned");
        for connected in connections.values().filter(|c| &c.key == key) {
            connected.connection.close(2u32.into(), why);
        }
    }
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
        let shared = Arc::new(Shared {
            fingerprint: identity.fingerprint(),
            service: config.service,
            friends: Mutex::new(friends.into_iter().map(|f| (f.key, f.secret)).collect()),
            generation: watch::channel(0).0,
            allowed: Mutex::new(HashMap::new()),
            connections: Mutex::new(HashMap::new()),
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
            candidates,
        });
        let endpoint = tunnel::endpoint(gathered.socket, Some(server))?;
        let (stop, stop_rx) = watch::channel(false);
        let handler: Arc<dyn FriendHandler> = Arc::new(handler);
        let tasks = vec![
            tokio::spawn(accept_loop(endpoint.clone(), shared.clone(), handler)),
            tokio::spawn(service_loop(shared.clone(), sprayer, stop_rx.clone())),
            tokio::spawn(portmap_loop(shared.clone(), stop_rx)),
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
    let friends = shared.friends.lock().expect("not poisoned");
    let mut records = Vec::new();
    let mut index = HashMap::new();
    let mut renew = RENEW_AT_MOST;
    for (key, secret) in friends.iter() {
        for lookup in meet::publish_lookups(secret, now) {
            match meet::seal_record(secret, &lookup, &shared.candidates, now) {
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
/// while and punches towards them: one spray per friend, at most [`MAX_SPRAYS`].
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
    // one spray per friend: a new knock replaces their last one
    for (friend, spray) in sprays.iter() {
        if friend == key {
            spray.abort();
        }
    }
    sprays.retain(|(friend, spray)| !spray.is_finished() && friend != key);
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

async fn portmap_loop(shared: Arc<Shared>, mut stop: watch::Receiver<bool>) {
    loop {
        tokio::select! {
            _ = tokio::time::sleep(PORTMAP_EVERY) => {}
            _ = stop.changed() => return,
        }
        let shared = shared.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Some(mapping) = shared.mapping.lock().expect("not poisoned").as_ref() {
                if let Err(err) = mapping.renew() {
                    tracing::debug!("friends: couldn't renew the router's forwarding: {err}");
                }
            }
        })
        .await;
    }
}

// ---------------------------------------------------------------------------
// Connections

async fn accept_loop(
    endpoint: quinn::Endpoint,
    shared: Arc<Shared>,
    handler: Arc<dyn FriendHandler>,
) {
    let unproven = Arc::new(AtomicUsize::new(0));
    while let Some(incoming) = endpoint.accept().await {
        // Silence for anyone without an accepted knock: not even a retry or a refusal,
        // which would show a server is here.
        if !shared.is_allowed(canonical(incoming.remote_address()).ip()) {
            incoming.ignore();
            continue;
        }
        if !incoming.remote_address_validated() {
            if let Err(err) = incoming.retry() {
                err.into_incoming().ignore();
            }
            continue;
        }
        if shared.connections.lock().expect("not poisoned").len() >= MAX_FRIENDS {
            incoming.refuse();
            continue;
        }
        let Some(slot) = UnprovenSlot::take(&unproven) else {
            incoming.refuse();
            continue;
        };
        let shared = shared.clone();
        let handler = handler.clone();
        tokio::spawn(async move {
            let connection = match incoming.await {
                Ok(connection) => connection,
                Err(err) => return tracing::debug!("friends: a connection attempt failed: {err}"),
            };
            serve_connection(connection, slot, shared, handler).await;
        });
    }
}

/// Reads the friend hello from the connection's first stream. Returns the friend's key
/// if it's a friend's and the signature is good.
async fn hello(connection: &quinn::Connection, shared: &Shared) -> Option<PublicKey> {
    let (mut send, mut recv) = connection.accept_bi().await.ok()?;
    let mut message = [0u8; 1 + 32 + SIGNATURE_LEN];
    tokio::time::timeout(HEADER_WITHIN, recv.read_exact(&mut message))
        .await
        .ok()?
        .ok()?;
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
    slot: UnprovenSlot,
    shared: Arc<Shared>,
    handler: Arc<dyn FriendHandler>,
) {
    let key = match tokio::time::timeout(PROVE_WITHIN, hello(&connection, &shared)).await {
        Ok(Some(key)) => key,
        _ => {
            connection.close(1u32.into(), b"not a friend");
            return;
        }
    };
    if shared.connections_of(&key) >= MAX_CONNECTIONS_PER_FRIEND {
        connection.close(3u32.into(), b"too many connections");
        return;
    }
    drop(slot);
    let id = shared.next_connection.fetch_add(1, Ordering::Relaxed);
    shared.connections.lock().expect("not poisoned").insert(
        id,
        Connected {
            key,
            connection: connection.clone(),
            connected_at: SystemTime::now(),
        },
    );
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
        .remove(&id)
        .is_some()
    {
        shared.changed.send_modify(|n| *n += 1);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn identities_round_trip() {
        let identity = CoreIdentity::generate().unwrap();
        let back = CoreIdentity::from_json(&identity.to_json()).unwrap();
        assert_eq!(back.fingerprint(), identity.fingerprint());
        assert!(!format!("{identity:?}").contains(&hex(&identity.key_der)));
        assert!(CoreIdentity::from_json("{}").is_err());
    }
}
