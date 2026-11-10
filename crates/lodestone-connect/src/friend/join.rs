//! Joining a friend's core (launcher/docs/friends.md, "Joining"): knock on its lookup id,
//! probe the addresses in its record, open QUIC accepting only the certificate from the
//! friend's state, and prove who this side is with a friend hello.

use std::{
    net::SocketAddr,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{io::AsyncReadExt as _, task::JoinHandle};

use super::{
    crypto::{probe_tag, PersonKey, Secret},
    protocol::{hello_message, FriendStreamKind, HELLO_CONTEXT, HELLO_LABEL, MAX_INSTANCE_LEN},
    meet,
};
use crate::{
    addr::ConnectionKind,
    gather, info,
    info::SharedServer,
    join::{connect, keep_open, JoinOptions},
    portmap, probe,
    service::Service,
    tunnel::{self, TunnelStream, FRIEND_ALPN, HELLO_ANSWER},
    Error, Result,
};

/// The path of the list of servers a friend's core shares with you.
pub const SERVERS_PATH: &str = "/api/v1/friend/servers";

/// A friend's core, as this side knows it: its certificate's fingerprint (from the
/// friend's signed state) and the core secret it shares with this side.
#[derive(Clone)]
pub struct FriendCore {
    pub fingerprint: [u8; 32],
    pub secret: Secret,
}

impl std::fmt::Debug for FriendCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FriendCore")
            .field("fingerprint", &crate::hex(&self.fingerprint))
            .finish_non_exhaustive()
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// The error for "this core isn't sharing with you now", as invites say it.
fn offline() -> Error {
    Error::Service {
        status: 404,
        code: "host_offline".into(),
        message: "Your friend's core isn't sharing with you right now.".into(),
    }
}

/// Whether `core` is sharing with this side now (a presence check at the service).
pub async fn is_online(service: &Service, core: &FriendCore) -> Result<bool> {
    let now = now_secs();
    for lookup in meet::knock_lookups(&core.secret, now).iter().take(2) {
        if service.is_publishing(lookup).await? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A tunnel to a friend's core. Dropping it closes the tunnel.
pub struct FriendJoined {
    connection: quinn::Connection,
    endpoint: quinn::Endpoint,
    path: SocketAddr,
    mapping: Mutex<Option<portmap::Mapping>>,
    keep_open: JoinHandle<()>,
}

/// Connects to `core` as `me`. Errors: [`Error::Service`] with code `host_offline` when
/// the core isn't sharing with this side now; [`Error::NoPath`]; [`Error::WrongHost`];
/// [`Error::Tunnel`] when the core refused this side's hello (it's no longer a friend).
pub async fn join_friend(
    service: &Service,
    me: &PersonKey,
    core: &FriendCore,
    options: JoinOptions,
) -> Result<FriendJoined> {
    let gathered = gather::gather_async(options.gather).await?;
    let mapping = gathered.mapping;
    let candidates: Vec<SocketAddr> = gathered.candidates.iter().map(|c| c.addr).collect();
    let result = async {
        let now = now_secs();
        let mut found = None;
        for lookup in meet::knock_lookups(&core.secret, now) {
            let (knock, sealed) = meet::seal_knock(&core.secret, &lookup, &candidates, now)?;
            if let Some(record) = service.knock(&lookup, &sealed).await? {
                let host = meet::open_record(&core.secret, &lookup, &record, now)?;
                found = Some((knock, host));
                break;
            }
        }
        let (knock, host) = found.ok_or_else(offline)?;
        let tag = probe_tag(&core.secret, &knock.id);
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
                "Your friend's network answered, but not in a way that works. These two networks need a relay.".into()
            } else {
                "Nothing arrived from your friend's core. Their computer may be off, or these two networks need a relay.".into()
            })
        })?;
        let keep_open = keep_open(&socket, path, tag)?;
        let mut endpoint = tunnel::endpoint(socket, None)?;
        endpoint.set_default_client_config(tunnel::client_config_for(core.fingerprint, FRIEND_ALPN)?);
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
    let joined = FriendJoined {
        connection,
        endpoint,
        path,
        mapping: Mutex::new(mapping),
        keep_open,
    };
    joined.hello(me, &core.fingerprint).await?;
    Ok(joined)
}

