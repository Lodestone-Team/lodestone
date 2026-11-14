//! Sharing a server: [`Host`] keeps an invite open at the introduction service, punches
//! towards each friend who joins, and passes the streams friends open to a
//! [`StreamHandler`].
//!
//! ```no_run
//! # async fn example() -> lodestone_connect::Result<()> {
//! use lodestone_connect::host::{Forward, Host, HostConfig, HostIdentity};
//! let identity = HostIdentity::generate()?;
//! let handler = Forward { game: "127.0.0.1:25565".parse().unwrap(), info: None };
//! let host = Host::start(identity, HostConfig::default(), handler).await?;
//! println!("Share this link: {}", host.invite().link());
//! // Save `host.identity().to_json()` to keep the same link after a restart.
//! host.stop(true).await; // ends the invite: the link stops working
//! # Ok(()) }
//! ```

use std::{
    collections::{HashMap, VecDeque},
    fmt,
    net::{Ipv4Addr, SocketAddr, UdpSocket},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tokio::{
    io::AsyncWriteExt as _,
    sync::{watch, Notify},
    task::JoinHandle,
};

use crate::{
    addr::{canonical, ConnectionKind},
    friend::gate::Gate,
    gather::{self, Candidate, CandidateKind, GatherOptions},
    hex, portmap, probe,
    service::{Event, Registered, Service},
    tunnel::{self, StreamKind, TunnelStream, HELLO_ANSWER, PROVE_WITHIN, TOKEN_LEN},
    unhex, Error, Invite, Result,
};

/// How long the host punches towards a friend who joined.
const SPRAY_FOR: Duration = Duration::from_secs(60);
/// How often a kept invite is renewed (the service ends an invite after a day).
const RENEW_EVERY: Duration = Duration::from_secs(23 * 60 * 60);
/// Renew early once this many friends have joined (a kept invite takes 64 a day).
const RENEW_AFTER_JOINS: u64 = 48;
/// How often the host collects its addresses again (renewing the invite with them if
/// they changed) and renews the router's forwarding (which lasts an hour).
const REFRESH_EVERY: Duration = Duration::from_secs(10 * 60);
/// The most friends connected at once.
pub const MAX_FRIENDS: usize = 32;
/// The most connections at once that haven't shown the token yet. Anyone who reaches
/// the host's port can open one, so they are few and last [`PROVE_WITHIN`] at most.
pub const MAX_UNPROVEN: usize = 16;
/// The most joins the host probes towards at once. Anyone with the code can join, with
/// any addresses, so a new join stops the oldest probing beyond this.
pub const MAX_SPRAYS: usize = 8;
/// The most addresses of one join the host probes (the service allows 8).
pub const MAX_PEERS: usize = 8;
/// How long a stream has to send its header.
const HEADER_WITHIN: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------------
// Identity

/// What makes a host's invite links: its certificate and key, the token, and the code
/// and secret of its invite at the service. Save it ([`HostIdentity::to_json`], in a
/// file only the host's user can read) to keep the same link across restarts.
#[derive(Clone)]
pub struct HostIdentity {
    cert_der: Vec<u8>,
    key_der: Vec<u8>,
    token: [u8; TOKEN_LEN],
    /// The invite at the service, once registered.
    pub invite: Option<SavedInvite>,
}

/// An invite the service holds for this host.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedInvite {
    /// The service's base URL, such as `https://connect.lodestone.cc`.
    pub service: String,
    pub code: String,
    pub secret: String,
    /// The service keeps the code for this host (a stable link).
    #[serde(default)]
    pub kept: bool,
}

impl fmt::Debug for SavedInvite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SavedInvite")
            .field("service", &self.service)
            .field("code", &self.code)
            .field("kept", &self.kept)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for HostIdentity {
    /// Leaves out the key, token and secret.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostIdentity")
            .field("fingerprint", &hex(&self.fingerprint()))
            .field("invite", &self.invite)
            .finish_non_exhaustive()
    }
}

#[derive(Serialize, Deserialize)]
struct IdentityFile {
    version: u32,
    cert: String,
    key: String,
    token: String,
    #[serde(default)]
    invite: Option<SavedInvite>,
}

