use std::{collections::HashSet, net::SocketAddrV4};

use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};

use crate::error::Error;

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

    pub fn allocate(&mut self, start_port: u32) -> u32 {
        if self.allocated_ports.contains(&start_port) {
            let mut new_port = start_port + 1;
            while self.allocated_ports.contains(&new_port)
                || !port_scanner::local_port_available(new_port as u16)
            {
                new_port += 1;
            }
            self.allocated_ports.insert(new_port);
            new_port
        } else {
            self.allocated_ports.insert(start_port);
            start_port
        }
    }

    pub fn port_status(&self, port: u32) -> PortStatus {
        PortStatus {
            is_in_use: !port_scanner::local_port_available(port as u16),
            is_allocated: self.allocated_ports.contains(&port),
        }
    }

    pub fn add_port(&mut self, port: u32) {
        self.allocated_ports.insert(port);
    }

    pub fn deallocate(&mut self, port: u32) {
        self.allocated_ports.remove(&port);
    }

    pub async fn open_port(&self, port: u16) -> Result<(), Error> {
        tokio::task::spawn_blocking(move || {
            if let Ok(local_ip) = local_ip_address::local_ip() {
                // convert local_ip to a SocketAddrV4
                let local_ip = if let std::net::IpAddr::V4(ipv4) = local_ip {
                    SocketAddrV4::new(ipv4, port)
                } else {
                    panic!();
                };

                igd::search_gateway(Default::default())
                    .context("Could not find gateway")?
                    .add_port(
                        igd::PortMappingProtocol::TCP,
                        port,
                        local_ip,
                        0,
                        "Port opened by Lodestone",
                    )
                    .context("Could not open port")?;
                Ok(())
            } else {
                Err(eyre!("Could not find local ip address").into())
            }
        })
        .await
        .unwrap()
    }
}
