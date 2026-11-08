//! Minecraft's LAN worlds ("Open to LAN"): the game sends a UDP packet to the multicast
//! group `224.0.2.60:4445` every 1.5 seconds,
//! `[MOTD]<motd>[/MOTD][AD]<port>[/AD]`, UTF-8. Text outside the tags is ignored, the
//! MOTD may have `§` formatting codes, a port that isn't a number means 25565, and the
//! host is the packet's sender. See
//! <https://minecraft.wiki/w/Java_Edition_protocol/Server_List_Ping#Ping_via_LAN_(Open_to_LAN_in_Singleplayer)>.
//!
//! The listener is a `std::net::UdpSocket` that joins the group, on its own thread. It
//! binds the port with `SO_REUSEADDR` and `SO_REUSEPORT`, as Minecraft does, so the
//! game's own multiplayer screen can listen at the same time.

use std::{
    collections::BTreeMap,
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub const LAN_GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 2, 60);
pub const LAN_PORT: u16 = 4445;
/// Minecraft reads at most this many bytes of a packet; longer packets are refused.
pub const MAX_PACKET_BYTES: usize = 1024;
/// MOTDs are cut at this many characters.
pub const MAX_MOTD_CHARS: usize = 100;
/// A world that hasn't been announced for this long is gone.
pub const EXPIRE_AFTER: Duration = Duration::from_secs(6);
/// At most this many worlds are shown.
pub const MAX_WORLDS: usize = 32;
/// The default game port.
pub const DEFAULT_PORT: u16 = 25565;

/// One announcement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Announcement {
    /// Without formatting codes; `None` when the packet has no MOTD.
    pub motd: Option<String>,
    pub port: u16,
}

/// `text` without `§` formatting codes and control characters.
pub fn strip_formatting(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else if !c.is_control() {
            out.push(c);
        }
    }
    out
}

const MOTD_OPEN: &str = "[MOTD]";
const MOTD_CLOSE: &str = "[/MOTD]";
const AD_OPEN: &str = "[AD]";
const AD_CLOSE: &str = "[/AD]";

/// Reads a LAN packet as Minecraft does (`LanServerPinger.parseMotd` and
/// `parseAddress`). `None` for packets that aren't announcements: too long, without
/// exactly one `[/MOTD]`, or without `[AD]…[/AD]` after it.
pub fn parse_packet(bytes: &[u8]) -> Option<Announcement> {
    if bytes.is_empty() || bytes.len() > MAX_PACKET_BYTES {
        return None;
    }
    let text = String::from_utf8_lossy(bytes);
    let motd_end = text.find(MOTD_CLOSE)?;
    let after_motd = motd_end + MOTD_CLOSE.len();
    if text[after_motd..].contains(MOTD_CLOSE) {
        return None;
    }
    let ad_start = after_motd + text[after_motd..].find(AD_OPEN)? + AD_OPEN.len();
    let ad_end = ad_start + text[ad_start..].find(AD_CLOSE)?;
    let address = &text[ad_start..ad_end];
    let motd = text
        .find(MOTD_OPEN)
        .map(|open| open + MOTD_OPEN.len())
        .filter(|&open| open <= motd_end)
        .map(|open| &text[open..motd_end]);
    let address = address.trim();
    let port = if !address.is_empty() && address.chars().all(|c| c.is_ascii_digit()) {
        match address.parse::<u16>() {
            Ok(0) | Err(_) => return None,
            Ok(port) => port,
        }
    } else {
        DEFAULT_PORT
    };
    let motd = motd
        .map(|motd| {
            strip_formatting(motd)
                .trim()
                .chars()
                .take(MAX_MOTD_CHARS)
                .collect::<String>()
        })
        .filter(|motd| !motd.is_empty());
    Some(Announcement { motd, port })
}

/// A world on the network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanWorld {
    pub motd: Option<String>,
    pub host: IpAddr,
    pub port: u16,
    pub first_seen: Instant,
    pub last_seen: Instant,
}

