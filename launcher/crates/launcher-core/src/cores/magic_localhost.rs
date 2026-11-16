//! Magic localhost for the servers on the Lodestone Host page: an address on this
//! computer, `127.0.0.1:<port>`, that any Minecraft client (the official launcher,
//! another launcher) can join, the same every session.
//!
//! - A server on a core on **this computer** is simply at its own port,
//!   `127.0.0.1:<server port>` ([`MagicAddress::ThisComputer`]): nothing listens for it
//!   and nothing is saved. It works whenever the server runs.
//! - A server on **another host** (added by address, signed in or not) gets a port of its
//!   own on this computer, picked once from the core's address and the server's id
//!   ([`magic_id`], [`friends::port_choices`]) and saved with the core in `servers.json`
//!   ([`MagicPort`], `SavedCore::magic_ports`). A [`Forwarder`] listens there, on
//!   127.0.0.1 only, and passes each connection on to the core's host at the server's
//!   game port: the same place Play joins (`super::join::join_target`). The app keeps
//!   the forwarders open for as long as it runs, because an outside client connects
//!   without the app knowing it is about to.
//! - Servers shared through **Lodestone Connect** keep their own Magic localhost (see
//!   [`friends`]): the tunnel's game port, while the launcher is connected.
//!
//! When something else has a saved port, the forwarder takes any free port for the
//! session and says so ([`Forwarder::taken`]); the saved port stays for next time.

use std::{
    collections::HashSet,
    future::Future,
    io,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::channel::oneshot;
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpSocket, TcpStream},
    runtime::Handle,
    sync::{OwnedSemaphorePermit, Semaphore},
    task::{AbortHandle, JoinSet},
};
use url::Url;

use super::{
    client::is_loopback_host,
    connect::{GamePort, bind_game},
    friends::{self, GAME_PORTS},
    models::ServerInstance,
};

/// The most servers of one core that keep a port: a hostile core can't fill the file.
pub const MAX_PER_CORE: usize = 64;
// The limits below fit the open-file limit of an app started from the Finder or the
// Dock, 256 on macOS (launchd's soft limit, which nothing in the app raises). Each
// forwarder takes one file while it listens and each relayed connection two, so at
// most 48 + 48 × 2 = 144, which leaves the rest of the app (about 50 at rest) room.
/// The most forwarders open at once, over every core.
pub const MAX_FORWARDERS: usize = 48;
/// The most connections one forwarder relays at once; more are closed at once.
pub const MAX_CONNECTIONS: usize = 16;
/// The most connections every forwarder together relays at once; more are closed at
/// once. A Minecraft server list pings each of its servers at the same time.
pub const MAX_TOTAL_CONNECTIONS: usize = 48;
/// The most threads the forwarders' runtime resolves names on. A lookup that hangs
/// (a captive portal's DNS, say) keeps its thread until the system gives up, after the
/// connection's [`CONNECT_TIMEOUT`]; more wait their turn instead of starting threads.
const MAX_LOOKUP_THREADS: usize = 8;
// launchd's 256, less the files the app holds at rest and room for its own work
const _: () = assert!(MAX_FORWARDERS + 2 * MAX_TOTAL_CONNECTIONS <= 256 - 50 - 40);
const _: () = assert!(MAX_CONNECTIONS <= MAX_TOTAL_CONNECTIONS);
/// How long a forwarder waits for the server to answer (and for its name to resolve).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// The longest server id kept.
const MAX_INSTANCE_LEN: usize = 64;
/// How many of a name's addresses a connection tries.
const MAX_ADDRESSES: usize = 8;

/// One server's port on this computer, saved with its core in `servers.json`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MagicPort {
    /// The server's id on its core.
    pub instance: String,
    /// The port on this computer, in [`GAME_PORTS`].
    pub port: u16,
    /// The server's game port, as the core said last. Kept so the forwarder opens at
    /// startup, before the page fetches the core's list.
    pub target: u16,
}

impl MagicPort {
    /// Whether the entry is one this module could have written: an entry edited by
    /// hand, or written by a newer launcher, is dropped rather than trusted.
    pub fn is_valid(&self) -> bool {
        valid_instance(&self.instance) && GAME_PORTS.contains(&self.port) && self.target != 0
    }
}

/// A server id as cores make them (a UUID), short and plain.
fn valid_instance(instance: &str) -> bool {
    !instance.is_empty()
        && instance.len() <= MAX_INSTANCE_LEN
        && instance
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// What a server's port is picked from: the core's address (which stays when the core
/// gets a new id on restart) and the server's id.
pub fn magic_id(core_url: &str, instance: &str) -> String {
    format!("core:{core_url}/{instance}")
}

/// The Magic localhost address a server's row shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MagicAddress {
    /// The server runs on this computer: its own port.
    ThisComputer { port: u16 },
    /// A forwarder on this computer: `port` is where it listens this session (or will).
    /// `taken` is the saved port when something else had it, so this session uses
    /// another.
    Forwarded { port: u16, taken: Option<u16> },
}

impl MagicAddress {
    pub fn port(&self) -> u16 {
        match *self {
            MagicAddress::ThisComputer { port } | MagicAddress::Forwarded { port, .. } => port,
        }
    }

    /// `127.0.0.1:<port>`, for Minecraft's server list.
    pub fn address(&self) -> String {
        friends::game_address(self.port())
    }

    /// The control API's word for the kind.
    pub fn kind(&self) -> &'static str {
        match self {
            MagicAddress::ThisComputer { .. } => "this_computer",
            MagicAddress::Forwarded { .. } => "forwarded",
        }
    }
}

/// The server's game port, when the core shares one that fits.
fn game_port(server: &ServerInstance) -> Option<u16> {
    u16::try_from(server.port?).ok().filter(|port| *port != 0)
}

/// The address `server`'s row shows. `local`: its core's card is in "On this computer".
/// `saved` is the core's saved ports, and `bound` the port its forwarder listens on this
/// session, once it does. `None` when the core hides the server's port, or no port is
/// saved for it yet.
pub fn magic_address(
    local: bool,
    saved: &[MagicPort],
    server: &ServerInstance,
    bound: Option<u16>,
) -> Option<MagicAddress> {
    let target = game_port(server)?;
    if local {
        return Some(MagicAddress::ThisComputer { port: target });
    }
    let entry = saved
        .iter()
        .find(|entry| entry.instance == server.uuid && entry.is_valid())?;
    let port = bound.unwrap_or(entry.port);
    Some(MagicAddress::Forwarded {
        port,
        taken: (port != entry.port).then_some(entry.port),
    })
}

