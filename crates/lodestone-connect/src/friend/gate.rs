//! The friend host's socket, gated: QUIC gets only the datagrams from addresses that may
//! reach the core (in an accepted knock, or on a connection now). quinn answers some
//! datagrams before the host can decide anything: a version negotiation for an Initial
//! with a version it doesn't know, a stateless reset for a packet of no connection. A
//! datagram from anyone else never reaches it, so the core says nothing at all to them.

use std::{
    fmt,
    io::{self, IoSliceMut},
    net::{IpAddr, SocketAddr, UdpSocket},
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use quinn::{
    udp::{RecvMeta, Transmit},
    AsyncUdpSocket, Endpoint, EndpointConfig, Runtime as _, TokioRuntime, UdpPoller,
};

use crate::{addr::canonical, Result};

/// Says whether a datagram from this address may reach QUIC.
pub(crate) type Admit = Arc<dyn Fn(IpAddr) -> bool + Send + Sync>;

/// How many batches of dropped datagrams a read goes through before it lets other tasks
/// run, so a flood of them can't hold the thread.
const DROPPED_BATCHES_PER_POLL: usize = 32;

/// A UDP socket that passes QUIC only the datagrams `admit` lets in.
pub(crate) struct Gate {
    inner: Arc<dyn AsyncUdpSocket>,
    admit: Admit,
}

impl fmt::Debug for Gate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Gate").field("inner", &self.inner).finish()
    }
}

impl Gate {
    /// A QUIC server endpoint on `socket` that hears only what `admit` lets in.
    pub(crate) fn endpoint(
        socket: UdpSocket,
        server: quinn::ServerConfig,
        admit: Admit,
    ) -> Result<Endpoint> {
        socket.set_nonblocking(true)?;
        let runtime = Arc::new(TokioRuntime);
        let gate = Arc::new(Gate {
            inner: runtime.wrap_udp_socket(socket)?,
            admit,
        });
        Ok(Endpoint::new_with_abstract_socket(
            EndpointConfig::default(),
            Some(server),
            gate,
            runtime,
        )?)
    }

    fn admits(&self, from: SocketAddr) -> bool {
        (self.admit)(canonical(from).ip())
    }
}

/// Keeps the datagrams `keep` says yes to at the front of `bufs` and `meta`, in order.
/// Returns how many there are.
fn keep_only(
    bufs: &mut [IoSliceMut<'_>],
    meta: &mut [RecvMeta],
    count: usize,
    mut keep: impl FnMut(&RecvMeta) -> bool,
) -> usize {
    let mut kept = 0;
    for i in 0..count {
        if !keep(&meta[i]) {
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
            let kept = keep_only(bufs, meta, count, |meta| self.admits(meta.addr));
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
        let kept = keep_only(&mut bufs, &mut meta, 4, |meta| {
            meta.addr == addrs[1] || meta.addr == addrs[3]
        });
        assert_eq!(kept, 2);
        assert_eq!(meta[0].addr, addrs[1]);
        assert_eq!(meta[1].addr, addrs[3]);
        assert_eq!(&bufs[0][..3], &[2, 2, 2]);
        assert_eq!(&bufs[1][..3], &[4, 4, 4]);
    }
}
