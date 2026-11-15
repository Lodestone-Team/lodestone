use std::{
    collections::HashSet,
    net::{IpAddr, SocketAddr, SocketAddrV4},
};

use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};

use crate::error::{Error, ErrorKind};
use crate::util::local_port_available;

pub struct PortManager {
    allocated_ports: HashSet<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct PortStatus {
    pub is_in_use: bool,
    pub is_allocated: bool,
}

impl PortManager {
    pub fn new(allocated_ports: HashSet<u32>) -> PortManager {
        PortManager { allocated_ports }
    }

    /// A port above 65535 cannot be bound, so it is reported as in use instead of being
    /// truncated to a different (often free) port.
    pub fn port_status(&self, port: u32) -> PortStatus {
        PortStatus {
            is_in_use: u16::try_from(port).map_or(true, |port| !local_port_available(port)),
            is_allocated: self.allocated_ports.contains(&port),
        }
    }

    pub fn add_port(&mut self, port: u32) {
        self.allocated_ports.insert(port);
    }

    pub fn deallocate(&mut self, port: u32) {
        self.allocated_ports.remove(&port);
    }

    /// The first port from `from` up that no instance has and nothing listens on, for a
    /// new server that didn't choose one (a backup restored as a new server). `None` when
    /// every port up to 65535 is taken.
    pub fn next_free(&self, from: u32) -> Option<u32> {
        (from..=u16::MAX as u32).find(|port| {
            let status = self.port_status(*port);
            !status.is_allocated && !status.is_in_use
        })
    }

    /// Forwards TCP `port` on the local network's UPnP gateway to this machine.
    ///
    /// The gateway search can take several seconds, so callers should not hold the
    /// port manager lock while awaiting this.
    pub async fn open_upnp_port(port: u16) -> Result<(), Error> {
        tokio::task::spawn_blocking(move || {
            let local_ip = local_ip_address::local_ip().map_err(|e| Error {
                kind: ErrorKind::Internal,
                source: eyre!("Could not find local ip address: {e}"),
            })?;
            let local_addr = upnp_local_addr(local_ip, port)?;

            igd_next::search_gateway(Default::default())
                .context("Could not find gateway")?
                .add_port(
                    igd_next::PortMappingProtocol::TCP,
                    port,
                    local_addr,
                    0,
                    "Port opened by Lodestone",
                )
                .context("Could not open port")?;
            Ok(())
        })
        .await
        .context("Port forwarding task failed")?
    }
}

/// UPnP IGD port mappings are IPv4 only.
fn upnp_local_addr(local_ip: IpAddr, port: u16) -> Result<SocketAddr, Error> {
    match local_ip {
        IpAddr::V4(ipv4) => Ok(SocketAddr::V4(SocketAddrV4::new(ipv4, port))),
        IpAddr::V6(ipv6) => Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!(
                "UPnP port forwarding needs an IPv4 address, but this machine's address is {ipv6}"
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_of_range_port_is_in_use() {
        let manager = PortManager::new(HashSet::from([70000]));
        // 70000 as u16 is 4464, which used to be checked instead
        let status = manager.port_status(70000);
        assert!(status.is_in_use);
        assert!(status.is_allocated);
    }

    #[test]
    fn next_free_skips_allocated_and_used_ports() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let used = listener.local_addr().unwrap().port() as u32;
        let manager = PortManager::new(HashSet::from([used + 1]));
        let free = manager.next_free(used).unwrap();
        assert!(free > used + 1, "{free} {used}");
        assert_eq!(PortManager::new(HashSet::new()).next_free(70000), None);
    }

    #[test]
    fn ipv6_is_an_error_not_a_panic() {
        assert!(upnp_local_addr("::1".parse().unwrap(), 25565).is_err());
        assert_eq!(
            upnp_local_addr("192.168.1.2".parse().unwrap(), 25565).unwrap(),
            "192.168.1.2:25565".parse().unwrap()
        );
    }
}