/// Drops the entries [`MagicPort::is_valid`] refuses, later entries for a server or a
/// port already kept, and any past [`MAX_PER_CORE`]. Returns whether anything went.
pub fn sanitize(saved: &mut Vec<MagicPort>) -> bool {
    let before = saved.len();
    let (mut instances, mut ports) = (HashSet::new(), HashSet::new());
    saved.retain(|entry| {
        entry.is_valid() && instances.insert(entry.instance.clone()) && ports.insert(entry.port)
    });
    saved.truncate(MAX_PER_CORE);
    saved.len() != before
}

/// Drops the saved ports that are in `ports` (a server on this computer took one as its
/// game port, say): the next [`update_ports`], given them as `taken`, picks those
/// servers new ones. Returns whether anything went.
pub fn release(saved: &mut Vec<MagicPort>, ports: &[u16]) -> bool {
    let before = saved.len();
    saved.retain(|entry| !ports.contains(&entry.port));
    saved.len() != before
}

/// Brings the saved ports of the core at `core_url` up to date with its list, `servers`:
/// a server with a game port and no saved port gets one, picked from [`magic_id`] away
/// from `taken` (other cores' and friends' ports) and this core's; a saved port follows
/// its server's game port. With `authoritative` (the list came signed in, its public
/// half too, so it has every server), servers no longer listed lose theirs; any other
/// list only adds, so ports stay while signed out or when the public half failed, until
/// the core has [`MAX_PER_CORE`]: then the oldest port of a server it doesn't list makes
/// room for one it does. Returns whether anything changed, for the caller to save.
pub fn update_ports(
    saved: &mut Vec<MagicPort>,
    core_url: &str,
    servers: &[ServerInstance],
    authoritative: bool,
    taken: &[u16],
) -> bool {
    let mut changed = sanitize(saved);
    if authoritative {
        let before = saved.len();
        saved.retain(|entry| servers.iter().any(|s| s.uuid == entry.instance));
        changed |= saved.len() != before;
    }
    for server in servers {
        let Some(target) = game_port(server) else {
            continue;
        };
        if let Some(entry) = saved.iter_mut().find(|e| e.instance == server.uuid) {
            if entry.target != target {
                entry.target = target;
                changed = true;
            }
            continue;
        }
        if !valid_instance(&server.uuid) {
            continue;
        }
        if saved.len() >= MAX_PER_CORE {
            // Full: the oldest saved port this list doesn't show makes room for a server
            // it does, so a core whose servers come and go while you're signed out
            // doesn't keep deleted servers' ports for good and leave its new ones none.
            let Some(stale) = saved
                .iter()
                .position(|e| !servers.iter().any(|s| s.uuid == e.instance))
            else {
                continue;
            };
            saved.remove(stale);
        }
        let mut avoid: Vec<u16> = taken.to_vec();
        avoid.extend(saved.iter().map(|e| e.port));
        let Some(&port) = friends::port_choices(&magic_id(core_url, &server.uuid), &avoid).first()
        else {
            continue;
        };
        saved.push(MagicPort {
            instance: server.uuid.clone(),
            port,
            target,
        });
        changed = true;
    }
    changed
}

/// A core on another host, for [`wanted_forwarders`].
pub struct HostCore<'a> {
    /// The core's address as saved, which [`magic_id`]s are made from.
    pub saved_url: &'a str,
    /// The address the forwarders connect to.
    pub url: &'a Url,
    pub saved: &'a [MagicPort],
    /// The servers in the core's latest list, when one arrived this session.
    pub listed: Option<&'a [ServerInstance]>,
}

/// A forwarder [`wanted_forwarders`] gives a server: its [`magic_id`], its saved port
/// and where it forwards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wanted {
    pub id: String,
    pub port: u16,
    pub target: Target,
}

/// Shares the [`MAX_FORWARDERS`] forwarders among the saved ports of `cores`. Returns
/// the forwarders to open and the [`magic_id`]s of the servers with a saved port that get
/// none (past the cap, or a host that is this computer), whose rows show no address.
///
/// The share is fair, so no core can shut out the others: a hostile core, or one that
/// makes and deletes many servers, has as many saved ports as any other (up to
/// [`MAX_PER_CORE`], more than the cap). Forwarders go round the cores one at a time,
/// first to the servers each core's latest list shows (every saved one when no list
/// arrived yet), then to the saved ports of servers it no longer lists, which a list
/// you aren't signed in to never drops (they may be private servers, or deleted ones).
pub fn wanted_forwarders(cores: &[HostCore]) -> (Vec<Wanted>, Vec<String>) {
    let (mut listed, mut unlisted, mut unserved) = (Vec::new(), Vec::new(), Vec::new());
    for core in cores {
        let (mut shown, mut hidden) = (Vec::new(), Vec::new());
        let entries = core.saved.iter().filter(|e| e.is_valid());
        for entry in entries.take(MAX_PER_CORE) {
            let id = magic_id(core.saved_url, &entry.instance);
            let Some(target) = Target::new(core.url, entry.target) else {
                unserved.push(id);
                continue;
            };
            let wanted = Wanted {
                id,
                port: entry.port,
                target,
            };
            let is_listed = core
                .listed
                .is_none_or(|list| list.iter().any(|s| s.uuid == entry.instance));
            if is_listed {
                shown.push(wanted);
            } else {
                hidden.push(wanted);
            }
        }
        listed.push(shown);
        unlisted.push(hidden);
    }
    let mut served = Vec::new();
    for queues in [listed, unlisted] {
        for wanted in round_robin(queues) {
            if served.len() < MAX_FORWARDERS {
                served.push(wanted);
            } else {
                unserved.push(wanted.id);
            }
        }
    }
    (served, unserved)
}

/// The first of each queue, then the second of each, and so on.
fn round_robin<T>(queues: Vec<Vec<T>>) -> Vec<T> {
    let mut queues: Vec<_> = queues.into_iter().map(Vec::into_iter).collect();
    let mut out = Vec::new();
    loop {
        let before = out.len();
        out.extend(queues.iter_mut().filter_map(Iterator::next));
        if out.len() == before {
            return out;
        }
    }
}

/// Where a forwarder sends its connections: the core's host, at the server's game port.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// A domain name or an IP address (an IPv6 address without brackets).
    pub host: String,
    pub port: u16,
}