impl HostIdentity {
    /// A new certificate (ECDSA P-256) and token, and no invite yet.
    pub fn generate() -> Result<Self> {
        let made = crate::cert::self_signed(tunnel::SERVER_NAME)?;
        Ok(Self {
            cert_der: made.cert_der,
            key_der: made.key_der,
            token: crate::random()?,
            invite: None,
        })
    }

    /// The SHA-256 of the certificate, as links carry it.
    pub fn fingerprint(&self) -> [u8; 32] {
        tunnel::fingerprint(&self.cert_der)
    }

    /// The invite link for `code` at `service`.
    pub fn invite_at(&self, service: Service, code: &str) -> Invite {
        Invite {
            service,
            code: code.to_string(),
            fingerprint: self.fingerprint(),
            token: self.token,
        }
    }

    /// The identity as JSON. It holds the private key, the token and the secret: keep it
    /// where only this computer's user can read it.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&IdentityFile {
            version: 1,
            cert: hex(&self.cert_der),
            key: hex(&self.key_der),
            token: hex(&self.token),
            invite: self.invite.clone(),
        })
        .expect("serializable")
    }

    pub fn from_json(text: &str) -> Result<Self> {
        let bad =
            |what: &str| Error::Other(format!("The saved sharing identity is damaged ({what})"));
        let file: IdentityFile = serde_json::from_str(text).map_err(|_| bad("not JSON"))?;
        if file.version != 1 {
            return Err(bad("unknown version"));
        }
        let identity = Self {
            cert_der: unhex(&file.cert).ok_or_else(|| bad("certificate"))?,
            key_der: unhex(&file.key).ok_or_else(|| bad("key"))?,
            token: unhex(&file.token)
                .and_then(|token| token.try_into().ok())
                .ok_or_else(|| bad("token"))?,
            invite: file.invite,
        };
        // the key must load
        tunnel::server_config(&identity.cert_der, &identity.key_der).map_err(|_| bad("key"))?;
        Ok(identity)
    }
}

// ---------------------------------------------------------------------------
// Streams

/// A friend's connection, numbered from 1 in the order they connected.
pub type FriendId = u64;

/// What the host does with the streams friends open. Each method gets a stream whose
/// header (token and kind) has been read and checked; it should spawn a task and return.
pub trait StreamHandler: Send + Sync + 'static {
    /// Minecraft's connection: pass it to the game port (see [`pipe`]).
    fn game(&self, stream: TunnelStream, friend: FriendId);
    /// HTTP/1.1 for the info routes ([`crate::info`]). Close it if there's no info.
    fn info(&self, stream: TunnelStream, friend: FriendId);
}

/// Passes game streams to `game`, and info streams to `info` (or closes them).
#[derive(Clone, Debug)]
pub struct Forward {
    pub game: SocketAddr,
    pub info: Option<SocketAddr>,
}

impl StreamHandler for Forward {
    fn game(&self, stream: TunnelStream, _friend: FriendId) {
        tokio::spawn(pipe(stream, self.game));
    }

    fn info(&self, stream: TunnelStream, _friend: FriendId) {
        if let Some(info) = self.info {
            tokio::spawn(pipe(stream, info));
        }
    }
}

/// Copies a stream to and from a TCP connection to `target` until either side ends.
pub async fn pipe(mut stream: TunnelStream, target: SocketAddr) {
    match tokio::net::TcpStream::connect(target).await {
        Ok(mut tcp) => {
            let _ = tokio::io::copy_bidirectional(&mut tcp, &mut stream).await;
        }
        Err(err) => tracing::debug!("couldn't reach {target}: {err}"),
    }
}

// ---------------------------------------------------------------------------
// Status

/// How sharing is going.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostState {
    /// The invite is open and the host is waiting for friends.
    Sharing,
    /// The introduction service can't be reached; trying again. Friends already
    /// connected keep playing, new ones can't join meanwhile.
    Reconnecting,
    /// Stopped.
    Stopped,
}

/// A friend connected right now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendStatus {
    pub id: FriendId,
    /// The friend's address, as the tunnel sees it.
    pub address: SocketAddr,
    /// How the friend is connected.
    pub kind: ConnectionKind,
    /// When they connected, in seconds since 1970.
    pub connected_at: u64,
    /// The round trip time, in milliseconds.
    pub rtt_ms: u64,
}

