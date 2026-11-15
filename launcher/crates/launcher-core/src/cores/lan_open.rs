//! The worlds this computer has open to LAN, for sharing them with friends
//! (launcher/docs/friends.md, "LAN worlds"; the sharing itself is
//! [`crate::friend_list::lan`]).
//!
//! Two things say a world is open:
//!
//! - **Its announcement** ([`super::lan`]), from an address of this computer
//!   ([`is_this_computer`]): any game, whoever launched it. It stops within a few seconds
//!   of the world closing ([`super::lan::EXPIRE_AFTER`]).
//! - **The game's log**, for games this launcher started: Minecraft writes
//!   `Started serving on <port>` when a world opens to LAN ([`started_serving_port`]).
//!   The launcher knows that game's instance: its version, loader and account. This
//!   works where announcements don't arrive (macOS before the Local Network permission
//!   is given). The world goes when the game exits, or when its port stops answering.
//!
//! Either way, a world counts only once `127.0.0.1:<port>` answers a Minecraft status
//! ping ([`ping`]), which also gives its version and MOTD; it's asked again every
//! [`RECHECK_EVERY`], and a port that fails [`FAILS_TO_CLOSE`] times in a row is closed.
//! So an announcement naming some other port of this computer shares nothing that
//! isn't a Minecraft server, and a world is only ever reached at 127.0.0.1.

use std::{
    collections::BTreeMap,
    io::{self, Read as _, Write as _},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket},
    time::{Duration, Instant},
};

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::lan::EXPIRE_AFTER;

/// How often an open world's port is pinged again.
pub const RECHECK_EVERY: Duration = Duration::from_secs(15);
/// After this many failed pings in a row, a world is closed.
pub const FAILS_TO_CLOSE: u32 = 2;
/// How long a status ping may take.
pub const PING_TIMEOUT: Duration = Duration::from_secs(3);
/// The most worlds shared at once.
pub const MAX_OPEN: usize = 8;
/// The largest status answer read.
const MAX_STATUS: usize = 64 * 1024;

/// The port in Minecraft's `Started serving on <port>` (the integrated server, when a
/// world opens to LAN), if `line` is that line.
pub fn started_serving_port(line: &str) -> Option<u16> {
    let rest = line.split("Started serving on ").nth(1)?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    match digits.parse::<u16>() {
        Ok(0) | Err(_) => None,
        Ok(port) => Some(port),
    }
}

/// Whether `ip` is one of this computer's addresses: loopback, or an address a socket
/// can be bound to here (a LAN announcement from this computer comes from its address
/// on the network).
pub fn is_this_computer(ip: IpAddr) -> bool {
    ip.is_loopback() || (!ip.is_unspecified() && UdpSocket::bind(SocketAddr::new(ip, 0)).is_ok())
}

/// What a Minecraft server says to a status ping.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    /// Such as `1.21.4`.
    pub version: Option<String>,
    pub protocol: Option<i64>,
    /// The MOTD as plain text: a LAN world's is `<player> - <world name>`.
    pub motd: Option<String>,
    pub online: Option<u32>,
    pub max: Option<u32>,
    /// Forge or NeoForge said so (`forgeData` or `modinfo`).
    pub forge: bool,
}

fn write_varint(out: &mut Vec<u8>, value: i32) {
    let mut value = value as u32;
    loop {
        if value & !0x7f == 0 {
            out.push(value as u8);
            return;
        }
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
}

fn read_varint(read: &mut impl io::Read) -> io::Result<i32> {
    let mut value: u32 = 0;
    for i in 0..5 {
        let mut byte = [0u8];
        read.read_exact(&mut byte)?;
        value |= u32::from(byte[0] & 0x7f) << (7 * i);
        if byte[0] & 0x80 == 0 {
            return Ok(value as i32);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "VarInt too long",
    ))
}

fn packet(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = Vec::with_capacity(body.len() + 2);
    write_varint(&mut inner, id);
    inner.extend_from_slice(body);
    let mut out = Vec::with_capacity(inner.len() + 3);
    write_varint(&mut out, inner.len() as i32);
    out.extend_from_slice(&inner);
    out
}

/// The handshake (to the status state) and the status request.
fn status_request(port: u16) -> Vec<u8> {
    let host = "127.0.0.1";
    let mut handshake = Vec::new();
    write_varint(&mut handshake, -1); // any protocol: only asking
    write_varint(&mut handshake, host.len() as i32);
    handshake.extend_from_slice(host.as_bytes());
    handshake.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut handshake, 1); // status
    let mut out = packet(0, &handshake);
    out.extend_from_slice(&packet(0, &[]));
    out
}