impl FriendJoined {
    async fn hello(&self, me: &PersonKey, fingerprint: &[u8; 32]) -> Result<()> {
        let mut exporter = [0u8; 32];
        self.connection
            .export_keying_material(&mut exporter, HELLO_LABEL, b"")
            .map_err(|_| Error::Tunnel("couldn't prove who you are".into()))?;
        let key = me.public();
        let sig = me.sign(HELLO_CONTEXT, &hello_message(&exporter, fingerprint, &key));
        let mut message = Vec::with_capacity(1 + 32 + sig.len());
        message.push(FriendStreamKind::Hello as u8);
        message.extend_from_slice(&key.0);
        message.extend_from_slice(&sig);
        let (mut send, recv) = self
            .connection
            .open_bi()
            .await
            .map_err(|err| Error::Tunnel(err.to_string()))?;
        send.write_all(&message)
            .await
            .map_err(|err| Error::Tunnel(err.to_string()))?;
        let mut stream = tokio::io::join(recv, send);
        let mut answer = [0u8; 1];
        let read = tokio::time::timeout(Duration::from_secs(10), stream.read_exact(&mut answer))
            .await
            .map_err(|_| Error::Tunnel("your friend's core didn't answer".into()))?;
        match read {
            Ok(_) if answer[0] == HELLO_ANSWER => Ok(()),
            _ => Err(Error::Tunnel(
                "your friend's core didn't accept you. They may have removed you.".into(),
            )),
        }
    }

    /// The core's address the tunnel runs to.
    pub fn path(&self) -> SocketAddr {
        self.path
    }

    pub fn kind(&self) -> ConnectionKind {
        ConnectionKind::of(self.path)
    }

    pub fn rtt(&self) -> Duration {
        self.connection.rtt()
    }

    fn header(kind: FriendStreamKind, instance: Option<&str>) -> Result<Vec<u8>> {
        let mut header = vec![kind as u8];
        if let Some(instance) = instance {
            if instance.is_empty() || instance.len() > MAX_INSTANCE_LEN {
                return Err(Error::Other("That server's id is too long".into()));
            }
            header.push(instance.len() as u8);
            header.extend_from_slice(instance.as_bytes());
        }
        Ok(header)
    }

    async fn open(&self, header: &[u8]) -> Result<TunnelStream> {
        let (mut send, recv) = self
            .connection
            .open_bi()
            .await
            .map_err(|err| Error::Tunnel(err.to_string()))?;
        send.write_all(header)
            .await
            .map_err(|err| Error::Tunnel(err.to_string()))?;
        Ok(tokio::io::join(recv, send))
    }

    /// A stream starting with `header` as given: only for tests of what the core does
    /// with malformed or forbidden streams.
    #[doc(hidden)]
    pub async fn open_raw(&self, header: &[u8]) -> Result<TunnelStream> {
        self.open(header).await
    }

    /// An info stream: HTTP/1.1 to the routes about the servers shared with you.
    pub async fn open_info(&self) -> Result<TunnelStream> {
        self.open(&Self::header(FriendStreamKind::Info, None)?).await
    }

    /// A game stream to `instance`.
    pub async fn open_game(&self, instance: &str) -> Result<TunnelStream> {
        self.open(&Self::header(FriendStreamKind::Game, Some(instance))?).await
    }

    /// The servers this core shares with you.
    pub async fn servers(&self) -> Result<Vec<SharedServer>> {
        let (status, body) = info::get(self.open_info().await?, SERVERS_PATH).await?;
        if status != 200 {
            return Err(Error::Tunnel(format!(
                "your friend's core answered {status} when asked about its servers"
            )));
        }
        serde_json::from_slice(&body)
            .map_err(|err| Error::Tunnel(format!("your friend's server list was garbled: {err}")))
    }

    /// Listens on `listen` and passes each TCP connection to it through the tunnel:
    /// to `instance`'s game port with `Some`, or to the info routes with `None` (a core
    /// client with `http://<local_addr>` as its address).
    pub async fn forward(&self, instance: Option<&str>, listen: SocketAddr) -> Result<crate::join::Forward> {
        let header = match instance {
            Some(instance) => Self::header(FriendStreamKind::Game, Some(instance))?,
            None => Self::header(FriendStreamKind::Info, None)?,
        };
        crate::join::Forward::start(self.connection.clone(), header, listen).await
    }

    pub async fn closed(&self) -> String {
        self.connection.closed().await.to_string()
    }

    pub fn is_closed(&self) -> bool {
        self.connection.close_reason().is_some()
    }

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

impl Drop for FriendJoined {
    fn drop(&mut self) {
        self.connection.close(0u32.into(), b"bye");
        self.keep_open.abort();
        if let Some(mapping) = self.mapping.lock().expect("not poisoned").take() {
            std::thread::spawn(move || mapping.remove());
        }
    }
}