impl LanWorld {
    pub fn title(&self) -> String {
        self.motd.clone().unwrap_or_else(|| "LAN world".into())
    }

    pub fn address(&self) -> String {
        match self.host {
            IpAddr::V6(ip) => format!("[{ip}]:{}", self.port),
            IpAddr::V4(ip) => format!("{ip}:{}", self.port),
        }
    }
}

/// The worlds heard recently, by host and port.
#[derive(Clone, Debug, Default)]
pub struct LanWorlds {
    worlds: BTreeMap<(IpAddr, u16), LanWorld>,
    /// When the first announcement arrived since listening started.
    pub heard_any: bool,
}

impl LanWorlds {
    /// Records an announcement. Returns whether the list changed in a way people see (a
    /// new world or a new MOTD); repeats of the same announcement only refresh it.
    pub fn record(&mut self, host: IpAddr, announcement: Announcement, now: Instant) -> bool {
        self.heard_any = true;
        let key = (host, announcement.port);
        if let Some(world) = self.worlds.get_mut(&key) {
            world.last_seen = now;
            if world.motd != announcement.motd {
                world.motd = announcement.motd;
                return true;
            }
            return false;
        }
        if self.worlds.len() >= MAX_WORLDS {
            return false;
        }
        self.worlds.insert(
            key,
            LanWorld {
                motd: announcement.motd,
                host,
                port: announcement.port,
                first_seen: now,
                last_seen: now,
            },
        );
        true
    }

    /// Drops the worlds not heard for [`EXPIRE_AFTER`]. Returns whether any went.
    pub fn expire(&mut self, now: Instant) -> bool {
        let before = self.worlds.len();
        self.worlds
            .retain(|_, world| now.saturating_duration_since(world.last_seen) < EXPIRE_AFTER);
        self.worlds.len() != before
    }

    /// The worlds, the first heard first.
    pub fn list(&self) -> Vec<LanWorld> {
        let mut worlds: Vec<LanWorld> = self.worlds.values().cloned().collect();
        worlds.sort_by_key(|w| (w.first_seen, w.host, w.port));
        worlds
    }

    pub fn clear(&mut self) {
        self.worlds.clear();
        self.heard_any = false;
    }
}

/// A running listener. Dropping it stops the thread within a fraction of a second.
pub struct LanListener {
    stop: Arc<AtomicBool>,
}

impl Drop for LanListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// The socket Minecraft's LAN worlds are heard on: `0.0.0.0:4445` with address and
/// port reuse, in the multicast group.
pub fn lan_socket() -> io::Result<UdpSocket> {
    let socket = reusable_udp_socket(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, LAN_PORT))?;
    socket.join_multicast_v4(&LAN_GROUP, &Ipv4Addr::UNSPECIFIED)?;
    Ok(socket)
}

#[cfg(unix)]
fn reusable_udp_socket(addr: SocketAddrV4) -> io::Result<UdpSocket> {
    use std::os::fd::FromRawFd as _;
    // SAFETY: plain socket calls on a descriptor this function owns; it is closed on
    // every error path, and handed to `UdpSocket` (which then owns it) on success.
    unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0);
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let fail = |fd: libc::c_int| {
            let err = io::Error::last_os_error();
            libc::close(fd);
            err
        };
        let one: libc::c_int = 1;
        for option in [libc::SO_REUSEADDR, libc::SO_REUSEPORT] {
            if libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                option,
                (&one as *const libc::c_int).cast(),
                std::mem::size_of::<libc::c_int>() as libc::socklen_t,
            ) != 0
            {
                return Err(fail(fd));
            }
        }
        let mut sockaddr: libc::sockaddr_in = std::mem::zeroed();
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            sockaddr.sin_len = std::mem::size_of::<libc::sockaddr_in>() as u8;
        }
        sockaddr.sin_family = libc::AF_INET as libc::sa_family_t;
        sockaddr.sin_port = addr.port().to_be();
        sockaddr.sin_addr = libc::in_addr {
            s_addr: u32::from(*addr.ip()).to_be(),
        };
        if libc::bind(
            fd,
            (&sockaddr as *const libc::sockaddr_in).cast(),
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        ) != 0
        {
            return Err(fail(fd));
        }
        Ok(UdpSocket::from_raw_fd(fd))
    }
}