/// Everything about a [`Host`] for people to see.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostStatus {
    pub state: HostState,
    /// The invite link (it carries the token: show it only to whoever may share it).
    pub link: String,
    pub code: String,
    /// The link keeps working across restarts (the service keeps the code).
    pub stable_link: bool,
    /// The addresses offered to friends. Collected again every few minutes.
    pub candidates: Vec<Candidate>,
    /// This network's router picks a new port per destination, which punching rarely
    /// gets through: friends may not be able to connect.
    pub symmetric_nat: bool,
    /// Friends who joined through the service since sharing started.
    pub joins: u64,
    /// Friends connected now.
    pub friends: Vec<FriendStatus>,
    /// The last problem, for people.
    pub problem: Option<String>,
}

// ---------------------------------------------------------------------------
// Host

/// How to share.
#[derive(Clone, Debug)]
pub struct HostConfig {
    /// The introduction service.
    pub service: Service,
    pub gather: GatherOptions,
    /// Ask the service to keep the code (a stable link). On by default.
    pub keep: bool,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            service: Service::default(),
            gather: GatherOptions::default(),
            keep: true,
        }
    }
}

struct Friend {
    connection: quinn::Connection,
    connected_at: SystemTime,
}

struct Shared {
    identity: Mutex<HostIdentity>,
    service: Service,
    keep: bool,
    status: Mutex<HostStatus>,
    friends: Mutex<HashMap<FriendId, Friend>>,
    next_friend: AtomicU64,
    mapping: Mutex<Option<portmap::Mapping>>,
    /// The addresses the invite carries. Collected again every [`REFRESH_EVERY`].
    candidates: Mutex<Vec<SocketAddr>>,
    /// Told when [`Self::candidates`] changed, for the invite to be renewed with them.
    readdressed: Notify,
    tag: [u8; 8],
    /// Changes whenever the status or identity does.
    changed: watch::Sender<u64>,
}

impl Shared {
    fn update(&self, change: impl FnOnce(&mut HostStatus)) {
        change(&mut self.status.lock().expect("not poisoned"));
        self.changed.send_modify(|n| *n += 1);
    }

    /// Takes a new or renewed registration: saves it in the identity and the status.
    fn registered(&self, registered: &Registered) {
        let invite = {
            let mut identity = self.identity.lock().expect("not poisoned");
            identity.invite = Some(SavedInvite {
                service: self.service.base_url(),
                code: registered.code.clone(),
                secret: registered.secret.clone(),
                kept: registered.kept,
            });
            identity.invite_at(self.service.clone(), &registered.code)
        };
        self.update(|status| {
            status.link = invite.link();
            status.code = registered.code.clone();
            status.stable_link = registered.kept;
            status.state = HostState::Sharing;
            status.problem = None;
        });
    }

    fn candidates(&self) -> Vec<SocketAddr> {
        self.candidates.lock().expect("not poisoned").clone()
    }

    /// Takes the addresses collected again. When they changed, the invite is renewed
    /// with them at once: friends who join get only the addresses the service holds.
    /// Returns whether they changed.
    fn readdress(&self, candidates: Vec<Candidate>, symmetric: bool) -> bool {
        let addrs: Vec<SocketAddr> = candidates.iter().map(|c| c.addr).collect();
        let changed = {
            let mut current = self.candidates.lock().expect("not poisoned");
            let changed = *current != addrs;
            *current = addrs;
            changed
        };
        let shown = {
            let status = self.status.lock().expect("not poisoned");
            status.candidates == candidates && status.symmetric_nat == symmetric
        };
        if !shown {
            self.update(|status| {
                status.candidates = candidates;
                status.symmetric_nat = symmetric;
            });
        }
        if changed {
            tracing::debug!("connect: this computer's addresses changed; renewing the invite");
            self.readdressed.notify_one();
        }
        changed
    }

    fn saved(&self) -> Option<SavedInvite> {
        self.identity
            .lock()
            .expect("not poisoned")
            .invite
            .clone()
            .filter(|invite| invite.service == self.service.base_url())
    }
}

/// A server being shared. Dropping it stops sharing without ending the invite, as
/// [`Host::stop`]`(false)` does, but without waiting.
pub struct Host {
    shared: Arc<Shared>,
    stop: watch::Sender<bool>,
    endpoint: quinn::Endpoint,
    tasks: Vec<JoinHandle<()>>,
}