/// Reads a status answer's JSON from `read`.
fn read_status(read: &mut impl io::Read) -> io::Result<Status> {
    let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidData, what.to_string());
    let len = read_varint(read)?;
    if len <= 0 || len as usize > MAX_STATUS {
        return Err(bad("not a status answer"));
    }
    let mut body = vec![0u8; len as usize];
    read.read_exact(&mut body)?;
    let mut body = &body[..];
    if read_varint(&mut body)? != 0 {
        return Err(bad("not a status answer"));
    }
    let text_len = read_varint(&mut body)?;
    if text_len < 0 || text_len as usize > body.len() {
        return Err(bad("not a status answer"));
    }
    let value: Value = serde_json::from_slice(&body[..text_len as usize])
        .map_err(|_| bad("the status isn't JSON"))?;
    parse_status(&value).ok_or_else(|| bad("not a Minecraft status"))
}

/// A status answer's JSON. `None` if it isn't one (no `version` object).
pub fn parse_status(value: &Value) -> Option<Status> {
    let version = value.get("version")?.as_object()?;
    let motd = value
        .get("description")
        .map(|d| crate::mc_text::parse_component(d).text)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    let count = |key: &str| {
        value
            .get("players")
            .and_then(|p| p.get(key))
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    };
    Some(Status {
        version: version
            .get("name")
            .and_then(Value::as_str)
            .map(|v| v.chars().filter(|c| !c.is_control()).take(64).collect()),
        protocol: version.get("protocol").and_then(Value::as_i64),
        motd,
        online: count("online"),
        max: count("max"),
        forge: value.get("forgeData").is_some() || value.get("modinfo").is_some(),
    })
}

/// Asks `127.0.0.1:port` for its status, as Minecraft's server list does. An error
/// when nothing answers there, or not as a Minecraft server.
pub fn ping(port: u16, timeout: Duration) -> io::Result<Status> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&addr, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream.write_all(&status_request(port))?;
    let status = read_status(&mut (&stream).take(MAX_STATUS as u64 + 8))?;
    Ok(status)
}

/// What the launcher knows of a game it started that opened a world to LAN.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameInfo {
    /// The instance's id.
    pub instance: String,
    pub game_version: String,
    /// `vanilla`, `fabric`, `forge`, `neoforge` or `quilt`.
    pub loader: String,
    pub loader_version: Option<String>,
    /// The game runs with a Microsoft account: joining needs one too.
    pub online_mode: Option<bool>,
}

/// A world open to LAN on this computer, as friends see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenWorld {
    /// Stable while the world keeps its name: `lan-` and 12 hex digits of its name's
    /// SHA-256, so a friend's Magic localhost port stays the same each time it opens.
    pub id: String,
    /// Its MOTD as plain text (`<player> - <world name>`), or empty when it has none.
    pub name: String,
    pub port: u16,
    /// Empty when unknown.
    pub game_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub online_mode: Option<bool>,
    pub player_count: Option<u32>,
    pub max_player_count: Option<u32>,
    /// The instance of this launcher's that opened it, if it did.
    pub instance: Option<String>,
}

#[derive(Clone, Debug)]
struct Announced {
    motd: Option<String>,
    last: Instant,
}

#[derive(Clone, Debug)]
struct Checked {
    status: Option<Status>,
    at: Instant,
    failures: u32,
}

/// The worlds this computer has open to LAN: what announcements and game logs said,
/// and what pinging their ports found.
#[derive(Clone, Debug, Default)]
pub struct OpenWorlds {
    announced: BTreeMap<u16, Announced>,
    games: BTreeMap<u16, GameInfo>,
    checked: BTreeMap<u16, Checked>,
}

impl OpenWorlds {
    /// An announcement from this computer.
    pub fn announced(&mut self, port: u16, motd: Option<String>, now: Instant) {
        if self.announced.len() >= MAX_OPEN * 2 && !self.announced.contains_key(&port) {
            return;
        }
        self.announced.insert(port, Announced { motd, last: now });
    }

    /// A game this launcher started logged that it serves a world on `port`.
    pub fn game_serving(&mut self, port: u16, info: GameInfo) {
        self.games.retain(|_, game| game.instance != info.instance);
        // ask again now: the port may have answered for an earlier world
        self.checked.remove(&port);
        self.games.insert(port, info);
    }

