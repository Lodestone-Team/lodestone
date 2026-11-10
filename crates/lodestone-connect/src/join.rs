//! Joining a shared server: [`join`] connects to the host of an invite, and [`Joined`]
//! opens streams through the tunnel or forwards a local TCP port into it.
//!
//! ```no_run
//! # async fn example() -> lodestone_connect::Result<()> {
//! use lodestone_connect::{join::{join, JoinOptions}, Invite, StreamKind};
//! let invite = Invite::parse("https://connect.lodestone.cc/j/ABCDEFGH23#...")?;
//! let joined = join(&invite, JoinOptions::default()).await?;
//! let server = joined.server_info().await?; // name, version, loader, modpack
//! let game = joined.forward(StreamKind::Game, "127.0.0.1:0".parse().unwrap()).await?;
//! println!("{} on {}: join {} in Minecraft", server.name, server.game_version, game.local_addr());
//! joined.closed().await; // until the tunnel ends
//! # Ok(()) }
//! ```
//!
//! Everything here runs on a Tokio runtime (multi-threaded or current-thread with IO
//! and time enabled).

use std::{
    net::{SocketAddr, UdpSocket},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tokio::{io::AsyncReadExt as _, task::JoinHandle};

use crate::{
    addr::{to_socket, ConnectionKind},
    gather::{self, GatherOptions},
    info::{self, SharedServer},
    portmap, probe,
    tunnel::{self, StreamKind, TunnelStream, HELLO_ANSWER, TOKEN_LEN},
    Error, Invite, Result,
};

/// How to join.
#[derive(Clone, Copy, Debug)]
pub struct JoinOptions {
    pub gather: GatherOptions,
    /// How long to probe the host's addresses.
    pub probe_for: Duration,
    /// How long to try the encrypted connection once a path is found.
    pub connect_for: Duration,
}

impl Default for JoinOptions {
    fn default() -> Self {
        Self {
            gather: GatherOptions::default(),
            probe_for: Duration::from_secs(45),
            connect_for: Duration::from_secs(20),
        }
    }
}

/// A tunnel to a shared server. Dropping it closes the tunnel.
pub struct Joined {
    connection: quinn::Connection,
    endpoint: quinn::Endpoint,
    token: [u8; TOKEN_LEN],
    path: SocketAddr,
    mapping: Mutex<Option<portmap::Mapping>>,
    keep_open: JoinHandle<()>,
}

/// Connects to the host of `invite`: asks the service for the host's addresses, probes
/// them, opens the encrypted tunnel (checking the host's certificate against the
/// invite) and proves to the host that this side has the link.
///
/// Errors: [`Error::Service`] with code `not_found` or `host_offline` when the invite
/// has ended or its host isn't sharing right now; [`Error::NoPath`] when no probe got
/// through (these two networks would need a relay); [`Error::WrongHost`].
pub async fn join(invite: &Invite, options: JoinOptions) -> Result<Joined> {
    let gathered = gather::gather_async(options.gather).await?;
    let mapping = gathered.mapping;
    let result = async {
        let host = invite
            .service
            .join(&invite.code, &gathered.candidates.iter().map(|c| c.addr).collect::<Vec<_>>())
            .await?;
        let tag = probe::tag(&invite.token);
        let socket = gathered.socket;
        let probe_for = options.probe_for;
        let (socket, heard) = tokio::task::spawn_blocking(move || {
            let heard = probe::until_heard(&socket, &host, tag, probe_for);
            (socket, heard)
        })
        .await
        .map_err(|err| Error::Other(err.to_string()))?;
        let path = heard.map_err(|heard| {
            Error::NoPath(if heard {
                "The host's network answered, but not in a way that works. These two networks need a relay.".into()
            } else {
                "Nothing arrived from the host. Their computer may be off, or these two networks need a relay.".into()
            })
        })?;
        let keep_open = keep_open(&socket, path, tag)?;
        let mut endpoint = tunnel::endpoint(socket, None)?;
        endpoint.set_default_client_config(tunnel::client_config(invite.fingerprint)?);
        let connection = match connect(&endpoint, path, options.connect_for).await {
            Ok(connection) => connection,
            Err(err) => {
                keep_open.abort();
                return Err(err);
            }
        };
        Ok::<_, Error>((connection, endpoint, path, keep_open))
    }
    .await;
    let (connection, endpoint, path, keep_open) = match result {
        Ok(found) => found,
        Err(err) => {
            if let Some(mapping) = mapping {
                let _ = tokio::task::spawn_blocking(move || mapping.remove()).await;
            }
            return Err(err);
        }
    };
    let joined = Joined {
        connection,
        endpoint,
        token: invite.token,
        path,
        mapping: Mutex::new(mapping),
        keep_open,
    };
    joined.hello().await?;
    Ok(joined)
}

/// Keeps answering along the chosen path for a while as QUIC takes over the socket, so
/// the routers keep it open and a host still probing hears back.
pub(crate) fn keep_open(socket: &UdpSocket, peer: SocketAddr, tag: [u8; 8]) -> Result<JoinHandle<()>> {
    let socket = socket.try_clone()?;
    Ok(tokio::spawn(async move {
        let until = tokio::time::Instant::now() + Duration::from_secs(30);
        while tokio::time::Instant::now() < until {
            let _ = socket.send_to(&probe::message(tag, probe::ACK), to_socket(peer));
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }))
}

pub(crate) async fn connect(
    endpoint: &quinn::Endpoint,
    path: SocketAddr,
    how_long: Duration,
) -> Result<quinn::Connection> {
    let until = Instant::now() + how_long;
    loop {
        let attempt = endpoint
            .connect(to_socket(path), tunnel::SERVER_NAME)
            .map_err(|err| Error::Tunnel(err.to_string()))?;
        let last = match tokio::time::timeout(Duration::from_secs(5), attempt).await {
            Ok(Ok(connection)) => return Ok(connection),
            Ok(Err(err)) if err.to_string().contains(tunnel::WRONG_HOST) => {
                return Err(Error::WrongHost);
            }
            Ok(Err(err)) => err.to_string(),
            Err(_) => "no answer".to_string(),
        };
        if Instant::now() >= until {
            return Err(Error::Tunnel(format!(
                "probes got through but the encrypted connection didn't ({last})"
            )));
        }
    }
}

impl Joined {
    /// The host's address the tunnel runs to.
    pub fn path(&self) -> SocketAddr {
        self.path
    }

    /// How this side is connected to the host.
    pub fn kind(&self) -> ConnectionKind {
        ConnectionKind::of(self.path)
    }

    /// The current round trip time.
    pub fn rtt(&self) -> Duration {
        self.connection.rtt()
    }

    /// Opens a stream of `kind` to the host.
    pub async fn open(&self, kind: StreamKind) -> Result<TunnelStream> {
        let (mut send, recv) = self
            .connection
            .open_bi()
            .await
            .map_err(|err| Error::Tunnel(err.to_string()))?;
        send.write_all(&tunnel::header(&self.token, kind))
            .await
            .map_err(|err| Error::Tunnel(err.to_string()))?;
        Ok(tokio::io::join(recv, send))
    }

    async fn hello(&self) -> Result<()> {
        let mut stream = self.open(StreamKind::Hello).await?;
        let mut answer = [0u8; 1];
        let read = tokio::time::timeout(Duration::from_secs(10), stream.read_exact(&mut answer))
            .await
            .map_err(|_| Error::Tunnel("the host didn't answer".into()))?;
        match read {
            Ok(_) if answer[0] == HELLO_ANSWER => Ok(()),
            _ => Err(Error::Tunnel(
                "the host refused this link. Ask for a new one.".into(),
            )),
        }
    }

    /// The shared server's name, Minecraft version, loader and whether it has a client
    /// modpack, over an info stream.
    pub async fn server_info(&self) -> Result<SharedServer> {
        info::shared_server(self.open(StreamKind::Info).await?).await
    }

    /// Listens on `listen` (such as `127.0.0.1:0`) and passes each TCP connection made
    /// to it through the tunnel as a stream of `kind`: [`StreamKind::Game`] for
    /// Minecraft, [`StreamKind::Info`] for an HTTP client of the info routes (a core
    /// client with `http://<local_addr>` as its address). Stops when the [`Forward`]
    /// is dropped or the tunnel closes.
    pub async fn forward(&self, kind: StreamKind, listen: SocketAddr) -> Result<Forward> {
        Forward::start(
            self.connection.clone(),
            tunnel::header(&self.token, kind).to_vec(),
            listen,
        )
        .await
    }

    /// Waits until the tunnel closes; returns why, for people.
    pub async fn closed(&self) -> String {
        self.connection.closed().await.to_string()
    }

    /// Whether the tunnel has closed.
    pub fn is_closed(&self) -> bool {
        self.connection.close_reason().is_some()
    }

    /// Closes the tunnel and removes the router's forwarding.
    pub async fn close(self) {
        self.connection.close(0u32.into(), b"bye");
        self.keep_open.abort();
        let mapping = self.mapping.lock().expect("not poisoned").take();
        if let Some(mapping) = mapping {
            let _ = tokio::task::spawn_blocking(move || mapping.remove()).await;
        }
        self.endpoint.wait_idle().await;
    }
}

impl Drop for Joined {
    fn drop(&mut self) {
        self.connection.close(0u32.into(), b"bye");
        self.keep_open.abort();
        if let Some(mapping) = self.mapping.lock().expect("not poisoned").take() {
            std::thread::spawn(move || mapping.remove());
        }
    }
}

/// A local TCP port that forwards into the tunnel. Dropping every clone stops it.
#[derive(Clone)]
pub struct Forward {
    local_addr: SocketAddr,
    task: Arc<JoinHandle<()>>,
}

impl Forward {
    /// Listens on `listen` and passes each TCP connection to it through `connection` as
    /// a stream starting with `header`, until dropped or the connection closes.
    pub(crate) async fn start(
        connection: quinn::Connection,
        header: Vec<u8>,
        listen: SocketAddr,
    ) -> Result<Forward> {
        let listener = tokio::net::TcpListener::bind(listen).await?;
        let local_addr = listener.local_addr()?;
        let accepting = connection.clone();
        let header: Arc<[u8]> = header.into();
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut tcp, _)) = listener.accept().await else {
                    continue;
                };
                let connection = accepting.clone();
                let header = header.clone();
                tokio::spawn(async move {
                    let Ok((mut send, recv)) = connection.open_bi().await else {
                        return;
                    };
                    if send.write_all(&header).await.is_err() {
                        return;
                    }
                    let mut stream = tokio::io::join(recv, send);
                    let _ = tokio::io::copy_bidirectional(&mut tcp, &mut stream).await;
                });
            }
        });
        // stop listening once the tunnel closes
        let abort = task.abort_handle();
        tokio::spawn(async move {
            connection.closed().await;
            abort.abort();
        });
        Ok(Forward {
            local_addr,
            task: Arc::new(task),
        })
    }

    /// Where to connect: Minecraft's server address, or the core client's base URL as
    /// `http://<local_addr>`.
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

impl Drop for Forward {
    fn drop(&mut self) {
        if Arc::strong_count(&self.task) == 1 {
            self.task.abort();
        }
    }
}