impl Host {
    /// Starts sharing: collects this computer's addresses, opens the invite at the
    /// service (renewing the identity's saved one if it has one), and waits for friends
    /// in the background. Fails if no invite could be opened and none was saved.
    pub async fn start(
        identity: HostIdentity,
        config: HostConfig,
        handler: impl StreamHandler,
    ) -> Result<Host> {
        let server = tunnel::server_config(&identity.cert_der, &identity.key_der)?;
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
        let token = identity.token;
        let shared = Arc::new(Shared {
            identity: Mutex::new(identity),
            service: config.service.clone(),
            keep: config.keep,
            status: Mutex::new(HostStatus {
                state: HostState::Reconnecting,
                link: String::new(),
                code: String::new(),
                stable_link: false,
                candidates: gathered.candidates.clone(),
                symmetric_nat: gathered.symmetric,
                joins: 0,
                friends: Vec::new(),
                problem: None,
            }),
            friends: Mutex::new(HashMap::new()),
            next_friend: AtomicU64::new(1),
            mapping: Mutex::new(gathered.mapping),
            candidates: Mutex::new(candidates),
            readdressed: Notify::new(),
            tag: probe::tag(&token),
            changed: watch::channel(0).0,
        });

        // The invite: renew the saved one, else make one.
        let opened = open_invite(&shared, false).await;
        match opened {
            Ok(registered) => shared.registered(&registered),
            Err(err) => match shared.saved() {
                // offline: keep the saved link, and try again in the background
                Some(saved) => {
                    let invite = shared
                        .identity
                        .lock()
                        .expect("not poisoned")
                        .invite_at(shared.service.clone(), &saved.code);
                    shared.update(|status| {
                        status.link = invite.link();
                        status.code = saved.code.clone();
                        status.stable_link = saved.kept;
                        status.problem = Some(err.to_string());
                    });
                }
                None => {
                    remove_mapping(&shared).await;
                    return Err(err);
                }
            },
        }

        // Everyone may reach the host (they show the token over QUIC); the gate only
        // takes the answers to the STUN requests that collect its addresses again.
        let (endpoint, gate) = Gate::endpoint(gathered.socket, server, Arc::new(|_| true))?;
        let (stop, stop_rx) = watch::channel(false);
        let handler: Arc<dyn StreamHandler> = Arc::new(handler);
        let tasks = vec![
            tokio::spawn(accept_loop(
                endpoint.clone(),
                shared.clone(),
                token,
                handler,
            )),
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
        Ok(Host {
            shared,
            stop,
            endpoint,
            tasks,
        })
    }

    /// The invite link friends join with.
    pub fn invite(&self) -> Invite {
        let identity = self.shared.identity.lock().expect("not poisoned");
        let code = identity
            .invite
            .as_ref()
            .map(|invite| invite.code.clone())
            .unwrap_or_default();
        identity.invite_at(self.shared.service.clone(), &code)
    }

    /// The identity with the current invite, to save.
    pub fn identity(&self) -> HostIdentity {
        self.shared.identity.lock().expect("not poisoned").clone()
    }

    /// How sharing is going, and who is connected.
    pub fn status(&self) -> HostStatus {
        let mut status = self.shared.status.lock().expect("not poisoned").clone();
        let friends = self.shared.friends.lock().expect("not poisoned");
        let mut list: Vec<FriendStatus> = friends
            .iter()
            .map(|(id, friend)| {
                let address = canonical(friend.connection.remote_address());
                FriendStatus {
                    id: *id,
                    address,
                    kind: ConnectionKind::of(address),
                    connected_at: friend
                        .connected_at
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    rtt_ms: friend.connection.rtt().as_millis() as u64,
                }
            })
            .collect();
        list.sort_by_key(|friend| friend.id);
        status.friends = list;
        status
    }

    /// Changes whenever the status or the identity (a new code) does. Friends'
    /// connections and disconnections count too.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.shared.changed.subscribe()
    }

    /// Stops sharing: disconnects friends and removes the router's forwarding. With
    /// `end_invite`, also ends the invite at the service, so the link stops working
    /// for good; without it (the program is closing), the link works again once
    /// sharing resumes with the saved identity.
    pub async fn stop(mut self, end_invite: bool) {
        let _ = self.stop.send(true);
        self.endpoint.close(0u32.into(), b"stopped");
        for task in self.tasks.drain(..) {
            task.abort();
        }
        if end_invite {
            if let Some(saved) = self.shared.saved() {
                let _ = tokio::time::timeout(
                    Duration::from_secs(5),
                    self.shared.service.end(&saved.code, &saved.secret),
                )
                .await;
            }
            self.shared.identity.lock().expect("not poisoned").invite = None;
        }
        remove_mapping(&self.shared).await;
        self.shared.friends.lock().expect("not poisoned").clear();
        self.shared
            .update(|status| status.state = HostState::Stopped);
    }
}