    /// A game this launcher started exited: its worlds are closed.
    pub fn game_exited(&mut self, instance: &str) {
        let ports: Vec<u16> = self
            .games
            .iter()
            .filter(|(_, game)| game.instance == instance)
            .map(|(port, _)| *port)
            .collect();
        for port in ports {
            self.games.remove(&port);
            if !self.announced.contains_key(&port) {
                self.checked.remove(&port);
            }
        }
    }

    /// Forgets announcements not heard for [`EXPIRE_AFTER`], and pings for ports
    /// nothing names any more.
    pub fn expire(&mut self, now: Instant) {
        self.announced
            .retain(|_, a| now.saturating_duration_since(a.last) < EXPIRE_AFTER);
        let (announced, games) = (&self.announced, &self.games);
        self.checked
            .retain(|port, _| announced.contains_key(port) || games.contains_key(port));
    }

    /// The ports to ping now: named but never pinged, or pinged [`RECHECK_EVERY`] ago.
    pub fn due(&self, now: Instant) -> Vec<u16> {
        self.announced
            .keys()
            .chain(self.games.keys())
            .copied()
            .filter(|port| {
                self.checked
                    .get(port)
                    .is_none_or(|c| now.saturating_duration_since(c.at) >= RECHECK_EVERY)
            })
            .collect::<std::collections::BTreeSet<u16>>()
            .into_iter()
            .collect()
    }

    /// What pinging `port` found: its status, or `None` when it didn't answer as a
    /// Minecraft server. A game's world whose port fails [`FAILS_TO_CLOSE`] times in a
    /// row was closed (the game goes on, back in the menu or singleplayer).
    pub fn checked(&mut self, port: u16, status: Option<Status>, now: Instant) {
        if !self.announced.contains_key(&port) && !self.games.contains_key(&port) {
            return;
        }
        let entry = self.checked.entry(port).or_insert(Checked {
            status: None,
            at: now,
            failures: 0,
        });
        entry.at = now;
        match status {
            Some(status) => {
                entry.status = Some(status);
                entry.failures = 0;
            }
            None => {
                entry.failures += 1;
                if entry.failures >= FAILS_TO_CLOSE {
                    entry.status = None;
                    self.games.remove(&port);
                }
            }
        }
    }

    /// The open worlds: named by an announcement or a game, and answering pings. By
    /// port; at most [`MAX_OPEN`].
    pub fn open(&self) -> Vec<OpenWorld> {
        let mut worlds: Vec<OpenWorld> = Vec::new();
        for (port, checked) in &self.checked {
            let Some(status) = &checked.status else {
                continue;
            };
            let game = self.games.get(port);
            let announced = self.announced.get(port);
            if game.is_none() && announced.is_none() {
                continue;
            }
            let name = status
                .motd
                .clone()
                .or_else(|| {
                    announced
                        .and_then(|a| a.motd.as_deref())
                        .map(crate::mc_text::strip)
                })
                .unwrap_or_default();
            let (game_version, loader, loader_version) = match game {
                Some(game) => (
                    game.game_version.clone(),
                    game.loader.clone(),
                    game.loader_version.clone(),
                ),
                None => (
                    status.version.clone().unwrap_or_default(),
                    if status.forge { "forge" } else { "vanilla" }.to_string(),
                    None,
                ),
            };
            worlds.push(OpenWorld {
                id: world_id(&name),
                name,
                port: *port,
                game_version,
                loader,
                loader_version,
                online_mode: game.and_then(|g| g.online_mode),
                player_count: status.online,
                max_player_count: status.max,
                instance: game.map(|g| g.instance.clone()),
            });
            if worlds.len() >= MAX_OPEN {
                break;
            }
        }
        // two open worlds with the same name: the later ones by port too
        let mut seen = std::collections::HashSet::new();
        for world in &mut worlds {
            if !seen.insert(world.id.clone()) {
                world.id = format!("{}-{}", world.id, world.port);
            }
        }
        worlds
    }

    pub fn is_empty(&self) -> bool {
        self.announced.is_empty() && self.games.is_empty()
    }
}

