//! The smallest STUN client (RFC 8489): one Binding request, and the
//! XOR-MAPPED-ADDRESS from the answer. Blocking: run it before QUIC takes the socket.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs, UdpSocket};

use crate::addr::{canonical, to_socket};

const COOKIE: u32 = 0x2112_A442;

/// The public STUN servers asked over IPv4. Two, to notice a router that picks a new
/// port per destination.
pub const STUN_V4: [&str; 2] = ["stun.l.google.com:19302", "stun.cloudflare.com:3478"];
/// The STUN server asked over IPv6.
pub const STUN_V6: &str = "stun.l.google.com:19302";

/// `server`'s IPv4 or IPv6 (`v6`) address. Blocking (DNS).
pub(crate) fn resolve(server: &str, v6: bool) -> Result<SocketAddr, String> {
    server
        .to_socket_addrs()
        .map_err(|err| err.to_string())?
        .find(|addr| addr.is_ipv6() == v6)
        .ok_or_else(|| {
            if v6 {
                "no IPv6 address".into()
            } else {
                "no IPv4 address".into()
            }
        })
}

/// A Binding request with transaction id `txid`.
pub(crate) fn request(txid: &[u8; 12]) -> [u8; 20] {
    let mut request = [0u8; 20];
    request[..2].copy_from_slice(&0x0001u16.to_be_bytes()); // Binding request
                                                            // bytes 2..4: no attributes
    request[4..8].copy_from_slice(&COOKIE.to_be_bytes());
    request[8..].copy_from_slice(txid);
    request
}

/// The transaction id of what looks like a Binding success response.
#[cfg(feature = "host")]
pub(crate) fn response_txid(msg: &[u8]) -> Option<[u8; 12]> {
    (msg.len() >= 20
        && u16::from_be_bytes([msg[0], msg[1]]) == 0x0101
        && msg[4..8] == COOKIE.to_be_bytes())
    .then(|| msg[8..20].try_into().expect("12 bytes"))
}

/// What `server` sees as this socket's address, over IPv4 or IPv6 (`v6`). The socket
/// needs a read timeout.
pub fn mapped_address(socket: &UdpSocket, server: &str, v6: bool) -> Result<SocketAddr, String> {
    let server = resolve(server, v6)?;
    let txid: [u8; 12] = crate::random().map_err(|err| err.to_string())?;
    let request = request(&txid);
    let mut buf = [0u8; 512];
    for _ in 0..3 {
        socket
            .send_to(&request, to_socket(server))
            .map_err(|err| err.to_string())?;
        let Ok((len, from)) = socket.recv_from(&mut buf) else {
            continue;
        };
        if canonical(from) != server || len < 20 || buf[8..20] != txid {
            continue;
        }
        if let Some(addr) = parse(&buf[..len], &txid) {
            return Ok(addr);
        }
    }
    Err("no answer".into())
}

/// The mapped address in a Binding success response.
pub(crate) fn parse(msg: &[u8], txid: &[u8; 12]) -> Option<SocketAddr> {
    if msg.len() < 20 || u16::from_be_bytes([msg[0], msg[1]]) != 0x0101 {
        return None; // not a Binding success response
    }
    let mut at = 20;
    while at + 4 <= msg.len() {
        let kind = u16::from_be_bytes([msg[at], msg[at + 1]]);
        let len = u16::from_be_bytes([msg[at + 2], msg[at + 3]]) as usize;
        let value = msg.get(at + 4..at + 4 + len)?;
        if (kind == 0x0020 || kind == 0x0001) && value.len() >= 8 {
            let xor = kind == 0x0020;
            let port = u16::from_be_bytes([value[2], value[3]])
                ^ if xor { (COOKIE >> 16) as u16 } else { 0 };
            let ip = match value[1] {
                0x01 => {
                    let raw = u32::from_be_bytes([value[4], value[5], value[6], value[7]]);
                    IpAddr::V4(Ipv4Addr::from(if xor { raw ^ COOKIE } else { raw }))
                }
                0x02 if value.len() >= 20 => {
                    let mut raw = [0u8; 16];
                    raw.copy_from_slice(&value[4..20]);
                    if xor {
                        let mut key = [0u8; 16];
                        key[..4].copy_from_slice(&COOKIE.to_be_bytes());
                        key[4..].copy_from_slice(txid);
                        for (byte, k) in raw.iter_mut().zip(key) {
                            *byte ^= k;
                        }
                    }
                    IpAddr::V6(Ipv6Addr::from(raw))
                }
                _ => return None,
            };
            return Some(SocketAddr::new(ip, port));
        }
        at += 4 + len.div_ceil(4) * 4;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_an_xor_mapped_ipv4_address() {
        let txid = [7u8; 12];
        let port = 54321u16 ^ (COOKIE >> 16) as u16;
        let ip = u32::from(Ipv4Addr::new(203, 0, 113, 5)) ^ COOKIE;
        let mut msg = vec![0x01, 0x01, 0, 12];
        msg.extend_from_slice(&COOKIE.to_be_bytes());
        msg.extend_from_slice(&txid);
        msg.extend_from_slice(&[0x00, 0x20, 0, 8, 0, 0x01]);
        msg.extend_from_slice(&port.to_be_bytes());
        msg.extend_from_slice(&ip.to_be_bytes());
        assert_eq!(
            parse(&msg, &txid),
            Some("203.0.113.5:54321".parse().unwrap())
        );
        assert_eq!(parse(&msg[..10], &txid), None);
        #[cfg(feature = "host")]
        {
            assert_eq!(response_txid(&msg), Some(txid));
            assert_eq!(response_txid(&request(&txid)), None);
        }
    }
}
