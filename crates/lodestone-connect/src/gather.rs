//! Collecting the addresses the other side can try: the local network address, IPv6, the
//! public address STUN servers see, and a port the router forwards on request.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::Duration,
};

use serde::{Deserialize, Serialize};

use crate::{addr, portmap, stun, Error, Result};

/// The introduction service takes at most this many.
pub const MAX_CANDIDATES: usize = 8;

/// One address the other side can try.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub addr: SocketAddr,
    pub kind: CandidateKind,
}

/// Where a candidate address came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateKind {
    /// 127.0.0.1, for tests on one computer.
    Loopback,
    /// The address on the local network.
    LocalNetwork,
    /// The IPv6 address.
    Ipv6,
    /// The public address STUN servers see.
    Public,
    /// A port the router forwards (UPnP or NAT-PMP).
    RouterForwarded,
}

/// What to collect.
#[derive(Clone, Copy, Debug)]
pub struct GatherOptions {
    /// Ask the router to forward a port (UPnP, NAT-PMP).
    pub portmap: bool,
    /// Offer only 127.0.0.1: both sides on this computer (tests). No STUN, no router.
    pub loopback_only: bool,
}

impl Default for GatherOptions {
    fn default() -> Self {
        Self {
            portmap: true,
            loopback_only: false,
        }
    }
}

/// This side's socket, its candidates, and the router mapping to remove later.
#[derive(Debug)]
pub struct Gathered {
    pub socket: UdpSocket,
    pub candidates: Vec<Candidate>,
    pub mapping: Option<portmap::Mapping>,
    /// The STUN servers saw different ports: this router picks a new port per
    /// destination ("symmetric NAT"), which punching rarely gets through.
    pub symmetric: bool,
}

impl Gathered {
    pub fn addrs(&self) -> Vec<SocketAddr> {
        self.candidates.iter().map(|c| c.addr).collect()
    }
}

/// Opens the socket and collects its candidates. Blocking (STUN and the router take a
/// few seconds): call it from `spawn_blocking`.
pub fn gather(options: GatherOptions) -> Result<Gathered> {
    let socket = addr::dual_stack_socket()?;
    let port = socket.local_addr()?.port();
    if options.loopback_only {
        return Ok(Gathered {
            socket,
            candidates: vec![Candidate {
                addr: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port),
                kind: CandidateKind::Loopback,
            }],
            mapping: None,
            symmetric: false,
        });
    }
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut add = |addr: SocketAddr, kind: CandidateKind| {
        if candidates.len() < MAX_CANDIDATES && !candidates.iter().any(|c| c.addr == addr) {
            tracing::debug!("candidate ({kind:?}): {addr}");
            candidates.push(Candidate { addr, kind });
        }
    };

    let lan_v4 = addr::route_to(
        "0.0.0.0:0".parse().expect("valid"),
        "8.8.8.8:53".parse().expect("valid"),
    );
    if let Some(ip) = lan_v4 {
        add(SocketAddr::new(ip, port), CandidateKind::LocalNetwork);
    }
    let v6 = addr::route_to(
        "[::]:0".parse().expect("valid"),
        "[2001:4860:4860::8888]:53".parse().expect("valid"),
    )
    .filter(|ip| match ip {
        IpAddr::V6(v6) => !v6.is_loopback() && (v6.segments()[0] & 0xffc0) != 0xfe80,
        IpAddr::V4(_) => false,
    });
    if let Some(ip) = v6 {
        add(SocketAddr::new(ip, port), CandidateKind::Ipv6);
    }

    socket.set_read_timeout(Some(Duration::from_secs(2)))?;
    let mut seen = Vec::new();
    for server in stun::STUN_V4 {
        match stun::mapped_address(&socket, server, false) {
            Ok(addr) => seen.push(addr),
            Err(err) => tracing::debug!("STUN {server}: {err}"),
        }
    }
    if v6.is_some() {
        if let Ok(addr) = stun::mapped_address(&socket, stun::STUN_V6, true) {
            add(addr, CandidateKind::Ipv6);
        }
    }
    socket.set_read_timeout(None)?;
    let symmetric = seen.windows(2).any(|pair| pair[0].port() != pair[1].port());
    if let Some(first) = seen.first().copied() {
        add(first, CandidateKind::Public);
    }

    let mut mapping = None;
    if let (true, Some(IpAddr::V4(local_ip))) = (options.portmap, lan_v4) {
        match portmap::map(port, local_ip) {
            Ok(found) => {
                let behind_another_nat = match found.external.ip() {
                    IpAddr::V4(ip) => portmap::is_private(ip),
                    IpAddr::V6(_) => true,
                };
                if behind_another_nat {
                    // The router is behind another router or the ISP's NAT (CGNAT):
                    // its forwarding doesn't help.
                    found.remove();
                } else {
                    add(found.external, CandidateKind::RouterForwarded);
                    mapping = Some(found);
                }
            }
            Err(err) => tracing::debug!("router forwarding: not available ({err})"),
        }
    }
    if candidates.is_empty() {
        return Err(Error::NoAddress);
    }
    Ok(Gathered {
        socket,
        candidates,
        mapping,
        symmetric,
    })
}

/// [`gather`] on a blocking thread.
pub async fn gather_async(options: GatherOptions) -> Result<Gathered> {
    tokio::task::spawn_blocking(move || gather(options))
        .await
        .map_err(|err| Error::Other(format!("Collecting addresses failed: {err}")))?
}
