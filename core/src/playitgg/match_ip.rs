use std::net::{Ipv6Addr, Ipv4Addr, IpAddr};

#[derive(Debug)]
pub struct MatchIp {
    pub ip_number: u64,
    pub region_id: Option<u16>,
}

impl MatchIp {
    pub fn new(ip: Ipv6Addr) -> Self {
        let parts = ip.octets();

        /* 6 bytes /48 BGP Routing */
        /* 2 bytes for region id */
        let region_id = u16::from_be_bytes([parts[6], parts[7]]);

        /* 8 bytes for ip number */
        let ip_number = u64::from_be_bytes([
            parts[8],
            parts[9],
            parts[10],
            parts[11],
            parts[12],
            parts[13],
            parts[14],
            parts[15],
        ]);

        MatchIp {
            ip_number,
            region_id: if region_id == 0 {
                None
            } else {
                Some(region_id)
            },
        }
    }

    fn region_number_v4(ip: Ipv4Addr) -> u16 {
        let octs = ip.octets();

        /* 147.185.221.0/24 (1) */
        if octs[0] == 147 && octs[1] == 185 && octs[2] == 221 {
            1u16
        }
        /* 209.25.140.0/22 (2 to 5) */
        else if octs[0] == 209 && octs[1] == 25 && octs[2] >= 140 && octs[2] <= 143 {
            2u16 + (octs[2] - 140) as u16
        }
        /* 23.133.216.0/24 (6) */
        else if octs[0] == 23 && octs[1] == 133 && octs[2] == 216 {
            6u16
        }
        /* global IP */
        else {
            0
        }
    }

    pub fn matches(&self, ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(ip) => {
                let octs = ip.octets();

                if octs[3] as u64 != self.ip_number {
                    return false;
                }

                self.region_id.map(|v| v == Self::region_number_v4(ip)).unwrap_or(true)
            }
            IpAddr::V6(ip) => {
                let other = MatchIp::new(ip);
                self.ip_number == other.ip_number && self.region_id == other.region_id
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_match_ip() {
        let ip = MatchIp::new("2602:fbaf:0:2::10".parse().unwrap());
        // assert!(ip.matches("2602:fbaf:0:2::10".parse().unwrap()));
        // assert!(ip.matches("2602:fbaf:808:2::10".parse().unwrap()));
        // assert!(!ip.matches("2602:fbaf:808:3::10".parse().unwrap()));
        // assert!(ip.matches("209.25.140.16".parse().unwrap()));
        // assert!(!ip.matches("209.25.141.16".parse().unwrap()));
        assert!(!ip.matches("147.185.221.16".parse().unwrap()));

        let ip = MatchIp::new("2602:fbaf:0:1::10".parse().unwrap());
        assert!(ip.matches("147.185.221.16".parse().unwrap()));
        assert!(!ip.matches("209.25.140.16".parse().unwrap()));
    }
}