impl Drop for Host {
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

async fn remove_mapping(shared: &Shared) {
    let mapping = shared.mapping.lock().expect("not poisoned").take();
    if let Some(mapping) = mapping {
        let _ = tokio::task::spawn_blocking(move || mapping.remove()).await;
    }
}

/// Renews the saved invite (`reopen`: it has ended), or makes a new one when there's
/// none or the service no longer keeps it.
async fn open_invite(shared: &Shared, reopen: bool) -> Result<Registered> {
    let service = &shared.service;
    if let Some(saved) = shared.saved() {
        if saved.kept || !reopen {
            match service
                .renew(&saved.code, &saved.secret, &shared.candidates())
                .await
            {
                Ok(registered) => return Ok(registered),
                // gone, or a service without renewing (405 on older ones)
                Err(Error::Service {
                    status: 403..=405, ..
                }) => {}
                Err(err) => return Err(err),
            }
        }
    }
    service.create(&shared.candidates(), shared.keep).await
}

/// Waits on the invite for joins, punches towards each friend, keeps the invite open
/// (renewing it daily) and reconnects to the service when it drops.
async fn service_loop(shared: Arc<Shared>, sprayer: UdpSocket, mut stop: watch::Receiver<bool>) {
    let mut seen: u64 = 0;
    let mut sprays: VecDeque<JoinHandle<()>> = VecDeque::new();
    let mut backoff = Duration::from_secs(2);
    let mut renew_at = tokio::time::Instant::now() + RENEW_EVERY;
    // After a failed start the invite isn't known to be open: renew it first.
    let mut reopen = shared
        .status
        .lock()
        .expect("not poisoned")
        .problem
        .is_some();
    loop {
        if *stop.borrow() {
            return;
        }
        if reopen {
            match open_invite(&shared, true).await {
                Ok(registered) => {
                    shared.registered(&registered);
                    seen = 0;
                    renew_at = tokio::time::Instant::now() + RENEW_EVERY;
                    reopen = false;
                    backoff = Duration::from_secs(2);
                }
                Err(err) => {
                    wait_or_stop(&shared, &err, &mut backoff, &mut stop).await;
                    continue;
                }
            }
        }
        let Some(saved) = shared.saved() else {
            reopen = true;
            continue;
        };
        let mut watch = match shared.service.watch(&saved.code, &saved.secret, seen).await {
            Ok(Some(watch)) => watch,
            Ok(None) => {
                reopen = true;
                continue;
            }
            Err(err) => {
                // A wrong secret or a code the service no longer knows: start over.
                if matches!(err, Error::Service { status: 403, .. }) {
                    reopen = true;
                }
                wait_or_stop(&shared, &err, &mut backoff, &mut stop).await;
                continue;
            }
        };
        shared.update(|status| {
            status.state = HostState::Sharing;
            status.problem = None;
        });
        backoff = Duration::from_secs(2);
        loop {
            tokio::select! {
                _ = stop.changed() => return,
                // this computer's addresses changed: renew now, with the new ones
                _ = shared.readdressed.notified() => {
                    renew_at = tokio::time::Instant::now();
                }
                _ = tokio::time::sleep_until(renew_at) => {
                    match shared.service.renew(&saved.code, &saved.secret, &shared.candidates()).await {
                        Ok(registered) => {
                            shared.registered(&registered);
                            seen = 0;
                            renew_at = tokio::time::Instant::now() + RENEW_EVERY;
                        }
                        Err(Error::Service { status: 403..=405, .. }) => {
                            // renewing isn't possible: the invite runs out, then a new
                            // one is made
                            renew_at = tokio::time::Instant::now() + RENEW_EVERY;
                            continue;
                        }
                        Err(_) => {
                            renew_at = tokio::time::Instant::now() + Duration::from_secs(60);
                            continue;
                        }
                    }
                    break;
                }
                event = watch.next() => match event {
                    Ok(Event::Joins(joins)) => {
                        for (id, mut peers) in joins {
                            if id < seen {
                                continue;
                            }
                            seen = id + 1;
                            shared.update(|status| status.joins += 1);
                            peers.truncate(MAX_PEERS);
                            sprays.retain(|spray| !spray.is_finished());
                            while sprays.len() >= MAX_SPRAYS {
                                if let Some(oldest) = sprays.pop_front() {
                                    oldest.abort();
                                }
                            }
                            if let Ok(socket) = sprayer.try_clone() {
                                sprays.push_back(tokio::spawn(probe::spray(socket, peers, shared.tag, SPRAY_FOR, stop.clone())));
                            }
                        }
                        if seen >= RENEW_AFTER_JOINS && saved.kept {
                            renew_at = tokio::time::Instant::now();
                        }
                    }
                    Ok(Event::Renewed) => {
                        seen = 0;
                        break;
                    }
                    Ok(Event::Ended) => {
                        reopen = true;
                        break;
                    }
                    Err(err) => {
                        wait_or_stop(&shared, &err, &mut backoff, &mut stop).await;
                        break;
                    }
                },
            }
        }
    }
}

/// Notes the problem and waits (longer each time, up to a minute) unless stopped.
async fn wait_or_stop(
    shared: &Shared,
    err: &Error,
    backoff: &mut Duration,
    stop: &mut watch::Receiver<bool>,
) {
    tracing::debug!("connect: {err}; trying again in {backoff:?}");
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
            // try again next time
            Err(err) => tracing::debug!("connect: collecting the addresses again failed: {err}"),
        }
    }
}

