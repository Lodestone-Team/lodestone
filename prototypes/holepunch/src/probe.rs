//! Finding a path: both sides send small tagged probes to every candidate address of
//! the other, and answer the probes they get. Each side keeps going until the other
//! answers it (an ACK), which proves packets get through both ways on that path.
//!
//! A probe from an address that wasn't among the candidates (a router that picks a new
//! port per destination) is answered and added to the addresses probed, as ICE's
//! "peer-reflexive" candidates.

use std::{
    net::{SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

use crate::{canonical, to_socket};

pub const PROBE: u8 = 0;
pub const ACK: u8 = 1;
const MAGIC: &[u8; 4] = b"LHP2";
const EVERY: Duration = Duration::from_millis(300);

/// `LHP2` + the session's tag + the kind.
pub fn message(tag: [u8; 8], kind: u8) -> [u8; 13] {
    let mut msg = [0u8; 13];
    msg[..4].copy_from_slice(MAGIC);
    msg[4..12].copy_from_slice(&tag);
    msg[12] = kind;
    msg
}

fn read(msg: &[u8], tag: [u8; 8]) -> Option<u8> {
    (msg.len() == 13 && &msg[..4] == MAGIC && msg[4..12] == tag).then(|| msg[12])
}

/// Probes `peers` until one answers or `window` ends; returns the address that answered.
pub fn run(
    socket: &UdpSocket,
    peers: &[SocketAddr],
    tag: [u8; 8],
    window: Duration,
) -> Option<SocketAddr> {
    let mut peers: Vec<SocketAddr> = peers.to_vec();
    let mut heard: Vec<SocketAddr> = Vec::new();
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .ok()?;
    let until = Instant::now() + window;
    let mut next_send = Instant::now();
    let mut buf = [0u8; 64];
    let mut found = None;
    while Instant::now() < until {
        if Instant::now() >= next_send {
            for peer in &peers {
                let _ = socket.send_to(&message(tag, PROBE), to_socket(*peer));
            }
            next_send = Instant::now() + EVERY;
        }
        let Ok((len, from)) = socket.recv_from(&mut buf) else {
            continue;
        };
        let from = canonical(from);
        match read(&buf[..len], tag) {
            Some(PROBE) => {
                if !heard.contains(&from) {
                    println!("  heard a probe from {from}");
                    heard.push(from);
                }
                if !peers.contains(&from) {
                    peers.push(from);
                }
                let _ = socket.send_to(&message(tag, ACK), to_socket(from));
            }
            Some(ACK) => {
                println!("  {from} answered our probe");
                found = Some(from);
                break;
            }
            _ => {}
        }
    }
    // Keep answering for a moment, so the other side gets its ACK too before QUIC takes
    // the socket.
    if let Some(path) = found {
        let linger = Instant::now() + Duration::from_secs(2);
        while Instant::now() < linger {
            let _ = socket.send_to(&message(tag, ACK), to_socket(path));
            if let Ok((len, from)) = socket.recv_from(&mut buf)
                && read(&buf[..len], tag) == Some(PROBE)
            {
                let _ = socket.send_to(&message(tag, ACK), from);
            }
        }
    } else if heard.is_empty() {
        println!("  nothing arrived from the other side");
    } else {
        println!("  their probes arrived, but ours never reached them");
    }
    socket.set_read_timeout(None).ok()?;
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_this_sessions_messages_count() {
        let tag = [9u8; 8];
        assert_eq!(read(&message(tag, ACK), tag), Some(ACK));
        assert_eq!(read(&message([1u8; 8], ACK), tag), None);
        assert_eq!(read(b"lodestone-punch", tag), None);
    }
}
