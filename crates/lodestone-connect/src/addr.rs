//! Addresses: the dual-stack form a socket uses, and what kind of path an address is.

use std::{
    io,
    net::{IpAddr, Ipv6Addr, SocketAddr, SocketAddrV6, UdpSocket},
};

use serde::{Deserialize, Serialize};

/// IPv4 addresses as themselves, not as `::ffff:a.b.c.d` (how a dual-stack socket
/// reports them).
pub fn canonical(addr: SocketAddr) -> SocketAddr {
    SocketAddr::new(addr.ip().to_canonical(), addr.port())
}

/// The form a dual-stack socket sends to: IPv4 as `::ffff:a.b.c.d`.
pub fn to_socket(addr: SocketAddr) -> SocketAddr {
    match addr {
        SocketAddr::V4(v4) => {
            SocketAddr::V6(SocketAddrV6::new(v4.ip().to_ipv6_mapped(), v4.port(), 0, 0))
        }
        v6 => v6,
    }
}

/// One UDP socket for IPv4 and IPv6, on a port the system picks.
pub fn dual_stack_socket() -> io::Result<UdpSocket> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV6,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    socket.set_only_v6(false)?;
    socket.bind(&SocketAddr::from((Ipv6Addr::UNSPECIFIED, 0)).into())?;
    Ok(socket.into())
}

/// The address this computer uses to reach `probe`. No packet is sent.
pub(crate) fn route_to(bind: SocketAddr, probe: SocketAddr) -> Option<IpAddr> {
    let socket = UdpSocket::bind(bind).ok()?;
    socket.connect(probe).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified()).then_some(ip)
}

/// Whether a host may probe `addr`, an address the other side named, and let it
/// connect. Not port 0, and not an address no one else can be at, or one that would
/// make the host send to many: unspecified, `0.0.0.0/8`, broadcast, multicast,
/// link-local, documentation or reserved ranges, or loopback (unless `loopback`: both
/// sides on this computer, in tests). Local network addresses (`10/8`, `172.16/12`,
/// `192.168/16`, `fc00::/7`) stay, for friends on the same network.
pub fn is_probe_target(addr: SocketAddr, loopback: bool) -> bool {
    let addr = canonical(addr);
    if addr.port() == 0 {
        return false;
    }
    match addr.ip() {
        IpAddr::V4(v4) => {
            let first = v4.octets()[0];
            !(v4.is_unspecified()
                || first == 0
                || first >= 240 // reserved, and broadcast
                || v4.is_multicast()
                || v4.is_link_local()
                || v4.is_documentation()
                || (v4.is_loopback() && !loopback))
        }
        IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            !(v6.is_unspecified()
                || v6.is_multicast()
                || (first & 0xffc0) == 0xfe80 // link-local
                || (first == 0x2001 && v6.segments()[1] == 0x0db8) // documentation
                || (v6.is_loopback() && !loopback))
        }
    }
}

/// How a friend is connected to the host, from the address the tunnel runs over.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionKind {
    /// Both on this computer (loopback).
    SameComputer,
    /// On the same local network.
    LocalNetwork,
    /// Directly over IPv6.
    Ipv6,
    /// Directly over the internet (IPv4, through the routers).
    Internet,
}

impl ConnectionKind {
    /// The kind of path to `addr`, the other side's address.
    pub fn of(addr: SocketAddr) -> Self {
        match canonical(addr).ip() {
            ip if ip.is_loopback() => ConnectionKind::SameComputer,
            IpAddr::V4(v4) if v4.is_private() || v4.is_link_local() => ConnectionKind::LocalNetwork,
            // unique local (fc00::/7) and link-local (fe80::/10) addresses
            IpAddr::V6(v6) if (v6.segments()[0] & 0xfe00) == 0xfc00 => ConnectionKind::LocalNetwork,
            IpAddr::V6(v6) if (v6.segments()[0] & 0xffc0) == 0xfe80 => ConnectionKind::LocalNetwork,
            IpAddr::V6(_) => ConnectionKind::Ipv6,
            IpAddr::V4(_) => ConnectionKind::Internet,
        }
    }

    /// A few words for people: "the same computer", "the local network", ...
    pub fn describe(self) -> &'static str {
        match self {
            ConnectionKind::SameComputer => "the same computer",
            ConnectionKind::LocalNetwork => "the local network",
            ConnectionKind::Ipv6 => "IPv6",
            ConnectionKind::Internet => "the internet",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_map_to_and_from_the_dual_stack_form() {
        let v4: SocketAddr = "203.0.113.5:7".parse().unwrap();
        assert!(to_socket(v4).is_ipv6());
        assert_eq!(canonical(to_socket(v4)), v4);
    }

    #[test]
    fn hosts_probe_only_addresses_someone_else_can_be_at() {
        let target = |text: &str| is_probe_target(text.parse().unwrap(), false);
        for good in [
            "198.51.99.1:40000",
            "192.168.1.4:40000",
            "10.0.0.2:40000",
            "172.16.5.5:40000",
            "100.64.1.1:40000",
            "[2606:4700::1]:40000",
            "[fd00::1]:40000",
            "[::ffff:81.2.69.1]:40000",
        ] {
            assert!(target(good), "{good}");
        }
        for bad in [
            "127.0.0.1:40000",
            "[::ffff:127.0.0.1]:40000",
            "[::1]:40000",
            "0.0.0.0:40000",
            "0.1.2.3:40000",
            "255.255.255.255:40000",
            "240.0.0.1:40000",
            "224.0.0.251:5353",
            "[ff02::1]:40000",
            "169.254.1.1:40000",
            "[fe80::1]:40000",
            "192.0.2.1:40000",
            "203.0.113.5:40000",
            "[2001:db8::1]:40000",
            "[::]:40000",
            "81.2.69.1:0",
        ] {
            assert!(!target(bad), "{bad}");
        }
        assert!(is_probe_target("127.0.0.1:40000".parse().unwrap(), true));
    }

    #[test]
    fn kinds_of_path() {
        let kind = |text: &str| ConnectionKind::of(text.parse().unwrap());
        assert_eq!(kind("127.0.0.1:1"), ConnectionKind::SameComputer);
        assert_eq!(kind("[::ffff:192.168.1.4]:1"), ConnectionKind::LocalNetwork);
        assert_eq!(kind("10.0.0.2:1"), ConnectionKind::LocalNetwork);
        assert_eq!(kind("[fd00::1]:1"), ConnectionKind::LocalNetwork);
        assert_eq!(kind("[2001:db8::1]:1"), ConnectionKind::Ipv6);
        assert_eq!(kind("203.0.113.5:1"), ConnectionKind::Internet);
    }
}