/// Collects this computer's addresses again: its network or public address may have
/// changed since sharing started, and friends get only the addresses the invite
/// carries. Renews the router's forwarding, or asks for it again on another network
/// ([`gather::reforward`]; `lan_v4` was the address on the local network). Renews the
/// invite if the addresses changed. Returns the address on the local network now.
/// Blocking.
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
    let mapping = gather::reforward(old, port, found.lan_v4, lan_v4, options.portmap);
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
    if !candidates.is_empty() {
        shared.readdress(candidates, found.symmetric);
    }
    found.lan_v4
}

async fn accept_loop(
    endpoint: quinn::Endpoint,
    shared: Arc<Shared>,
    token: [u8; TOKEN_LEN],
    handler: Arc<dyn StreamHandler>,
) {
    let unproven = Arc::new(AtomicUsize::new(0));
    while let Some(incoming) = endpoint.accept().await {
        // First make the other side show it gets packets at its address (a QUIC
        // retry), so no one can open connections in someone else's name, which would
        // also make the host send its handshake to them.
        if !incoming.remote_address_validated() {
            if let Err(err) = incoming.retry() {
                err.into_incoming().refuse();
            }
            continue;
        }
        if shared.friends.lock().expect("not poisoned").len() >= MAX_FRIENDS {
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
                Err(err) => return tracing::debug!("connect: a connection attempt failed: {err}"),
            };
            serve_connection(connection, slot, shared, token, handler).await;
        });
    }
}

/// One of the [`MAX_UNPROVEN`] places for a connection that hasn't shown the token
/// yet. Dropping it frees the place.
pub(crate) struct UnprovenSlot(Arc<AtomicUsize>);

impl UnprovenSlot {
    pub(crate) fn take(count: &Arc<AtomicUsize>) -> Option<Self> {
        // When all places are taken, dropping `slot` gives this one back.
        let slot = Self(count.clone());
        (count.fetch_add(1, Ordering::AcqRel) < MAX_UNPROVEN).then_some(slot)
    }
}

impl Drop for UnprovenSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

