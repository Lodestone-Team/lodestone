//! The friend host's socket, gated: QUIC gets only the datagrams from addresses that may
//! reach the core (in an accepted knock, or on a connection now). quinn answers some
//! datagrams before the host can decide anything: a version negotiation for an Initial
//! with a version it doesn't know, a stateless reset for a packet of no connection. A
//! datagram from anyone else never reaches it, so the core says nothing at all to them.
//!
//! The gate also takes the answers to STUN requests sent from the socket
//! ([`Gate::mapped_address`]), so the host can learn its public address again while QUIC
//! owns the socket.

use std::{
    collections::HashMap,
    fmt,
    io::{self, IoSliceMut},
    net::{IpAddr, SocketAddr, UdpSocket},
    pin::Pin,
    sync::{mpsc, Arc, Mutex},
    task::{Context, Poll},
    time::Duration,
};

use quinn::{
    udp::{RecvMeta, Transmit},
    AsyncUdpSocket, Endpoint, EndpointConfig, Runtime as _, TokioRuntime, UdpPoller,
};

use crate::{
    addr::{canonical, to_socket},
    stun, Result,
};

/// Says whether a datagram from this address may reach QUIC.
pub(crate) type Admit = Arc<dyn Fn(IpAddr) -> bool + Send + Sync>;

/// How many batches of dropped datagrams a read goes through before it lets other tasks
/// run, so a flood of them can't hold the thread.
const DROPPED_BATCHES_PER_POLL: usize = 32;

/// How long a STUN request waits for its answer before it's sent again.
const STUN_WAIT: Duration = Duration::from_secs(2);

/// A STUN request waiting for its answer: the server asked, and where the answer goes.
type StunWait = (SocketAddr, mpsc::Sender<SocketAddr>);

/// A UDP socket that passes QUIC only the datagrams `admit` lets in.
pub(crate) struct Gate {
    inner: Arc<dyn AsyncUdpSocket>,
    admit: Admit,
    /// STUN requests sent from this socket, by transaction id.
    stun: Mutex<HashMap<[u8; 12], StunWait>>,
}

impl fmt::Debug for Gate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Gate").field("inner", &self.inner).finish()
    }
}

impl Gate {
    /// A QUIC server endpoint on `socket` that hears only what `admit` lets in, and the
    /// gate (for [`Gate::mapped_address`]).
    pub(crate) fn endpoint(
        socket: UdpSocket,
        server: quinn::ServerConfig,
        admit: Admit,
    ) -> Result<(Endpoint, Arc<Gate>)> {
        socket.set_nonblocking(true)?;
        let runtime = Arc::new(TokioRuntime);
        let gate = Arc::new(Gate {
            inner: runtime.wrap_udp_socket(socket)?,
            admit,
            stun: Mutex::new(HashMap::new()),
        });
        let endpoint = Endpoint::new_with_abstract_socket(
            EndpointConfig::default(),
            Some(server),
            gate.clone(),
            runtime,
        )?;
        Ok((endpoint, gate))
    }

    fn admits(&self, from: SocketAddr) -> bool {
        (self.admit)(canonical(from).ip())
    }

    /// Takes `data` if it answers one of this socket's STUN requests.
    fn take_stun_answer(&self, from: SocketAddr, data: &[u8]) -> bool {
        let Some(txid) = stun::response_txid(data) else {
            return false;
        };
        let mut waiting = self.stun.lock().expect("not poisoned");
        let Some((server, _)) = waiting.get(&txid) else {
            return false;
        };
        if *server != canonical(from) {
            return false;
        }
        if let (Some(addr), Some((_, answer))) = (stun::parse(data, &txid), waiting.remove(&txid)) {
            let _ = answer.send(addr);
        }
        true
    }

    /// What STUN `server` sees as this socket's address, over IPv4 or IPv6 (`v6`), while
    /// QUIC owns the socket: the request goes out through `socket` (a clone of it), and
    /// the gate takes the answer. Blocking.
    pub(crate) fn mapped_address(
        &self,
        socket: &UdpSocket,
        server: &str,
        v6: bool,
    ) -> std::result::Result<SocketAddr, String> {
        let server = stun::resolve(server, v6)?;
        let txid: [u8; 12] = crate::random().map_err(|err| err.to_string())?;
        let (send, answer) = mpsc::channel();
        self.stun
            .lock()
            .expect("not poisoned")
            .insert(txid, (server, send));
        let mut result = Err("no answer".to_string());
        for _ in 0..3 {
            // The socket doesn't block (QUIC set it so): a full buffer drops one try.
            let _ = socket.send_to(&stun::request(&txid), to_socket(server));
            if let Ok(addr) = answer.recv_timeout(STUN_WAIT) {
                result = Ok(addr);
                break;
            }
        }
        self.stun.lock().expect("not poisoned").remove(&txid);
        result
    }
}