impl Target {
    /// The server at `port` on the core at `core_url`. `None` for port 0, and for a core
    /// on this computer by its address, whose servers need no forwarder (and a forwarder
    /// to itself would loop). A name that only resolves to this computer is caught
    /// when a connection resolves it ([`is_this_computer`]).
    pub fn new(core_url: &Url, port: u16) -> Option<Self> {
        let (host, here) = match core_url.host()? {
            url::Host::Domain(domain) => {
                let name = domain.trim_end_matches('.').to_ascii_lowercase();
                let here = is_loopback_host(&name) || name.ends_with(".localhost");
                (domain.to_string(), here)
            }
            url::Host::Ipv4(ip) => (ip.to_string(), is_this_computer(ip.into())),
            url::Host::Ipv6(ip) => (ip.to_string(), is_this_computer(ip.into())),
        };
        (port != 0 && !here).then_some(Self { host, port })
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.host.contains(':') {
            write!(f, "[{}]:{}", self.host, self.port)
        } else {
            write!(f, "{}:{}", self.host, self.port)
        }
    }
}

/// Told when a connection couldn't reach its target (the server stopped, say), so the
/// app can log it. It runs on the forwarders' thread.
pub type OnError = Arc<dyn Fn(&Target, &io::Error) + Send + Sync>;

/// The forwarders' thread, with a current-thread Tokio runtime, started with the first
/// forwarder. Dropping this stops the thread, and with it every listener and
/// connection.
pub struct Forwarders {
    runtime: Option<RuntimeThread>,
    /// Whether forwarders may connect to this computer: only the tests' fake servers,
    /// which have nowhere else to listen.
    local_targets: bool,
    /// The connections every forwarder may relay at once ([`MAX_TOTAL_CONNECTIONS`]):
    /// each one holds a permit.
    budget: Arc<Semaphore>,
}

impl Default for Forwarders {
    fn default() -> Self {
        Self {
            runtime: None,
            local_targets: false,
            budget: Arc::new(Semaphore::new(MAX_TOTAL_CONNECTIONS)),
        }
    }
}

struct RuntimeThread {
    handle: Handle,
    /// Dropped with the [`Forwarders`], which ends the thread's `block_on`.
    _stop: oneshot::Sender<()>,
}

impl Forwarders {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forwarders that pass connections to servers on 127.0.0.1, for tests.
    #[cfg(test)]
    fn to_this_computer() -> Self {
        Self {
            local_targets: true,
            ..Self::default()
        }
    }

    /// [`Self::to_this_computer`], relaying at most `connections` at once in all.
    #[cfg(test)]
    fn to_this_computer_with_budget(connections: usize) -> Self {
        Self {
            budget: Arc::new(Semaphore::new(connections)),
            ..Self::to_this_computer()
        }
    }

    fn handle(&mut self) -> io::Result<Handle> {
        if let Some(runtime) = &self.runtime {
            return Ok(runtime.handle.clone());
        }
        let (handle_tx, handle_rx) = std::sync::mpsc::channel();
        let (stop_tx, stop_rx) = oneshot::channel::<()>();
        std::thread::Builder::new()
            .name("lodestone-magic-localhost".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .max_blocking_threads(MAX_LOOKUP_THREADS)
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(err) => {
                        let _ = handle_tx.send(Err(err));
                        return;
                    }
                };
                let _ = handle_tx.send(Ok(runtime.handle().clone()));
                // until the Forwarders are dropped; the runtime's tasks go with it
                runtime.block_on(async {
                    let _ = stop_rx.await;
                });
                // without waiting for a name lookup that hangs
                runtime.shutdown_background();
            })?;
        let handle = handle_rx
            .recv()
            .map_err(|_| io::Error::other("the Magic localhost thread stopped"))??;
        self.runtime = Some(RuntimeThread {
            handle: handle.clone(),
            _stop: stop_tx,
        });
        Ok(handle)
    }

    /// Listens on `127.0.0.1:saved` (or any free port when something else has it) and
    /// forwards every connection to `target`. Everything runs on the forwarders' thread,
    /// timers included, so the returned future can be awaited on any executor.
    /// Dropping it before it is done closes the listener.
    pub fn start(
        &mut self,
        saved: u16,
        target: Target,
        on_error: OnError,
    ) -> impl Future<Output = io::Result<Forwarder>> + Send + 'static {
        let local_targets = self.local_targets;
        let budget = self.budget.clone();
        let started = self.handle().map(|handle| {
            let target = Arc::new(Mutex::new(target));
            let (ready_tx, ready_rx) = oneshot::channel::<io::Result<SocketAddr>>();
            let task_target = target.clone();
            let task = handle.spawn(async move {
                let bound = bind_game(&GamePort::Saved(saved), TcpListener::bind)
                    .await
                    .and_then(|listener| Ok((listener.local_addr()?, listener)));
                match bound {
                    Ok((addr, listener)) => {
                        if ready_tx.send(Ok(addr)).is_ok() {
                            let relay = Relay {
                                own: addr,
                                local_targets,
                                budget,
                                on_error,
                            };
                            accept_loop(listener, task_target, relay).await;
                        }
                    }
                    Err(err) => {
                        let _ = ready_tx.send(Err(err));
                    }
                }
            });
            (target, ready_rx, Abort(task.abort_handle()))
        });
        async move {
            let (target, ready_rx, task) = started?;
            let addr = ready_rx
                .await
                .map_err(|_| io::Error::other("the Magic localhost thread stopped"))??;
            Ok(Forwarder {
                addr,
                saved,
                target,
                _task: task,
            })
        }
    }
}

/// Aborts a task when dropped.
struct Abort(AbortHandle);

impl Drop for Abort {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// One server's listener on this computer. Dropping it closes the listener and every
/// connection it relays.
pub struct Forwarder {
    addr: SocketAddr,
    saved: u16,
    target: Arc<Mutex<Target>>,
    _task: Abort,
}

impl std::fmt::Debug for Forwarder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Forwarder")
            .field("addr", &self.addr)
            .field("saved", &self.saved)
            .finish_non_exhaustive()
    }
}

impl Forwarder {
    /// The port it listens on this session.
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    #[cfg(test)]
    fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// The port it was asked for.
    pub fn saved(&self) -> u16 {
        self.saved
    }

    /// The saved port, when something else had it so this session listens elsewhere.
    pub fn taken(&self) -> Option<u16> {
        (self.port() != self.saved).then_some(self.saved)
    }

    /// Sends new connections to `target` (the server's port changed, say); open ones
    /// stay where they are.
    pub fn set_target(&self, target: Target) {
        *self.target.lock().unwrap_or_else(|e| e.into_inner()) = target;
    }
}

/// What one forwarder's connections share.
#[derive(Clone)]
struct Relay {
    /// The forwarder's own address, never a target.
    own: SocketAddr,
    local_targets: bool,
    /// Every forwarder's connection permits.
    budget: Arc<Semaphore>,
    on_error: OnError,
}