async fn serve_connection(
    connection: quinn::Connection,
    slot: UnprovenSlot,
    shared: Arc<Shared>,
    token: [u8; TOKEN_LEN],
    handler: Arc<dyn StreamHandler>,
) {
    let id = shared.next_friend.fetch_add(1, Ordering::Relaxed);
    let (proved_tx, mut proved_rx) = watch::channel(false);
    // Closes the connection unless it proves it has the token in time. The slot is
    // freed once it has, or once the connection ends.
    {
        let connection = connection.clone();
        tokio::spawn(async move {
            let _slot = slot;
            tokio::select! {
                proved = tokio::time::timeout(PROVE_WITHIN, proved_rx.wait_for(|p| *p)) => {
                    if !matches!(proved, Ok(Ok(_))) {
                        connection.close(1u32.into(), b"no token");
                    }
                }
                _ = connection.closed() => {}
            }
        });
    }
    let proved_tx = Arc::new(proved_tx);
    loop {
        let (send, mut recv) = match connection.accept_bi().await {
            Ok(streams) => streams,
            Err(_) => break,
        };
        let connection = connection.clone();
        let shared = shared.clone();
        let handler = handler.clone();
        let proved_tx = proved_tx.clone();
        tokio::spawn(async move {
            let mut header = [0u8; TOKEN_LEN + 1];
            let read = tokio::time::timeout(HEADER_WITHIN, recv.read_exact(&mut header)).await;
            let valid = matches!(read, Ok(Ok(())))
                && bool::from(subtle::ConstantTimeEq::ct_eq(
                    &header[..TOKEN_LEN],
                    &token[..],
                ));
            let kind = StreamKind::from_byte(header[TOKEN_LEN]);
            let (true, Some(kind)) = (valid, kind) else {
                // the wrong token: this isn't a friend with the link
                connection.close(1u32.into(), b"wrong token");
                return;
            };
            if !*proved_tx.borrow() {
                proved_tx.send_replace(true);
                shared.friends.lock().expect("not poisoned").insert(
                    id,
                    Friend {
                        connection: connection.clone(),
                        connected_at: SystemTime::now(),
                    },
                );
                shared.changed.send_modify(|n| *n += 1);
            }
            let mut stream = tokio::io::join(recv, send);
            match kind {
                StreamKind::Game => handler.game(stream, id),
                StreamKind::Info => handler.info(stream, id),
                StreamKind::Hello => {
                    let _ = stream.write_all(&[HELLO_ANSWER]).await;
                    let _ = stream.shutdown().await;
                }
            }
        });
    }
    if shared
        .friends
        .lock()
        .expect("not poisoned")
        .remove(&id)
        .is_some()
    {
        shared.changed.send_modify(|n| *n += 1);
    }
}

/// The in-process introduction service the integration tests use.
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/support/fake_service.rs"]
mod fake_service;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identities_round_trip_and_hide_their_secrets() {
        let mut identity = HostIdentity::generate().unwrap();
        identity.invite = Some(SavedInvite {
            service: "https://connect.lodestone.cc".into(),
            code: "ABCDEFGH23".into(),
            secret: "0123456789abcdef0123456789abcdef".into(),
            kept: true,
        });
        let back = HostIdentity::from_json(&identity.to_json()).unwrap();
        assert_eq!(back.fingerprint(), identity.fingerprint());
        assert_eq!(back.token, identity.token);
        assert_eq!(back.invite, identity.invite);
        let shown = format!("{identity:?}");
        assert!(!shown.contains("0123456789abcdef"), "{shown}");
        assert!(!shown.contains(&hex(&identity.token)), "{shown}");
        assert!(HostIdentity::from_json("{}").is_err());

