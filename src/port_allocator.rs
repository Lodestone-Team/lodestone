use std::collections::HashSet;

use serde::{Serialize, Deserialize};

pub struct PortAllocator {
    allocated_ports: HashSet<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct PortStatus {
    pub is_in_use: bool,
    pub is_allocated: bool,
}

impl PortAllocator {
    pub fn new(allocated_ports: HashSet<u32>) -> PortAllocator {
        PortAllocator { allocated_ports }
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
}
