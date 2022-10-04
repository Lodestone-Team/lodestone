use std::collections::HashSet;

pub struct PortAllocator {
    allocated_ports: HashSet<u32>,
}

impl PortAllocator {
    pub fn new(allocated_ports : HashSet<u32>) -> PortAllocator {
        PortAllocator {
            allocated_ports,
        }
    }

    pub fn allocate(&mut self, start_port: u32) -> u32 {
        if self.allocated_ports.contains(&start_port) {
            let mut new_port = start_port + 1;
            while self.allocated_ports.contains(&new_port)
                && port_scanner::local_port_available(new_port as u16)
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

    pub fn is_port_in_use(&self, port: u32) -> bool {
        self.allocated_ports.contains(&port) || !port_scanner::local_port_available(port as u16)
    }

    pub fn add_port(&mut self, port: u32) {
        self.allocated_ports.insert(port);
    }

    pub fn deallocate(&mut self, port: u32) {
        self.allocated_ports.remove(&port);
    }
}