/// Keeps the datagrams `keep` says yes to at the front of `bufs` and `meta`, in order.
/// Returns how many there are.
fn keep_only(
    bufs: &mut [IoSliceMut<'_>],
    meta: &mut [RecvMeta],
    count: usize,
    mut keep: impl FnMut(&RecvMeta, &[u8]) -> bool,
) -> usize {
    let mut kept = 0;
    for i in 0..count {
        if !keep(&meta[i], &bufs[i][..meta[i].len]) {
            continue;
        }
        if kept != i {
            let len = meta[i].len;
            let (front, back) = bufs.split_at_mut(i);
            front[kept][..len].copy_from_slice(&back[0][..len]);
            meta[kept] = meta[i];
        }
        kept += 1;
    }
    kept
}

impl AsyncUdpSocket for Gate {
    fn create_io_poller(self: Arc<Self>) -> Pin<Box<dyn UdpPoller>> {
        self.inner.clone().create_io_poller()
    }

    fn try_send(&self, transmit: &Transmit) -> io::Result<()> {
        self.inner.try_send(transmit)
    }

    fn poll_recv(
        &self,
        cx: &mut Context,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> Poll<io::Result<usize>> {
        for _ in 0..DROPPED_BATCHES_PER_POLL {
            let count = match self.inner.poll_recv(cx, bufs, meta) {
                Poll::Ready(Ok(count)) => count,
                other => return other,
            };
            let kept = keep_only(bufs, meta, count, |meta, data| {
                let first = &data[..meta.stride.min(data.len())];
                !self.take_stun_answer(meta.addr, first) && self.admits(meta.addr)
            });
            if kept > 0 {
                return Poll::Ready(Ok(kept));
            }
            // Everything was dropped: read on. When nothing is left, the socket says
            // Pending and wakes this task for the next datagram.
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    fn max_transmit_segments(&self) -> usize {
        self.inner.max_transmit_segments()
    }

    fn max_receive_segments(&self) -> usize {
        self.inner.max_receive_segments()
    }

    fn may_fragment(&self) -> bool {
        self.inner.may_fragment()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A STUN server on this computer that says every client is at `seen`.
    fn fake_stun(seen: SocketAddr) -> SocketAddr {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let addr = server.local_addr().unwrap();
        std::thread::spawn(move || {
            let mut buf = [0u8; 64];
            while let Ok((len, from)) = server.recv_from(&mut buf) {
                if len < 20 {
                    continue;
                }
                let SocketAddr::V4(seen) = seen else {
                    unreachable!()
                };
                let cookie = 0x2112_A442u32;
                let mut answer = vec![0x01, 0x01, 0, 12];
                answer.extend_from_slice(&cookie.to_be_bytes());
                answer.extend_from_slice(&buf[8..20]);
                answer.extend_from_slice(&[0x00, 0x20, 0, 8, 0, 0x01]);
                answer.extend_from_slice(&(seen.port() ^ (cookie >> 16) as u16).to_be_bytes());
                answer.extend_from_slice(&(u32::from(*seen.ip()) ^ cookie).to_be_bytes());
                let _ = server.send_to(&answer, from);
            }
        });
        addr
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stun_answers_reach_the_host_while_quic_owns_the_socket() {
        let made = crate::cert::self_signed("x").unwrap();
        let server = crate::tunnel::server_config_for(
            &made.cert_der,
            &made.key_der,
            crate::tunnel::FRIEND_ALPN,
        )
        .unwrap();
        let socket = crate::addr::dual_stack_socket().unwrap();
        let sender = socket.try_clone().unwrap();
        // nobody may reach QUIC: the STUN server's answer still gets through to the host
        let (endpoint, gate) = Gate::endpoint(socket, server, Arc::new(|_| false)).unwrap();
        let seen: SocketAddr = "203.0.113.5:54321".parse().unwrap();
        let stun_server = fake_stun(seen).to_string();
        let found =
            tokio::task::spawn_blocking(move || gate.mapped_address(&sender, &stun_server, false))
                .await
                .unwrap();
        assert_eq!(found, Ok(seen));
        endpoint.close(0u32.into(), b"");
    }

    #[test]
    fn only_admitted_datagrams_are_kept_in_order() {
        let mut storage = [[0u8; 4]; 4];
        let addrs: [SocketAddr; 4] = [
            "198.18.0.1:1".parse().unwrap(),
            "198.18.0.2:1".parse().unwrap(),
            "198.18.0.3:1".parse().unwrap(),
            "198.18.0.4:1".parse().unwrap(),
        ];
        let mut meta = [RecvMeta::default(); 4];
        for (n, buf) in storage.iter_mut().enumerate() {
            *buf = [n as u8 + 1; 4];
            meta[n].addr = addrs[n];
            meta[n].len = 3;
            meta[n].stride = 3;
        }
        let mut bufs: Vec<IoSliceMut> = storage.iter_mut().map(|b| IoSliceMut::new(b)).collect();
        let kept = keep_only(&mut bufs, &mut meta, 4, |meta, data| {
            assert_eq!(data[0], data[2]);
            meta.addr == addrs[1] || meta.addr == addrs[3]
        });
        assert_eq!(kept, 2);
        assert_eq!(meta[0].addr, addrs[1]);
        assert_eq!(meta[1].addr, addrs[3]);
        assert_eq!(&bufs[0][..3], &[2, 2, 2]);
        assert_eq!(&bufs[1][..3], &[4, 4, 4]);
    }
}
