//! Sharing the worlds this person opens to LAN with their friends
//! (launcher/docs/friends.md, "LAN worlds").
//!
//! The Launcher shares them as a core does its servers: it has a friend certificate of
//! its own ([`Friends::lan_identity`], sealed in the data folder with the friend file's
//! key), lists it in the person's state as one of their cores, marked as a launcher
//! ([`State::launchers`](lodestone_connect::friend::state::State::launchers)), and runs a
//! friend host with it ([`LanHost`]) while a world is open. Friends find it, knock,
//! punch through and say hello exactly as with a core: the same records, lookups and
//! hello, so only friends on the list (not hidden, not pending) get an answer at all.
//!
//! - **Presence is "open now".** The host runs only while a world is open, so a
//!   friend's presence check on the launcher's lookup id says whether a world is open,
//!   and nothing needs a new state when worlds open or close.
//! - **Info** answers `GET /api/v1/friend/servers` with the open worlds, as
//!   [`SharedServer`]s (nothing else answers).
//! - **Game** streams go to `127.0.0.1:<the world's port>`, never anywhere else.
//!
//! Launchers from before this see the launcher as one more core of their friend's, and
//! its worlds as servers on it, which they can join the same way.

use std::{
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use lodestone_connect::{
    GatherOptions, Service, SharedServer, TunnelStream,
    friend::{
        PublicKey,
        crypto::{self as friend_crypto, Role, core_secret},
        host::{
            CoreIdentity, FriendAccess, FriendHandler, FriendHost, FriendHostConfig,
            FriendHostStatus, KnockMemory,
        },
        join::SERVERS_PATH,
    },
    host::{HostState, pipe},
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

/// For the app that runs the host.
pub use lodestone_connect::friend::{
    PublicKey as FriendKey,
    host::{CoreIdentity as LanIdentity, FriendAccess as LanFriend, KnockMemory as LanKnocks},
};

use super::{Book, FriendStatus, Friends, FriendsError, MyCore, hex, lodestone_unhex};
use crate::cores::lan_open::OpenWorld;

/// [`MyCore::source`] of the Launcher sharing LAN worlds.
pub const LAN_SOURCE: &str = "lodestone-launcher:lan-worlds";
/// Its name in the state (friends' launchers say "LAN worlds" instead).
pub const LAN_CORE_NAME: &str = "Lodestone Launcher";
/// The file in the data folder with its certificate, sealed with the friend file's key.
pub const IDENTITY_FILE: &str = "friend_lan_host.json";
/// The most friends it shares with, as a core: each gets one or two records, and the
/// service takes at most 256 from one host.
pub const MAX_LAN_FRIENDS: usize = 120;
/// How long the host waits before trying to start again after it couldn't.
const RETRY_AFTER: Duration = Duration::from_secs(30);
/// How long a friend has to send an info request's head, and its largest size.
const REQUEST_WITHIN: Duration = Duration::from_secs(10);
const MAX_REQUEST: usize = 8 * 1024;

#[derive(serde::Serialize, serde::Deserialize)]
struct IdentityWire {
    v: u32,
    sealed: String,
}

impl Friends {
    fn lan_identity_path(&self) -> PathBuf {
        self.path.with_file_name(IDENTITY_FILE)
    }

    /// This launcher's friend certificate for LAN worlds: the saved one, else (or with
    /// `renew`) a new one, saved sealed with the friend file's key.
    pub fn lan_identity(&self, renew: bool) -> Result<CoreIdentity, FriendsError> {
        let path = self.lan_identity_path();
        let context = IDENTITY_FILE.as_bytes();
        if !renew && let Ok(text) = std::fs::read_to_string(&path) {
            let opened = serde_json::from_str::<IdentityWire>(&text)
                .ok()
                .filter(|wire| wire.v == 1)
                .and_then(|wire| lodestone_unhex(&wire.sealed))
                .and_then(|sealed| {
                    friend_crypto::open(&self.file_key, Role::File, context, &sealed)
                })
                .and_then(|plain| String::from_utf8(plain).ok())
                .and_then(|json| CoreIdentity::from_json(&json).ok());
            match opened {
                Some(identity) => return Ok(identity),
                // sealed with keys this list doesn't have (a new identity), or damaged:
                // a new certificate, which the state then lists in its place
                None => eprintln!(
                    "lodestone-launcher: the LAN worlds certificate is unreadable; making a new one"
                ),
            }
        }
        let identity = CoreIdentity::generate()?;
        let sealed = friend_crypto::seal(
            &self.file_key,
            Role::File,
            context,
            identity.to_json().as_bytes(),
        )?;
        let text = serde_json::to_string(&IdentityWire {
            v: 1,
            sealed: hex(&sealed),
        })
        .expect("serializable");
        crate::storage::write_atomic(&path, text.as_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(identity)
    }

    /// This launcher in the state as sharing LAN worlds, if it is.
    pub fn lan_launcher(&self) -> Option<&MyCore> {
        lan_launcher_of(&self.book)
    }

    /// Lists this launcher in the person's state as sharing LAN worlds, with its
    /// certificate (a new one if the saved one was retired). Returns the certificate,
    /// and whether the state changed (publish it with [`Friends::publish_states`]).
    pub fn start_lan_sharing(&mut self) -> Result<(CoreIdentity, bool), FriendsError> {
        let mut identity = self.lan_identity(false)?;
        for renew in [false, true] {
            if renew {
                identity = self.lan_identity(true)?;
            }
            match self.add_my_core(
                &hex(&identity.fingerprint()),
                LAN_CORE_NAME,
                Some(LAN_SOURCE),
            ) {
                Ok(changed) => return Ok((identity, changed)),
                Err(FriendsError::CoreRetired) if !renew => continue,
                Err(err) => return Err(err),
            }
        }
        Err(FriendsError::CoreRetired)
    }

    /// Takes this launcher out of the state (not retired: turning sharing on again
    /// lists the same certificate). Returns whether the state changed.
    pub fn stop_lan_sharing(&mut self) -> Result<bool, FriendsError> {
        self.modify(|book| {
            let before = book.my_cores.len();
            book.my_cores
                .retain(|core| core.source.as_deref() != Some(LAN_SOURCE));
            let changed = book.my_cores.len() != before;
            if changed {
                book.new_state();
            }
            Ok(changed)
        })
    }
}

/// The launcher sharing LAN worlds in `book`'s state, if it is.
pub fn lan_launcher_of(book: &Book) -> Option<&MyCore> {
    book.my_cores
        .iter()
        .find(|core| core.source.as_deref() == Some(LAN_SOURCE))
}

/// The friends a launcher with the certificate `fingerprint` shares LAN worlds with,
/// and the core secret it shares with each: friends who accepted (not pending or
/// retired) and whom the person doesn't hide from. At most [`MAX_LAN_FRIENDS`].
pub fn lan_friends(book: &Book, me: &PublicKey, fingerprint: &[u8; 32]) -> Vec<FriendAccess> {
    if book.retired {
        return Vec::new();
    }
    book.friends
        .iter()
        .filter(|f| f.status == FriendStatus::Active && !f.hidden)
        .filter_map(|f| {
            let key = f.public_key()?;
            let pair = f.pair_secret()?;
            Some(FriendAccess {
                key,
                secret: core_secret(&pair, me, &key, fingerprint),
            })
        })
        .take(MAX_LAN_FRIENDS)
        .collect()
}

/// A world as friends see it: a server on the launcher's "core".
pub fn shared_world(world: &OpenWorld, host_name: &str, fingerprint: &[u8; 32]) -> SharedServer {
    SharedServer {
        core_name: host_name.to_string(),
        core_uuid: format!("lan-{}", &hex(fingerprint)[..16]),
        instance_uuid: world.id.clone(),
        name: world.name.clone(),
        game_version: world.game_version.clone(),
        loader: world.loader.clone(),
        loader_version: world.loader_version.clone(),
        modpack: false,
        state: Some("Running".into()),
        player_count: world.player_count,
        max_player_count: world.max_player_count,
        online_mode: world.online_mode,
    }
}

/// What a [`LanHost`] serves: the open worlds, as the window last set them.
#[derive(Clone)]
struct Served {
    worlds: Arc<Mutex<Vec<OpenWorld>>>,
    host_name: Arc<Mutex<String>>,
    fingerprint: [u8; 32],
}

impl Served {
    fn port_of(&self, id: &str) -> Option<u16> {
        self.worlds
            .lock()
            .expect("not poisoned")
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.port)
    }

    fn list(&self) -> Vec<SharedServer> {
        let host_name = self.host_name.lock().expect("not poisoned").clone();
        self.worlds
            .lock()
            .expect("not poisoned")
            .iter()
            .map(|world| shared_world(world, &host_name, &self.fingerprint))
            .collect()
    }
}

impl FriendHandler for Served {
    fn game(&self, stream: TunnelStream, _friend: PublicKey, instance: String) {
        // only an open world's port, only on this computer
        if let Some(port) = self.port_of(&instance) {
            tokio::spawn(pipe(stream, SocketAddr::from((Ipv4Addr::LOCALHOST, port))));
        }
    }

    fn info(&self, stream: TunnelStream, _friend: PublicKey) {
        let served = self.clone();
        tokio::spawn(answer_info(stream, served));
    }
}

/// The request line of an HTTP request whose head is `head`: method and path.
fn request_line(head: &[u8]) -> Option<(String, String)> {
    let text = std::str::from_utf8(head).ok()?;
    let mut parts = text.lines().next()?.split_whitespace();
    Some((parts.next()?.to_string(), parts.next()?.to_string()))
}

async fn answer_info(mut stream: TunnelStream, served: Served) {
    let mut head = Vec::new();
    let mut buf = [0u8; 1024];
    let read = tokio::time::timeout(REQUEST_WITHIN, async {
        while !head.windows(4).any(|w| w == b"\r\n\r\n") {
            let n = stream.read(&mut buf).await.unwrap_or(0);
            if n == 0 || head.len() + n > MAX_REQUEST {
                return false;
            }
            head.extend_from_slice(&buf[..n]);
        }
        true
    })
    .await;
    if read != Ok(true) {
        return;
    }
    let (status, body) = match request_line(&head) {
        Some((method, path)) if method == "GET" && path == SERVERS_PATH => (
            "200 OK",
            serde_json::to_string(&served.list()).expect("serializable"),
        ),
        _ => (
            "404 Not Found",
            r#"{"error":"not_found","message":"Nothing here."}"#.to_string(),
        ),
    };
    let answer = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(answer.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// How the launcher's sharing of LAN worlds is going.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LanHostState {
    /// Collecting addresses and reaching the service.
    #[default]
    Starting,
    /// Publishing for friends; `connected` are joined now.
    Sharing { connected: usize },
    /// Couldn't start or reach the service; trying again.
    Problem(String),
}

/// The friend host the launcher runs while a world is open: on a thread of its own with
/// a small Tokio runtime, as tunnels do. Dropping it stops it (the records go at once,
/// friends are disconnected).
pub struct LanHost {
    served: Served,
    friends: Arc<Mutex<Vec<FriendAccess>>>,
    host: Arc<Mutex<Option<FriendHost>>>,
    state: Arc<Mutex<LanHostState>>,
    stop: tokio::sync::watch::Sender<bool>,
}

/// How to run a [`LanHost`].
#[derive(Clone, Debug, Default)]
pub struct LanHostConfig {
    pub service: Service,
    pub gather: GatherOptions,
    /// Keep one for as long as the launcher runs and give it to every host it starts,
    /// so a knock can't be replayed after the host stops and starts again.
    pub knocks: KnockMemory,
}

impl LanHost {
    /// Starts sharing `worlds` with `friends`, as `identity`, in the background.
    pub fn start(
        identity: CoreIdentity,
        friends: Vec<FriendAccess>,
        worlds: Vec<OpenWorld>,
        host_name: String,
        config: LanHostConfig,
    ) -> std::io::Result<Self> {
        let served = Served {
            worlds: Arc::new(Mutex::new(worlds)),
            host_name: Arc::new(Mutex::new(host_name)),
            fingerprint: identity.fingerprint(),
        };
        let friends = Arc::new(Mutex::new(friends));
        let host: Arc<Mutex<Option<FriendHost>>> = Arc::new(Mutex::new(None));
        let state = Arc::new(Mutex::new(LanHostState::Starting));
        let (stop, mut stopped) = tokio::sync::watch::channel(false);
        let (t_served, t_friends, t_host, t_state) =
            (served.clone(), friends.clone(), host.clone(), state.clone());
        std::thread::Builder::new()
            .name("lodestone-lan-worlds".into())
            .spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                else {
                    return;
                };
                runtime.block_on(async move {
                    loop {
                        let list = t_friends.lock().expect("not poisoned").clone();
                        let host_config = FriendHostConfig {
                            service: config.service.clone(),
                            gather: config.gather,
                            knocks: config.knocks.clone(),
                        };
                        let started = tokio::select! {
                            started = FriendHost::start(&identity, list, host_config, t_served.clone()) => started,
                            _ = stopped.wait_for(|stop| *stop) => return,
                        };
                        match started {
                            Ok(started) => {
                                // friends changed while it started
                                started.set_friends(t_friends.lock().expect("not poisoned").clone());
                                *t_host.lock().expect("not poisoned") = Some(started);
                                let _ = stopped.wait_for(|stop| *stop).await;
                                let running = t_host.lock().expect("not poisoned").take();
                                if let Some(running) = running {
                                    running.stop().await;
                                }
                                return;
                            }
                            Err(err) => {
                                *t_state.lock().expect("not poisoned") =
                                    LanHostState::Problem(err.to_string());
                                tokio::select! {
                                    _ = tokio::time::sleep(RETRY_AFTER) => {}
                                    _ = stopped.wait_for(|stop| *stop) => return,
                                }
                            }
                        }
                    }
                });
            })?;
        Ok(Self {
            served,
            friends,
            host,
            state,
            stop,
        })
    }

    /// The certificate fingerprint friends pin.
    pub fn fingerprint(&self) -> [u8; 32] {
        self.served.fingerprint
    }

    /// Replaces the friends it shares with: those no longer on it are disconnected at
    /// once, and the records are published again.
    pub fn set_friends(&self, friends: Vec<FriendAccess>) {
        *self.friends.lock().expect("not poisoned") = friends.clone();
        if let Some(host) = self.host.lock().expect("not poisoned").as_ref() {
            host.set_friends(friends);
        }
    }

    /// Replaces the open worlds it serves. A world no longer open takes no new streams
    /// (one already joined goes on until it ends, as the game's own connection would).
    pub fn set_worlds(&self, worlds: Vec<OpenWorld>, host_name: &str) {
        *self.served.worlds.lock().expect("not poisoned") = worlds;
        *self.served.host_name.lock().expect("not poisoned") = host_name.to_string();
    }

    /// How it's going.
    pub fn state(&self) -> LanHostState {
        if let Some(host) = self.host.lock().expect("not poisoned").as_ref() {
            let status: FriendHostStatus = host.status();
            return match (status.state, status.problem) {
                (HostState::Sharing, _) => LanHostState::Sharing {
                    connected: status.connected.len(),
                },
                (_, Some(problem)) => LanHostState::Problem(problem),
                _ => LanHostState::Starting,
            };
        }
        self.state.lock().expect("not poisoned").clone()
    }
}

impl Drop for LanHost {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_request_lines() {
        assert_eq!(
            request_line(b"GET /api/v1/friend/servers HTTP/1.1\r\nHost: x\r\n\r\n"),
            Some(("GET".into(), "/api/v1/friend/servers".into()))
        );
        assert_eq!(request_line(b"\r\n\r\n"), None);
        assert_eq!(request_line(&[0xff, 0xfe]), None);
    }

    #[test]
    fn worlds_look_like_servers_to_friends() {
        let world = OpenWorld {
            id: "lan-0123456789ab".into(),
            name: "Ana - New World".into(),
            port: 51234,
            game_version: "1.21.4".into(),
            loader: "vanilla".into(),
            loader_version: None,
            online_mode: Some(true),
            player_count: Some(1),
            max_player_count: Some(8),
            instance: None,
        };
        let shared = shared_world(&world, "Ana", &[7; 32]);
        assert_eq!(shared.instance_uuid, world.id);
        assert_eq!(shared.name, world.name);
        assert_eq!(shared.state.as_deref(), Some("Running"));
        assert!(!shared.modpack);
        assert_eq!(shared.online_mode, Some(true));
        // a launcher from before `online_mode` reads it as a server
        let json = serde_json::to_value(&shared).unwrap();
        assert_eq!(json["online_mode"], true);
        let back: SharedServer = serde_json::from_value(json).unwrap();
        assert_eq!(back, shared);
        // and the port never leaves this computer
        assert!(!serde_json::to_string(&shared).unwrap().contains("51234"));
    }
}
