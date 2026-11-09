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
