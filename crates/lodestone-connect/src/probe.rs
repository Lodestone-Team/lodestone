//! Finding a path: both sides send small tagged probes to every candidate address of
//! the other. The host is already a QUIC server on its socket, so it only sends probes
//! ([`spray`]), which open its router towards the friend. The friend probes the host's
//! addresses ([`until_heard`]) and stops at the first probe from the host: packets get
//! through on that path, and the friend opens QUIC on it.
//!
//! A probe is 13 bytes: `LHP2`, an 8-byte tag derived from the invite's token (so it
//! says nothing about the token), and a kind byte (0 probe, 1 answer).

use std::{
    net::{SocketAddr, UdpSocket},
    time::{Duration, Instant},
};

use sha2::{Digest as _, Sha256};

use crate::addr::{canonical, to_socket};

pub const PROBE: u8 = 0;
pub const ACK: u8 = 1;
const MAGIC: &[u8; 4] = b"LHP2";
/// How often probes go out.
pub const EVERY: Duration = Duration::from_millis(300);

/// The probes' tag for an invite's token.
pub fn tag(token: &[u8]) -> [u8; 8] {
    let digest = Sha256::new()
        .chain_update(b"lodestone-connect probe tag\0")
        .chain_update(token)
        .finalize();
    digest[..8].try_into().expect("32 bytes")
}

/// `LHP2` + the tag + the kind.
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

/// The friend's side: probes `peers` (which opens this side's router towards the host)
/// and stops at the first probe or answer from the host. Returns the address it came
/// from, or `Err(heard_anything)` when `window` ends first. Blocking; the socket must
/// not belong to QUIC yet.
pub fn until_heard(
    socket: &UdpSocket,
    peers: &[SocketAddr],
    tag: [u8; 8],
    window: Duration,
) -> Result<SocketAddr, bool> {
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|_| false)?;
    let until = Instant::now() + window;
    let mut next_send = Instant::now();
    let mut buf = [0u8; 64];
    let mut found = Err(false);
    while Instant::now() < until {
        if Instant::now() >= next_send {
            for peer in peers {
                let _ = socket.send_to(&message(tag, PROBE), to_socket(*peer));
            }
            next_send = Instant::now() + EVERY;
        }
        if let Ok((len, from)) = socket.recv_from(&mut buf) {
            if read(&buf[..len], tag).is_some() {
                let from = canonical(from);
                tracing::debug!("heard the host at {from}");
                found = Ok(from);
                break;
            }
        }
    }
    let _ = socket.set_read_timeout(None);
    found
}

#[cfg(feature = "host")]
/// The host's side: sends probes to `peers` for `how_long`, or until `stop` says so,
/// from a clone of the QUIC socket. This opens the router towards the friend and tells
/// the friend which address reaches the host.
pub async fn spray(
    socket: UdpSocket,
    peers: Vec<SocketAddr>,
    tag: [u8; 8],
    how_long: Duration,
    mut stop: tokio::sync::watch::Receiver<bool>,
) {
    let until = tokio::time::Instant::now() + how_long;
    while tokio::time::Instant::now() < until && !*stop.borrow() {
        for peer in &peers {
            // The socket is non-blocking (QUIC set it so): a full buffer just drops one.
            let _ = socket.send_to(&message(tag, PROBE), to_socket(*peer));
        }
        tokio::select! {
            _ = tokio::time::sleep(EVERY) => {}
            _ = stop.changed() => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_this_invites_messages_count() {
        let tag = tag(&[9u8; 16]);
        assert_eq!(read(&message(tag, ACK), tag), Some(ACK));
        assert_eq!(read(&message([1u8; 8], ACK), tag), None);
        assert_eq!(read(b"lodestone-punch", tag), None);
        // the tag gives nothing of the token away
        assert_ne!(&tag[..], &[9u8; 8][..]);
    }

    #[test]
    fn a_friend_hears_the_host() {
        let host = UdpSocket::bind("127.0.0.1:0").unwrap();
        let friend = UdpSocket::bind("127.0.0.1:0").unwrap();
        let tag = tag(&[1u8; 16]);
        host.send_to(&message(tag, PROBE), friend.local_addr().unwrap())
            .unwrap();
        let heard = until_heard(
            &friend,
            &[host.local_addr().unwrap()],
            tag,
            Duration::from_secs(2),
        );
        assert_eq!(heard, Ok(host.local_addr().unwrap()));
    }
}