async fn accept_loop(listener: TcpListener, target: Arc<Mutex<Target>>, relay: Relay) {
    // owned by this task, so aborting it closes every connection
    let mut connections = JoinSet::new();
    loop {
        let socket = match listener.accept().await {
            Ok((socket, _)) => socket,
            Err(_) => {
                // out of file descriptors, say: don't spin
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        while connections.try_join_next().is_some() {}
        if connections.len() >= MAX_CONNECTIONS {
            drop(socket);
            continue;
        }
        // past every forwarder's budget, closed like past this one's
        let Ok(permit) = relay.budget.clone().try_acquire_owned() else {
            drop(socket);
            continue;
        };
        let target = target.lock().unwrap_or_else(|e| e.into_inner()).clone();
        connections.spawn(relay.clone().run(socket, target, permit));
    }
}

impl Relay {
    /// Passes `client` on to `target` until either side closes. `_permit` is this
    /// connection's share of the budget, given back when it ends.
    async fn run(self, client: TcpStream, target: Target, _permit: OwnedSemaphorePermit) {
        let server = match connect(&target, self.own, self.local_targets).await {
            Ok(server) => server,
            Err(err) => {
                // the client's connection closes: Minecraft says it can't connect
                (self.on_error)(&target, &err);
                return;
            }
        };
        pipe(client, server).await;
    }
}

/// Copies bytes both ways between a client and its server.
async fn pipe(client: TcpStream, server: TcpStream) {
    let _ = client.set_nodelay(true);
    let _ = server.set_nodelay(true);
    let (mut client_read, mut client_write) = client.into_split();
    let (mut server_read, mut server_write) = server.into_split();
    // Minecraft never half-closes a connection, so the relay ends when either side
    // does: waiting for both would keep a connection whose host vanished without a
    // word (asleep, off the network) for good, and with it one of the forwarder's
    // MAX_CONNECTIONS slots, once its client left
    let up = std::pin::pin!(tokio::io::copy(&mut client_read, &mut server_write));
    let down = std::pin::pin!(tokio::io::copy(&mut server_read, &mut client_write));
    let _ = futures::future::select(up, down).await;
}

/// Whether `address` reaches this computer itself, where a forwarder never connects: a
/// core here needs none, and another forwarder there would make a loop. 127.0.0.0/8,
/// ::1, either written as an IPv4-mapped IPv6 address, and the unspecified addresses
/// (connecting to 0.0.0.0 reaches this computer).
fn is_this_computer(ip: IpAddr) -> bool {
    let ip = ip.to_canonical();
    ip.is_loopback() || ip.is_unspecified()
}

/// Connects to `target`, resolving its name now so a changed address counts. Never to
/// this computer ([`is_this_computer`], unless `local_targets`, for tests) and never
/// to `own` (the forwarder itself). One [`CONNECT_TIMEOUT`] covers the name and every
/// address it tries, so a host that drops packets holds a slot for that long at most.
/// The connection keeps TCP keepalive on, so a host that vanishes while nobody plays
/// (a client that doesn't close, say) is noticed eventually and its slot freed.
async fn connect(target: &Target, own: SocketAddr, local_targets: bool) -> io::Result<TcpStream> {
    let attempt = async {
        let addresses = tokio::net::lookup_host((target.host.as_str(), target.port)).await?;
        let mut last = io::Error::new(io::ErrorKind::NotFound, "the name has no address");
        for address in addresses.take(MAX_ADDRESSES) {
            if address == own || (!local_targets && is_this_computer(address.ip())) {
                last = io::Error::other("the address is this computer's own");
                continue;
            }
            let socket = match address {
                SocketAddr::V4(_) => TcpSocket::new_v4(),
                SocketAddr::V6(_) => TcpSocket::new_v6(),
            }?;
            let _ = socket.set_keepalive(true);
            match socket.connect(address).await {
                Ok(stream) => return Ok(stream),
                Err(err) => last = err,
            }
        }
        Err(last)
    };
    tokio::time::timeout(CONNECT_TIMEOUT, attempt)
        .await
        .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "no answer in time"))?
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    use super::*;
    use crate::cores::{
        models::{ServerLoader, ServerState},
        store::{self, SavedCore, ServersFile},
    };

    const CORE: &str = "https://192.168.1.42:16662/";

    fn server(uuid: &str, port: Option<u32>) -> ServerInstance {
        ServerInstance {
            uuid: uuid.into(),
            name: "SMP".into(),
            game_version: "1.21.1".into(),
            loader: ServerLoader::Vanilla,
            loader_version: None,
            port,
            state: ServerState::Running,
            player_count: None,
            max_player_count: None,
            has_client_pack: None,
            listed: true,
            public_only: false,
        }
    }

    fn quiet() -> OnError {
        Arc::new(|_, _| {})
    }

    fn loopback(port: u16) -> Target {
        Target {
            host: "127.0.0.1".into(),
            port,
        }
    }

    #[test]
    fn a_port_is_picked_once_and_kept_across_saves() {
        let list = [server("a", Some(25565)), server("b", Some(25566))];
        let mut saved = Vec::new();
        assert!(update_ports(&mut saved, CORE, &list, true, &[]));
        assert_eq!(saved.len(), 2);
        assert!(saved.iter().all(|e| GAME_PORTS.contains(&e.port)));
        assert_ne!(saved[0].port, saved[1].port);
        assert_eq!(
            saved[0].port,
            friends::port_choices(&magic_id(CORE, "a"), &[])[0]
        );
        assert_eq!((saved[0].target, saved[1].target), (25565, 25566));
        let first = saved.clone();
        // the same list again: nothing changes
        assert!(!update_ports(&mut saved, CORE, &list, true, &[]));
        assert_eq!(saved, first);

        // kept in servers.json, and back after a restart
        let dir = tempfile::tempdir().unwrap();
        let path = store::servers_file(dir.path());
        let file = ServersFile {
            cores: vec![SavedCore {
                uuid: "c".into(),
                url: CORE.into(),
                magic_ports: saved.clone(),
                ..SavedCore::default()
            }],
        };
        store::save(&path, &file).unwrap();
        let mut loaded = store::load(&path).unwrap();
        assert_eq!(loaded.cores[0].magic_ports, first);
        assert!(!update_ports(
            &mut loaded.cores[0].magic_ports,
            CORE,
            &list,
            true,
            &[]
        ));
        // a core without any is written as before
        let plain = ServersFile {
            cores: vec![SavedCore::default()],
        };
        store::save(&path, &plain).unwrap();
        assert!(
            !std::fs::read_to_string(&path)
                .unwrap()
                .contains("magic_ports")
        );
    }

    #[test]
    fn the_saved_target_follows_the_servers_port() {
        let mut saved = Vec::new();
        update_ports(&mut saved, CORE, &[server("a", Some(25565))], true, &[]);
        let port = saved[0].port;
        assert!(update_ports(
            &mut saved,
            CORE,
            &[server("a", Some(25570))],
            true,
            &[]
        ));
        assert_eq!((saved[0].port, saved[0].target), (port, 25570));
    }

    #[test]
    fn other_saved_ports_are_skipped() {
        let first = friends::port_choices(&magic_id(CORE, "a"), &[])[0];
        let mut saved = Vec::new();
        update_ports(&mut saved, CORE, &[server("a", Some(1))], true, &[first]);
        assert_ne!(saved[0].port, first);
        assert!(GAME_PORTS.contains(&saved[0].port));
    }

    #[test]
    fn only_a_signed_in_list_drops_ports() {
        let mut saved = Vec::new();
        update_ports(
            &mut saved,
            CORE,
            &[server("a", Some(1)), server("b", Some(2))],
            true,
            &[],
        );
        // a public list shows only some servers: the others keep their ports
        assert!(!update_ports(
            &mut saved,
            CORE,
            &[server("a", Some(1))],
            false,
            &[]
        ));
        assert_eq!(saved.len(), 2);
        assert!(update_ports(
            &mut saved,
            CORE,
            &[server("a", Some(1))],
            true,
            &[]
        ));
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].instance, "a");
    }

    /// A saved port that a server on this computer took as its game port afterwards
    /// (its row shows `127.0.0.1:<that port>`) is given up, and the server gets
    /// another, which then stays.
    #[test]
    fn a_port_this_computers_server_took_is_picked_again() {
        let list = [server("a", Some(25565)), server("b", Some(25566))];
        let mut saved = Vec::new();
        update_ports(&mut saved, CORE, &list, true, &[]);
        let (clash, kept) = (saved[0].port, saved[1].port);
        assert!(!release(&mut saved, &[25565]));
        assert!(release(&mut saved, &[clash]));
        assert_eq!(saved.len(), 1);
        // a list that only adds is enough: the server gets a port away from the clash
        assert!(update_ports(&mut saved, CORE, &list, false, &[clash]));
        let again = saved.iter().find(|e| e.instance == "a").unwrap();
        assert_ne!(again.port, clash);
        assert_ne!(again.port, kept);
        assert!(GAME_PORTS.contains(&again.port));
        assert_eq!(saved.iter().find(|e| e.instance == "b").unwrap().port, kept);
        let after = saved.clone();
        assert!(!update_ports(&mut saved, CORE, &list, false, &[clash]));
        assert_eq!(saved, after);
    }

    #[test]
    fn a_hidden_port_gets_no_address() {
        let mut saved = Vec::new();
        update_ports(&mut saved, CORE, &[server("a", None)], true, &[]);
        assert!(saved.is_empty());
        assert_eq!(magic_address(false, &saved, &server("a", None), None), None);
        assert_eq!(magic_address(true, &saved, &server("a", None), None), None);
        assert_eq!(
            magic_address(true, &saved, &server("a", Some(70000)), None),
            None
        );
    }

    #[test]
    fn a_server_on_this_computer_is_at_its_own_port() {
        let address = magic_address(true, &[], &server("a", Some(25570)), None).unwrap();
        assert_eq!(address, MagicAddress::ThisComputer { port: 25570 });
        assert_eq!(address.address(), "127.0.0.1:25570");
        assert_eq!(address.kind(), "this_computer");
    }

    #[test]
    fn a_forwarded_address_says_when_its_port_was_taken() {
        let mut saved = Vec::new();
        update_ports(&mut saved, CORE, &[server("a", Some(25565))], true, &[]);
        let port = saved[0].port;
        let row = server("a", Some(25565));
        assert_eq!(
            magic_address(false, &saved, &row, None),
            Some(MagicAddress::Forwarded { port, taken: None })
        );
        assert_eq!(
            magic_address(false, &saved, &row, Some(port)),
            Some(MagicAddress::Forwarded { port, taken: None })
        );
        assert_eq!(
            magic_address(false, &saved, &row, Some(40123)),
            Some(MagicAddress::Forwarded {
                port: 40123,
                taken: Some(port)
            })
        );
        // a server without a saved port yet
        assert_eq!(
            magic_address(false, &saved, &server("b", Some(1)), None),
            None
        );
    }

    #[test]
    fn ports_per_core_are_capped_and_hostile_entries_dropped() {
        let list: Vec<ServerInstance> = (0..MAX_PER_CORE + 10)
            .map(|i| server(&format!("s{i}"), Some(25565)))
            .collect();
        let mut saved = Vec::new();
        update_ports(&mut saved, CORE, &list, false, &[]);
        assert_eq!(saved.len(), MAX_PER_CORE);

        let good = MagicPort {
            instance: "ok".into(),
            port: 31000,
            target: 25565,
        };
        let mut saved = vec![
            good.clone(),
            MagicPort {
                port: 80,
                ..good.clone()
            },
            MagicPort {
                instance: "../x".into(),
                port: 31001,
                ..good.clone()
            },
            MagicPort {
                instance: "a".repeat(65),
                port: 31002,
                ..good.clone()
            },
            MagicPort {
                instance: "zero".into(),
                port: 31003,
                target: 0,
            },
            // a second entry for a server, and a second for a port
            MagicPort {
                port: 31004,
                ..good.clone()
            },
            MagicPort {
                instance: "other".into(),
                ..good.clone()
            },
        ];
        assert!(sanitize(&mut saved));
        assert_eq!(saved, [good]);
        // a server with an id no core makes gets no port
        let mut saved = Vec::new();
        update_ports(&mut saved, CORE, &[server("a/../b", Some(1))], true, &[]);
        assert!(saved.is_empty());
    }

    /// `count` servers named `<prefix>0`, `<prefix>1`, … with a game port.
    fn servers(prefix: &str, count: usize) -> Vec<ServerInstance> {
        (0..count)
            .map(|i| server(&format!("{prefix}{i}"), Some(25565)))
            .collect()
    }

    /// The saved ports a core at `url` gets for `list`, away from `taken`.
    fn saved_for(url: &str, list: &[ServerInstance], taken: &[u16]) -> Vec<MagicPort> {
        let mut saved = Vec::new();
        update_ports(&mut saved, url, list, false, taken);
        saved
    }

    fn served_of<'a>(served: &'a [Wanted], url: &str) -> Vec<&'a Wanted> {
        let prefix = format!("core:{url}/");
        served
            .iter()
            .filter(|w| w.id.starts_with(&prefix))
            .collect()
    }

    /// A core added by address, maybe hostile, lists more servers than there are
    /// forwarders, and sits before a friend's core: the friend's servers still get
    /// theirs, and two big cores split the forwarders evenly.
    #[test]
    fn one_core_cant_take_every_forwarder() {
        const HOSTILE: &str = "https://203.0.113.9:16662/";
        const FRIEND: &str = "https://198.51.100.7:16662/";
        let (hostile_url, friend_url) = (Url::parse(HOSTILE).unwrap(), Url::parse(FRIEND).unwrap());
        let many = servers("h", MAX_PER_CORE);
        let hostile = saved_for(HOSTILE, &many, &[]);
        assert_eq!(hostile.len(), MAX_PER_CORE);
        let taken: Vec<u16> = hostile.iter().map(|e| e.port).collect();
        let few = servers("f", 3);
        let friend = saved_for(FRIEND, &few, &taken);

        let cores = [
            HostCore {
                saved_url: HOSTILE,
                url: &hostile_url,
                saved: &hostile,
                listed: Some(many.as_slice()),
            },
            HostCore {
                saved_url: FRIEND,
                url: &friend_url,
                saved: &friend,
                listed: Some(few.as_slice()),
            },
        ];
        let (served, unserved) = wanted_forwarders(&cores);
        assert_eq!(served.len(), MAX_FORWARDERS);
        assert_eq!(served_of(&served, FRIEND).len(), 3);
        assert_eq!(served_of(&served, HOSTILE).len(), MAX_FORWARDERS - 3);
        assert_eq!(unserved.len(), MAX_PER_CORE - (MAX_FORWARDERS - 3));
        assert!(
            unserved
                .iter()
                .all(|id| id.starts_with(&format!("core:{HOSTILE}/")))
        );
        let ports: HashSet<u16> = served.iter().map(|w| w.port).collect();
        assert_eq!(ports.len(), served.len());

        // two cores with more servers than forwarders get half each
        let more = servers("g", MAX_PER_CORE);
        let other = saved_for(FRIEND, &more, &taken);
        let cores = [
            HostCore {
                saved: &hostile,
                ..cores[0]
            },
            HostCore {
                saved: &other,
                listed: Some(more.as_slice()),
                ..cores[1]
            },
        ];
        let (served, _) = wanted_forwarders(&cores);
        assert_eq!(served_of(&served, HOSTILE).len(), MAX_FORWARDERS / 2);
        assert_eq!(served_of(&served, FRIEND).len(), MAX_FORWARDERS / 2);
    }

    /// Saved ports of servers a core's latest list no longer shows (deleted, or private
    /// while signed out) only get the forwarders the listed servers of every core leave.
    #[test]
    fn listed_servers_come_before_stale_saved_ports() {
        const STALE: &str = "https://203.0.113.9:16662/";
        const BUSY: &str = "https://198.51.100.7:16662/";
        let (stale_url, busy_url) = (Url::parse(STALE).unwrap(), Url::parse(BUSY).unwrap());
        let old = servers("s", 40);
        let stale = saved_for(STALE, &old, &[]);
        let now = &old[..2];
        let taken: Vec<u16> = stale.iter().map(|e| e.port).collect();
        let busy_list = servers("b", MAX_FORWARDERS - 2);
        let busy = saved_for(BUSY, &busy_list, &taken);
        let mut cores = [
            HostCore {
                saved_url: STALE,
                url: &stale_url,
                saved: &stale,
                listed: Some(now),
            },
            HostCore {
                saved_url: BUSY,
                url: &busy_url,
                saved: &busy,
                listed: Some(busy_list.as_slice()),
            },
        ];
        let (served, unserved) = wanted_forwarders(&cores);
        assert_eq!(served.len(), MAX_FORWARDERS);
        assert_eq!(served_of(&served, BUSY).len(), MAX_FORWARDERS - 2);
        let kept: Vec<&str> = served_of(&served, STALE)
            .iter()
            .map(|w| w.id.as_str())
            .collect();
        assert_eq!(kept, [magic_id(STALE, "s0"), magic_id(STALE, "s1")]);
        assert_eq!(unserved.len(), 38);

        // before any list arrives, every saved port counts as listed
        cores[0].listed = None;
        let (served, _) = wanted_forwarders(&cores);
        assert_eq!(served_of(&served, STALE).len(), MAX_FORWARDERS / 2);
        // with room to spare, stale ones get forwarders too
        cores[1].saved = &[];
        let (served, unserved) = wanted_forwarders(&cores);
        cores[0].listed = Some(now);
        assert_eq!(served.len(), 40);
        assert!(unserved.is_empty());
        let (served, _) = wanted_forwarders(&cores);
        assert_eq!(served.len(), 40);
    }

    #[test]
    fn a_core_on_this_computer_gets_no_forwarders() {
        const HERE: &str = "https://127.0.0.1:16662/";
        let url = Url::parse(HERE).unwrap();
        let list = servers("a", 2);
        let saved = saved_for(HERE, &list, &[]);
        let (served, unserved) = wanted_forwarders(&[HostCore {
            saved_url: HERE,
            url: &url,
            saved: &saved,
            listed: Some(list.as_slice()),
        }]);
        assert!(served.is_empty());
        assert_eq!(unserved, [magic_id(HERE, "a0"), magic_id(HERE, "a1")]);
    }

    /// A core you aren't signed in to never drops a port on its own, but once it has
    /// [`MAX_PER_CORE`], the oldest port of a server it no longer lists makes room for a
    /// new server it does, so servers made and deleted over time don't leave the new
    /// ones without an address.
    #[test]
    fn a_full_core_makes_room_for_listed_servers() {
        let first = servers("s", MAX_PER_CORE);
        let mut saved = saved_for(CORE, &first, &[]);
        let ports: Vec<u16> = saved.iter().map(|e| e.port).collect();
        // s0 was deleted and "new" made; signed out, so the list isn't authoritative
        let mut later: Vec<ServerInstance> = first[1..].to_vec();
        later.push(server("new", Some(25565)));
        assert!(update_ports(&mut saved, CORE, &later, false, &[]));
        assert_eq!(saved.len(), MAX_PER_CORE);
        assert!(saved.iter().all(|e| e.instance != "s0"));
        assert!(saved.iter().any(|e| e.instance == "new"));
        // everyone else kept their port
        for (entry, port) in saved.iter().zip(&ports[1..]) {
            assert_eq!(entry.port, *port);
        }
        // a list that shows every saved server has nothing to make room with
        let mut full = later.clone();
        full.push(server("another", Some(25565)));
        assert!(!update_ports(&mut saved, CORE, &full, false, &[]));
        assert!(saved.iter().all(|e| e.instance != "another"));
    }

    #[test]
    fn a_target_is_never_this_computer() {
        let url = |s: &str| Url::parse(s).unwrap();
        assert_eq!(
            Target::new(&url("https://192.168.1.42:16662"), 25565),
            Some(Target {
                host: "192.168.1.42".into(),
                port: 25565
            })
        );
        let v6 = Target::new(&url("https://[2001:db8::1]:16662"), 25565).unwrap();
        assert_eq!(v6.host, "2001:db8::1");
        assert_eq!(v6.to_string(), "[2001:db8::1]:25565");
        assert_eq!(
            Target::new(&url("https://mc.example.com"), 25565)
                .unwrap()
                .to_string(),
            "mc.example.com:25565"
        );
        assert_eq!(Target::new(&url("https://192.168.1.42:16662"), 0), None);
        for local in [
            "http://127.0.0.1:16662",
            "http://127.0.0.2:16662",
            "http://localhost:16662",
            "http://LOCALHOST.:16662",
            "http://core.localhost:16662",
            "http://[::1]:16662",
            "http://0.0.0.0:16662",
            "http://[::]:16662",
            "http://[::ffff:127.0.0.1]:16662",
        ] {
            assert_eq!(Target::new(&url(local), 25565), None, "{local}");
        }
    }

    /// A fake server on 127.0.0.1 that sends back what it reads, with `tag` first.
    async fn echo_server(tag: &'static [u8]) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    return;
                };
                tokio::spawn(async move {
                    if socket.write_all(tag).await.is_err() {
                        return;
                    }
                    let mut buf = [0u8; 1024];
                    loop {
                        match socket.read(&mut buf).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => {
                                if socket.write_all(&buf[..n]).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                });
            }
        });
        (addr, task)
    }

    async fn talk(port: u16, tag: &[u8]) -> TcpStream {
        let mut client = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let mut got = vec![0u8; tag.len()];
        client.read_exact(&mut got).await.unwrap();
        assert_eq!(got, tag);
        client.write_all(b"hello").await.unwrap();
        let mut back = [0u8; 5];
        client.read_exact(&mut back).await.unwrap();
        assert_eq!(&back, b"hello");
        client
    }

    /// Whether a connection to `port` is closed (refused, or ended at once) within a
    /// few seconds.
    async fn closed(port: u16) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match TcpStream::connect(("127.0.0.1", port)).await {
                Err(_) => return true,
                Ok(mut socket) => {
                    let mut buf = [0u8; 1];
                    if let Ok(Ok(0) | Err(_)) =
                        tokio::time::timeout(Duration::from_millis(500), socket.read(&mut buf))
                            .await
                    {
                        return true;
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        false
    }

    /// A port nothing listens on right now.
    fn free_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    #[tokio::test]
    async fn a_forwarder_relays_both_ways_on_loopback_only() {
        let (server, _task) = echo_server(b"one").await;
        let mut forwarders = Forwarders::to_this_computer();
        let saved = free_port();
        let forwarder = forwarders
            .start(saved, loopback(server.port()), quiet())
            .await
            .unwrap();
        assert_eq!(
            forwarder.local_addr().ip(),
            std::net::IpAddr::from([127, 0, 0, 1])
        );
        assert_eq!(forwarder.port(), saved);
        assert_eq!(forwarder.taken(), None);
        let _client = talk(forwarder.port(), b"one").await;
        // several at once
        let _second = talk(forwarder.port(), b"one").await;
    }

    #[tokio::test]
    async fn a_taken_port_falls_back_to_a_free_one() {
        let (server, _task) = echo_server(b"one").await;
        let holder = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let saved = holder.local_addr().unwrap().port();
        let mut forwarders = Forwarders::to_this_computer();
        let forwarder = forwarders
            .start(saved, loopback(server.port()), quiet())
            .await
            .unwrap();
        assert_ne!(forwarder.port(), saved);
        assert_eq!(forwarder.taken(), Some(saved));
        assert_eq!(forwarder.saved(), saved);
        let _client = talk(forwarder.port(), b"one").await;
        drop(holder);
    }

    #[tokio::test]
    async fn a_new_target_takes_new_connections() {
        let (first, _a) = echo_server(b"one").await;
        let (second, _b) = echo_server(b"two").await;
        let mut forwarders = Forwarders::to_this_computer();
        let forwarder = forwarders
            .start(free_port(), loopback(first.port()), quiet())
            .await
            .unwrap();
        let mut open = talk(forwarder.port(), b"one").await;
        forwarder.set_target(loopback(second.port()));
        let _new = talk(forwarder.port(), b"two").await;
        // the open connection stays with the first server
        open.write_all(b"again").await.unwrap();
        let mut back = [0u8; 5];
        open.read_exact(&mut back).await.unwrap();
        assert_eq!(&back, b"again");
    }

    #[tokio::test]
    async fn dropping_a_forwarder_closes_it_and_its_connections() {
        let (server, _task) = echo_server(b"one").await;
        let mut forwarders = Forwarders::to_this_computer();
        let forwarder = forwarders
            .start(free_port(), loopback(server.port()), quiet())
            .await
            .unwrap();
        let port = forwarder.port();
        let mut open = talk(port, b"one").await;
        drop(forwarder);
        let mut buf = [0u8; 1];
        let ended = tokio::time::timeout(Duration::from_secs(5), open.read(&mut buf))
            .await
            .unwrap();
        assert!(matches!(ended, Ok(0) | Err(_)), "{ended:?}");
        assert!(closed(port).await);
    }

    #[tokio::test]
    async fn dropping_the_forwarders_closes_everything() {
        let (server, _task) = echo_server(b"one").await;
        let mut forwarders = Forwarders::to_this_computer();
        let forwarder = forwarders
            .start(free_port(), loopback(server.port()), quiet())
            .await
            .unwrap();
        let port = forwarder.port();
        let _open = talk(port, b"one").await;
        drop(forwarders);
        assert!(closed(port).await);
        drop(forwarder);
    }

    #[tokio::test]
    async fn connections_past_the_cap_are_closed() {
        let (server, _task) = echo_server(b"one").await;
        let mut forwarders = Forwarders::to_this_computer();
        let forwarder = forwarders
            .start(free_port(), loopback(server.port()), quiet())
            .await
            .unwrap();
        let mut open = Vec::new();
        for _ in 0..MAX_CONNECTIONS {
            open.push(talk(forwarder.port(), b"one").await);
        }
        let mut extra = TcpStream::connect(("127.0.0.1", forwarder.port()))
            .await
            .unwrap();
        let mut buf = [0u8; 1];
        let ended = tokio::time::timeout(Duration::from_secs(5), extra.read(&mut buf))
            .await
            .unwrap();
        assert!(matches!(ended, Ok(0) | Err(_)), "{ended:?}");
        // one closes: there is room again
        open.pop();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let _again = talk(forwarder.port(), b"one").await;
    }

    /// Whether `client`'s connection was closed by the forwarder instead of relayed.
    async fn refused(client: &mut TcpStream) -> bool {
        let mut buf = [0u8; 1];
        let ended = tokio::time::timeout(Duration::from_secs(5), client.read(&mut buf))
            .await
            .unwrap();
        matches!(ended, Ok(0) | Err(_))
    }

    #[tokio::test]
    async fn every_forwarder_shares_one_connection_budget() {
        let (server, _task) = echo_server(b"one").await;
        let mut forwarders = Forwarders::to_this_computer_with_budget(3);
        let first = forwarders
            .start(free_port(), loopback(server.port()), quiet())
            .await
            .unwrap();
        let second = forwarders
            .start(free_port(), loopback(server.port()), quiet())
            .await
            .unwrap();
        let mut open = vec![
            talk(first.port(), b"one").await,
            talk(first.port(), b"one").await,
            talk(second.port(), b"one").await,
        ];
        // each forwarder is under its own cap, but together they're at the budget
        for port in [first.port(), second.port()] {
            let mut extra = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            assert!(refused(&mut extra).await, "port {port}");
        }
        // one closes: its permit goes back, to either forwarder
        open.remove(0);
        tokio::time::sleep(Duration::from_millis(100)).await;
        let _again = talk(second.port(), b"one").await;
    }

    #[tokio::test]
    async fn a_stopped_server_closes_the_connection_and_says_why() {
        let errors = Arc::new(Mutex::new(Vec::new()));
        let seen = errors.clone();
        let on_error: OnError = Arc::new(move |target, err| {
            seen.lock().unwrap().push((target.port, err.kind()));
        });
        let stopped = free_port();
        let mut forwarders = Forwarders::to_this_computer();
        let forwarder = forwarders
            .start(free_port(), loopback(stopped), on_error)
            .await
            .unwrap();
        let started = Instant::now();
        let mut client = TcpStream::connect(("127.0.0.1", forwarder.port()))
            .await
            .unwrap();
        let mut buf = [0u8; 1];
        let ended = tokio::time::timeout(Duration::from_secs(5), client.read(&mut buf))
            .await
            .unwrap();
        assert!(matches!(ended, Ok(0) | Err(_)), "{ended:?}");
        assert!(started.elapsed() < Duration::from_secs(5));
        let errors = errors.lock().unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].0, stopped);
    }

    #[tokio::test]
    async fn a_forwarder_never_connects_to_itself() {
        let errors = Arc::new(Mutex::new(Vec::new()));
        let seen = errors.clone();
        let on_error: OnError = Arc::new(move |_, err| seen.lock().unwrap().push(err.kind()));
        let saved = free_port();
        let mut forwarders = Forwarders::to_this_computer();
        let forwarder = forwarders
            .start(saved, loopback(saved), on_error)
            .await
            .unwrap();
        assert_eq!(forwarder.port(), saved);
        assert!(closed(saved).await);
        assert!(!errors.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_forwarder_never_connects_to_this_computer() {
        let (server, _task) = echo_server(b"one").await;
        let errors = Arc::new(Mutex::new(Vec::new()));
        let seen = errors.clone();
        let on_error: OnError = Arc::new(move |_, err| seen.lock().unwrap().push(err.kind()));
        // the app's forwarders, not the tests'
        let mut forwarders = Forwarders::new();
        for host in ["127.0.0.1", "0.0.0.0", "::ffff:127.0.0.1"] {
            let target = Target {
                host: host.into(),
                port: server.port(),
            };
            let forwarder = forwarders
                .start(free_port(), target, on_error.clone())
                .await
                .unwrap();
            let mut client = TcpStream::connect(("127.0.0.1", forwarder.port()))
                .await
                .unwrap();
            let mut buf = [0u8; 3];
            let ended = tokio::time::timeout(Duration::from_secs(5), client.read(&mut buf))
                .await
                .unwrap();
            // closed without the fake server's greeting
            assert!(matches!(ended, Ok(0) | Err(_)), "{host}: {ended:?}");
        }
        assert_eq!(errors.lock().unwrap().len(), 3);
    }

    #[test]
    fn this_computer_covers_mapped_and_unspecified_addresses() {
        for here in [
            "127.0.0.1",
            "127.1.2.3",
            "::1",
            "0.0.0.0",
            "::",
            "::ffff:127.0.0.1",
        ] {
            assert!(is_this_computer(here.parse().unwrap()), "{here}");
        }
        for there in [
            "192.168.1.42",
            "10.0.0.1",
            "2001:db8::1",
            "::ffff:192.168.1.42",
        ] {
            assert!(!is_this_computer(there.parse().unwrap()), "{there}");
        }
    }

    /// A relay ends when its client leaves, even when the server never closes or
    /// answers (its host vanished without a word): otherwise each such connection
    /// would keep one of the forwarder's slots for good.
    #[tokio::test]
    async fn a_relay_ends_when_its_client_leaves() {
        // a fake server that greets each connection, then never reads or closes it
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server = listener.local_addr().unwrap();
        let _task = tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((mut socket, _)) = listener.accept().await {
                let _ = socket.write_all(b"hi").await;
                held.push(socket);
            }
        });
        let mut forwarders = Forwarders::to_this_computer();
        let forwarder = forwarders
            .start(free_port(), loopback(server.port()), quiet())
            .await
            .unwrap();
        // many more than the cap, one after another: each still gets through
        for round in 0..MAX_CONNECTIONS * 3 {
            let mut client = TcpStream::connect(("127.0.0.1", forwarder.port()))
                .await
                .unwrap();
            let mut got = [0u8; 2];
            tokio::time::timeout(Duration::from_secs(5), client.read_exact(&mut got))
                .await
                .unwrap_or_else(|_| panic!("round {round} timed out"))
                .unwrap_or_else(|err| panic!("round {round}: {err}"));
            assert_eq!(&got, b"hi");
            drop(client);
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}