/// [`OpenWorld::id`] for a world called `name`.
pub fn world_id(name: &str) -> String {
    let digest = Sha256::digest(name.as_bytes());
    let hex: String = digest[..6].iter().map(|b| format!("{b:02x}")).collect();
    format!("lan-{hex}")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn reads_the_log_line() {
        assert_eq!(
            started_serving_port("Started serving on 51234"),
            Some(51234)
        );
        assert_eq!(
            started_serving_port("[12:00:01] [Server thread/INFO]: Started serving on 25565"),
            Some(25565)
        );
        assert_eq!(started_serving_port("Started serving on 0"), None);
        assert_eq!(started_serving_port("Started serving on 99999"), None);
        assert_eq!(
            started_serving_port("Starting Minecraft server on *:25565"),
            None
        );
        assert_eq!(started_serving_port("Started serving on port"), None);
    }

    #[test]
    fn knows_this_computers_addresses() {
        assert!(is_this_computer(Ipv4Addr::LOCALHOST.into()));
        assert!(!is_this_computer(Ipv4Addr::UNSPECIFIED.into()));
        // a documentation address is nobody's here
        assert!(!is_this_computer("192.0.2.77".parse().unwrap()));
    }

    #[test]
    fn varints_round_trip() {
        for value in [0, 1, 127, 128, 255, 25565, 2_097_151, i32::MAX, -1] {
            let mut out = Vec::new();
            write_varint(&mut out, value);
            assert_eq!(read_varint(&mut &out[..]).unwrap(), value);
        }
        let mut out = Vec::new();
        write_varint(&mut out, -1);
        assert_eq!(out, vec![0xff, 0xff, 0xff, 0xff, 0x0f]);
        assert!(read_varint(&mut &[0xff; 6][..]).is_err());
    }

    #[test]
    fn reads_status_answers() {
        let status = parse_status(&serde_json::json!({
            "version": { "name": "1.21.4", "protocol": 769 },
            "players": { "max": 8, "online": 1 },
            "description": { "text": "Ana - ", "extra": [{ "text": "New World", "color": "gold" }] }
        }))
        .unwrap();
        assert_eq!(status.version.as_deref(), Some("1.21.4"));
        assert_eq!(status.protocol, Some(769));
        assert_eq!(status.motd.as_deref(), Some("Ana - New World"));
        assert_eq!((status.online, status.max), (Some(1), Some(8)));
        assert!(!status.forge);
        let forge = parse_status(&serde_json::json!({
            "version": { "name": "1.20.1", "protocol": 763 },
            "description": "§aModded",
            "forgeData": {}
        }))
        .unwrap();
        assert!(forge.forge);
        assert_eq!(forge.motd.as_deref(), Some("Modded"));
        assert_eq!(parse_status(&serde_json::json!({ "hello": 1 })), None);
    }

    /// A fake LAN world: answers one status ping per connection, as Minecraft does.
    pub(crate) fn fake_world(motd: &str, version: &str) -> (u16, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let json = serde_json::json!({
            "version": { "name": version, "protocol": 769 },
            "players": { "max": 8, "online": 1 },
            "description": { "text": motd },
        })
        .to_string();
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming().take(1) {
                let mut stream = stream.unwrap();
                // handshake and status request
                let len = read_varint(&mut stream).unwrap();
                let mut skip = vec![0u8; len as usize];
                stream.read_exact(&mut skip).unwrap();
                let len = read_varint(&mut stream).unwrap();
                let mut skip = vec![0u8; len as usize];
                stream.read_exact(&mut skip).unwrap();
                let mut body = Vec::new();
                write_varint(&mut body, json.len() as i32);
                body.extend_from_slice(json.as_bytes());
                stream.write_all(&packet(0, &body)).unwrap();
            }
        });
        (port, handle)
    }

    #[test]
    fn pings_a_world() {
        let (port, server) = fake_world("Ana - New World", "1.21.4");
        let status = ping(port, PING_TIMEOUT).unwrap();
        assert_eq!(status.motd.as_deref(), Some("Ana - New World"));
        assert_eq!(status.version.as_deref(), Some("1.21.4"));
        server.join().unwrap();

        // something that isn't Minecraft
        let other = TcpListener::bind("127.0.0.1:0").unwrap();
        let other_port = other.local_addr().unwrap().port();
        let talker = std::thread::spawn(move || {
            let (mut stream, _) = other.accept().unwrap();
            let _ = stream.write_all(b"SSH-2.0-OpenSSH_9.6\r\n");
        });
        assert!(ping(other_port, PING_TIMEOUT).is_err());
        talker.join().unwrap();
        // nothing at all
        drop(TcpListener::bind("127.0.0.1:0").unwrap());
    }

    /// The whole path for a fake world on this computer: its announcement (as the
    /// listener hears it from this computer's address), a real status ping of its
    /// port, and the world it makes.
    #[test]
    fn an_announced_world_on_this_computer_opens_after_a_ping() {
        let (port, server) = fake_world("Ana - Island", "1.21.4");
        let packet = format!("[MOTD]Ana - Island[/MOTD][AD]{port}[/AD]");
        let announcement = super::super::lan::parse_packet(packet.as_bytes()).unwrap();
        let from: IpAddr = Ipv4Addr::LOCALHOST.into();
        assert!(is_this_computer(from));
        let now = Instant::now();
        let mut worlds = OpenWorlds::default();
        worlds.announced(announcement.port, announcement.motd, now);
        for due in worlds.due(now) {
            worlds.checked(due, ping(due, PING_TIMEOUT).ok(), now);
        }
        server.join().unwrap();
        let open = worlds.open();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].port, port);
        assert_eq!(open[0].name, "Ana - Island");
        assert_eq!(open[0].game_version, "1.21.4");
    }

    fn status(motd: &str) -> Status {
        Status {
            version: Some("1.21.4".into()),
            motd: Some(motd.into()),
            ..Status::default()
        }
    }

    #[test]
    fn a_world_is_open_only_once_its_port_answers() {
        let start = Instant::now();
        let mut worlds = OpenWorlds::default();
        worlds.announced(40000, Some("§aAna - New World".into()), start);
        assert!(worlds.open().is_empty(), "not pinged yet");
        assert_eq!(worlds.due(start), vec![40000]);
        worlds.checked(40000, None, start);
        assert!(worlds.open().is_empty(), "didn't answer as Minecraft");
        worlds.checked(40000, Some(status("Ana - New World")), start);
        let open = worlds.open();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].name, "Ana - New World");
        assert_eq!(open[0].loader, "vanilla");
        assert_eq!(open[0].game_version, "1.21.4");
        assert_eq!(open[0].id, world_id("Ana - New World"));
        assert!(worlds.due(start + Duration::from_secs(1)).is_empty());
        assert_eq!(worlds.due(start + RECHECK_EVERY), vec![40000]);

        // the announcement stops: the world closes
        worlds.expire(start + EXPIRE_AFTER + Duration::from_secs(1));
        assert!(worlds.open().is_empty());
        assert!(worlds.is_empty());
    }

    #[test]
    fn a_games_world_closes_with_the_game_or_its_port() {
        let start = Instant::now();
        let mut worlds = OpenWorlds::default();
        let game = GameInfo {
            instance: "inst".into(),
            game_version: "1.20.1".into(),
            loader: "fabric".into(),
            loader_version: Some("0.16.9".into()),
            online_mode: Some(true),
        };
        worlds.game_serving(51000, game.clone());
        worlds.checked(51000, Some(status("Ana - Base")), start);
        let open = worlds.open();
        assert_eq!(open[0].loader, "fabric");
        assert_eq!(open[0].game_version, "1.20.1");
        assert_eq!(open[0].online_mode, Some(true));
        assert_eq!(open[0].instance.as_deref(), Some("inst"));
        // no announcement needed, and expiring announcements leaves it
        worlds.expire(start + Duration::from_secs(60));
        assert_eq!(worlds.open().len(), 1);
        // one failed ping isn't enough to close it, two are
        worlds.checked(51000, None, start + RECHECK_EVERY);
        assert_eq!(worlds.open().len(), 1);
        worlds.checked(51000, None, start + RECHECK_EVERY * 2);
        assert!(worlds.open().is_empty());
        assert!(worlds.is_empty());

        // the game opens it again, then exits
        worlds.game_serving(51001, game);
        worlds.checked(51001, Some(status("Ana - Base")), start);
        assert_eq!(worlds.open().len(), 1);
        worlds.game_exited("inst");
        assert!(worlds.open().is_empty());
    }

    #[test]
    fn worlds_with_the_same_name_get_their_own_ids() {
        let start = Instant::now();
        let mut worlds = OpenWorlds::default();
        for port in [1000, 1001] {
            worlds.announced(port, Some("World".into()), start);
            worlds.checked(port, Some(status("World")), start);
        }
        let open = worlds.open();
        assert_eq!(open.len(), 2);
        assert_ne!(open[0].id, open[1].id);
        assert!(open.iter().all(|w| w.id.len() <= 64));
    }
}