#[cfg(not(unix))]
fn reusable_udp_socket(addr: SocketAddrV4) -> io::Result<UdpSocket> {
    UdpSocket::bind(addr)
}

/// Listens on `socket` on a new thread, calling `on_announcement` with each sender and
/// announcement, until the returned listener is dropped (or `on_announcement` returns
/// `false`). Malformed packets are ignored.
pub fn listen(
    socket: UdpSocket,
    on_announcement: impl Fn(IpAddr, Announcement) -> bool + Send + 'static,
) -> io::Result<LanListener> {
    socket.set_read_timeout(Some(Duration::from_millis(250)))?;
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    thread::Builder::new()
        .name("lan-worlds".into())
        .spawn(move || {
            // one byte more than allowed, to tell long packets from full ones
            let mut buffer = [0u8; MAX_PACKET_BYTES + 1];
            while !stopped.load(Ordering::Relaxed) {
                match socket.recv_from(&mut buffer) {
                    Ok((len, SocketAddr::V4(from))) => {
                        if let Some(announcement) = parse_packet(&buffer[..len])
                            && !on_announcement(IpAddr::V4(*from.ip()), announcement)
                        {
                            break;
                        }
                    }
                    Ok((len, SocketAddr::V6(from))) => {
                        if let Some(announcement) = parse_packet(&buffer[..len])
                            && !on_announcement(IpAddr::V6(*from.ip()), announcement)
                        {
                            break;
                        }
                    }
                    Err(err)
                        if matches!(
                            err.kind(),
                            io::ErrorKind::WouldBlock
                                | io::ErrorKind::TimedOut
                                | io::ErrorKind::Interrupted
                        ) => {}
                    // a refused or unreachable network (macOS without the local network
                    // permission) is retried slowly rather than ending the thread
                    Err(_) => thread::sleep(Duration::from_millis(500)),
                }
            }
        })?;
    Ok(LanListener { stop })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Option<Announcement> {
        parse_packet(text.as_bytes())
    }

    #[test]
    fn reads_announcements() {
        assert_eq!(
            parse("[MOTD]Steve - New World[/MOTD][AD]51234[/AD]"),
            Some(Announcement {
                motd: Some("Steve - New World".into()),
                port: 51234
            })
        );
        // text around the tags is ignored
        assert_eq!(
            parse("junk[MOTD]A[/MOTD]more[AD] 25570 [/AD]tail")
                .unwrap()
                .port,
            25570
        );
        // a port that isn't a number is the default port
        assert_eq!(parse("[MOTD]A[/MOTD][AD]host[/AD]").unwrap().port, 25565);
        assert_eq!(parse("[MOTD]A[/MOTD][AD][/AD]").unwrap().port, 25565);
        // no MOTD
        assert_eq!(
            parse("[/MOTD][AD]25566[/AD]"),
            Some(Announcement {
                motd: None,
                port: 25566
            })
        );
    }

    #[test]
    fn strips_formatting_codes() {
        assert_eq!(
            parse("[MOTD]§aGreen §lbold§r world§[/MOTD][AD]1[/AD]")
                .unwrap()
                .motd
                .unwrap(),
            "Green bold world"
        );
        assert_eq!(strip_formatting("a\u{7}b\nc§"), "abc");
        // only formatting: no MOTD
        assert_eq!(parse("[MOTD]§a§l[/MOTD][AD]1[/AD]").unwrap().motd, None);
    }

    #[test]
    fn refuses_malformed_and_oversized_packets() {
        for bad in [
            "",
            "hello",
            "[MOTD]A[/MOTD]",
            "[MOTD]A[/MOTD][AD]1",
            "[AD]25566[/AD]",
            "[MOTD]A[/MOTD][/MOTD][AD]1[/AD]",
            "[AD]1[/AD][MOTD]A[/MOTD]",
            "[MOTD]A[/MOTD][AD]0[/AD]",
            "[MOTD]A[/MOTD][AD]70000[/AD]",
            "[MOTD]A[/MOTD][AD]99999999999999999999[/AD]",
        ] {
            assert_eq!(parse(bad), None, "{bad:?}");
        }
        let long = format!("[MOTD]{}[/MOTD][AD]1[/AD]", "x".repeat(MAX_PACKET_BYTES));
        assert_eq!(parse(&long), None);
        // the longest packet that fits, with the MOTD cut short
        let motd = "y".repeat(MAX_PACKET_BYTES - "[MOTD][/MOTD][AD]1[/AD]".len());
        let fits = parse(&format!("[MOTD]{motd}[/MOTD][AD]1[/AD]")).unwrap();
        assert_eq!(fits.motd.unwrap().chars().count(), MAX_MOTD_CHARS);
        // invalid UTF-8 is read like Minecraft reads it
        let mut bytes = b"[MOTD]a".to_vec();
        bytes.push(0xff);
        bytes.extend_from_slice(b"b[/MOTD][AD]2[/AD]");
        assert_eq!(parse_packet(&bytes).unwrap().motd.unwrap(), "a\u{fffd}b");
    }

    #[test]
    fn worlds_are_debounced_and_expire() {
        let host: IpAddr = "192.168.1.20".parse().unwrap();
        let start = Instant::now();
        let mut worlds = LanWorlds::default();
        let a = |motd: &str, port| Announcement {
            motd: Some(motd.into()),
            port,
        };
        assert!(worlds.record(host, a("World", 1), start));
        // the same announcement again changes nothing people see
        assert!(!worlds.record(host, a("World", 1), start + Duration::from_secs(1)));
        assert!(worlds.record(host, a("Renamed", 1), start + Duration::from_secs(2)));
        assert!(worlds.record(host, a("Other", 2), start + Duration::from_secs(2)));
        assert_eq!(worlds.list().len(), 2);
        assert_eq!(worlds.list()[0].title(), "Renamed");
        assert_eq!(worlds.list()[0].address(), "192.168.1.20:1");
        assert!(!worlds.expire(start + Duration::from_secs(7)));
        assert!(worlds.expire(start + Duration::from_secs(8) + EXPIRE_AFTER));
        assert!(worlds.list().is_empty());
        assert!(worlds.heard_any);

        // a flood of worlds is capped
        let mut flood = LanWorlds::default();
        for port in 1..=(MAX_WORLDS as u16 + 10) {
            flood.record(host, a("x", port), start);
        }
        assert_eq!(flood.list().len(), MAX_WORLDS);
    }

    #[test]
    fn the_listener_hears_packets() {
        // a unicast socket on loopback stands in for the multicast group
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = socket.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let listener = listen(socket, move |host, announcement| {
            tx.send((host, announcement)).is_ok()
        })
        .unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        sender.send_to(b"not an announcement", addr).unwrap();
        sender
            .send_to(b"[MOTD]\xc2\xa7eLAN world[/MOTD][AD]40000[/AD]", addr)
            .unwrap();
        let (host, announcement) = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(host, IpAddr::V4(Ipv4Addr::LOCALHOST));
        assert_eq!(announcement.motd.as_deref(), Some("LAN world"));
        assert_eq!(announcement.port, 40000);
        drop(listener);
    }

    #[test]
    fn the_lan_port_can_be_shared() {
        // Minecraft's own multiplayer screen binds the same port; two listeners at once
        // must both work. Skipped where the port can't be bound at all.
        let Ok(first) = reusable_udp_socket(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)) else {
            return;
        };
        let port = first.local_addr().unwrap().port();
        assert!(reusable_udp_socket(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).is_ok());
    }
}