        let invite = identity.invite_at(Service::default(), "ABCDEFGH23");
        assert_eq!(Invite::parse(&invite.link()), Ok(invite));
    }

    /// A host waiting for friends on a loopback socket.
    fn host_endpoint(identity: &HostIdentity) -> (quinn::Endpoint, Arc<Shared>, SocketAddr) {
        let server = tunnel::server_config(&identity.cert_der, &identity.key_der).unwrap();
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = socket.local_addr().unwrap();
        let endpoint = tunnel::endpoint(socket, Some(server)).unwrap();
        let shared = Arc::new(Shared {
            identity: Mutex::new(identity.clone()),
            service: Service::default(),
            keep: false,
            status: Mutex::new(HostStatus {
                state: HostState::Sharing,
                link: String::new(),
                code: String::new(),
                stable_link: false,
                candidates: Vec::new(),
                symmetric_nat: false,
                joins: 0,
                friends: Vec::new(),
                problem: None,
            }),
            friends: Mutex::new(HashMap::new()),
            next_friend: AtomicU64::new(1),
            mapping: Mutex::new(None),
            candidates: Mutex::new(Vec::new()),
            readdressed: Notify::new(),
            tag: probe::tag(&identity.token),
            changed: watch::channel(0).0,
        });
        (endpoint, shared, addr)
    }

    struct Discard;

    impl StreamHandler for Discard {
        fn game(&self, _: TunnelStream, _: FriendId) {}
        fn info(&self, _: TunnelStream, _: FriendId) {}
    }

    async fn stranger(identity: &HostIdentity, host: SocketAddr) -> quinn::Connection {
        let mut client = tunnel::endpoint(UdpSocket::bind("127.0.0.1:0").unwrap(), None).unwrap();
        client.set_default_client_config(tunnel::client_config(identity.fingerprint()).unwrap());
        client
            .connect(host, tunnel::SERVER_NAME)
            .unwrap()
            .await
            .unwrap()
    }

    /// After this computer's addresses change, the invite is renewed with the new ones
    /// at once, so friends who join get them (LS-53).
    #[tokio::test]
    async fn new_addresses_reach_the_invite() {
        let service = fake_service::FakeService::start().await;
        let config = HostConfig {
            service: Service::from_setting(&service.url()).unwrap(),
            gather: GatherOptions {
                portmap: false,
                loopback_only: true,
            },
            keep: true,
        };
        let host = Host::start(HostIdentity::generate().unwrap(), config, Discard)
            .await
            .unwrap();
        let code = host.status().code;
        let registered = |service: &fake_service::FakeService| -> Vec<SocketAddr> {
            service
                .host_candidates(&code)
                .iter()
                .map(|addr| addr.parse().unwrap())
                .collect()
        };
        assert_eq!(registered(&service), host.shared.candidates());

        let moved = Candidate {
            addr: "198.51.100.9:40000".parse().unwrap(),
            kind: CandidateKind::Public,
        };
        assert!(host.shared.readdress(vec![moved], true));
        tokio::time::timeout(Duration::from_secs(10), async {
            while registered(&service) != vec![moved.addr] {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("the invite carries the new addresses");
        let status = host.status();
        assert_eq!(status.candidates, vec![moved]);
        assert!(status.symmetric_nat);
        assert_eq!(status.code, code, "the same link");
        // the same addresses again: nothing to renew
        assert!(!host.shared.readdress(vec![moved], true));
        host.stop(true).await;
    }

    /// Someone who reaches the host's port but has no token can't make the host hold
    /// much of their data in streams it never reads.
    #[tokio::test]
    async fn a_stranger_cannot_make_the_host_buffer_much() {
        let identity = HostIdentity::generate().unwrap();
        let (endpoint, shared, addr) = host_endpoint(&identity);
        tokio::spawn(accept_loop(
            endpoint,
            shared,
            identity.token,
            Arc::new(Discard),
        ));
        let connection = stranger(&identity, addr).await;
        let pushing = connection.clone();
        tokio::spawn(async move {
            let chunk = vec![7u8; 1 << 20];
            let mut streams = Vec::new();
            for _ in 0..64 {
                let Ok(mut stream) = pushing.open_uni().await else {
                    return;
                };
                let _ = stream.write_all(&chunk).await;
                streams.push(stream);
            }
            std::future::pending::<()>().await
        });
        tokio::time::sleep(Duration::from_secs(3)).await;
        let sent = connection.stats().udp_tx.bytes;
        assert!(
            sent < 2 * tunnel::HOST_RECEIVE_WINDOW as u64,
            "the host let a stranger send {sent} bytes"
        );
    }

    /// Strangers can hold only a few connections open at once while they haven't shown
    /// the token.
    #[tokio::test]
    async fn strangers_cannot_hold_many_connections() {
        let identity = HostIdentity::generate().unwrap();
        let (endpoint, shared, addr) = host_endpoint(&identity);
        tokio::spawn(accept_loop(
            endpoint,
            shared,
            identity.token,
            Arc::new(Discard),
        ));
        let mut client = tunnel::endpoint(UdpSocket::bind("127.0.0.1:0").unwrap(), None).unwrap();
        client.set_default_client_config(tunnel::client_config(identity.fingerprint()).unwrap());
        let mut held = Vec::new();
        for _ in 0..40 {
            let attempt = client.connect(addr, tunnel::SERVER_NAME).unwrap();
            if let Ok(Ok(connection)) = tokio::time::timeout(Duration::from_secs(2), attempt).await
            {
                held.push(connection);
            }
        }
        assert!(
            held.len() <= MAX_UNPROVEN,
            "a stranger holds {} connections",
            held.len()
        );
    }
}
