//! Binds the core's HTTP port on every network interface, over IPv4 and IPv6.
//!
//! A socket bound to `[::]` accepts IPv4 clients too (as `::ffff:a.b.c.d` addresses) only if
//! its `IPV6_V6ONLY` option is off. The default differs by OS: off on Linux (unless the
//! `net.ipv6.bindv6only` sysctl says otherwise), on on Windows. A plain `bind("[::]:port")`
//! therefore serves only IPv6 on Windows, and `127.0.0.1` is refused. The option is set
//! explicitly here.

use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};

use socket2::{Domain, Protocol, Socket, Type};
use tracing::debug;

const BACKLOG: i32 = 1024;

/// The listening sockets of the core, already bound and in non-blocking mode.
pub struct Listeners {
    pub listeners: Vec<TcpListener>,
    /// Whether the one listener serves IPv4 and IPv6, instead of one listener per family.
    pub dual_stack: bool,
}

impl Listeners {
    /// For logs, such as `[::]:16662 (IPv4 and IPv6)`.
    pub fn describe(&self) -> String {
        let addrs: Vec<String> = self
            .listeners
            .iter()
            .filter_map(|listener| listener.local_addr().ok())
            .map(|addr| addr.to_string())
            .collect();
        let addrs = addrs.join(" and ");
        if self.dual_stack {
            format!("{addrs} (IPv4 and IPv6)")
        } else {
            addrs
        }
    }
}

fn bind_socket(addr: SocketAddr, only_v6: Option<bool>) -> io::Result<TcpListener> {
    let socket = Socket::new(Domain::for_address(addr), Type::STREAM, Some(Protocol::TCP))?;
    if let Some(only_v6) = only_v6 {
        socket.set_only_v6(only_v6)?;
    }
    // What std does: on Unix, a restarted core can bind its port again at once. Not on
    // Windows, where SO_REUSEADDR would let another program take the port over.
    #[cfg(unix)]
    socket.set_reuse_address(true)?;
    // A plain bind on Windows succeeds although another program holds the port on another
    // address: 127.0.0.1, `0.0.0.0` under this `[::]`, a LAN address. That program would
    // then get some of the connections. An exclusive bind fails instead, and also stops
    // other programs from binding the port while the core has it.
    #[cfg(windows)]
    set_exclusive_address_use(&socket)?;
    socket.bind(&addr.into())?;
    socket.listen(BACKLOG)?;
    socket.set_nonblocking(true)?;
    Ok(socket.into())
}

/// Sets `SO_EXCLUSIVEADDRUSE`, which socket2 has no setter for.
#[cfg(windows)]
fn set_exclusive_address_use(socket: &Socket) -> io::Result<()> {
    use std::os::windows::io::AsRawSocket;
    use windows_sys::Win32::Networking::WinSock::{
        setsockopt, SOCKET, SOCKET_ERROR, SOL_SOCKET, SO_EXCLUSIVEADDRUSE,
    };

    let enable: i32 = 1;
    // SAFETY: the socket stays open for the call, and the option value points to an i32
    // that outlives it, with its size.
    let result = unsafe {
        setsockopt(
            socket.as_raw_socket() as SOCKET,
            SOL_SOCKET,
            SO_EXCLUSIVEADDRUSE,
            (&enable as *const i32).cast(),
            std::mem::size_of::<i32>() as i32,
        )
    };
    if result == SOCKET_ERROR {
        // Winsock's error is the thread's last error, as WSAGetLastError returns it
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Whether binding failed because something else has the port.
///
/// On Windows a port in a range the system reserved (Hyper-V and WSL reserve some, and
/// the ranges can change at each boot) gives "access denied" (WSAEACCES) rather than "in
/// use". A port another program holds gives "in use", even with the exclusive bind
/// (`SO_EXCLUSIVEADDRUSE`, see [`bind_socket`]) against a holder using `SO_REUSEADDR`.
/// Either way the port can't be had, so it counts as taken: the core then says so (telling
/// a reserved port apart), and a debug build moves on to the next one.
pub fn port_taken(error: &io::Error) -> bool {
    match error.kind() {
        io::ErrorKind::AddrInUse => true,
        kind => cfg!(windows) && kind == io::ErrorKind::PermissionDenied,
    }
}

/// One socket on `[::]` that accepts IPv4 and IPv6 clients.
fn bind_dual_stack(port: u16) -> io::Result<TcpListener> {
    bind_socket((Ipv6Addr::UNSPECIFIED, port).into(), Some(false))
}

/// A socket for IPv4 and a socket for IPv6, on the same port, for systems that cannot do
/// dual-stack sockets, or that have no IPv6.
fn bind_split(port: u16) -> io::Result<Vec<TcpListener>> {
    let mut listeners = Vec::new();
    let mut port = port;
    match bind_socket((Ipv6Addr::UNSPECIFIED, port).into(), Some(true)) {
        Ok(listener) => {
            port = listener.local_addr()?.port();
            listeners.push(listener);
        }
        Err(e) if port_taken(&e) => return Err(e),
        Err(e) => debug!("IPv6 is not available: {e}"),
    }
    listeners.push(bind_socket((Ipv4Addr::UNSPECIFIED, port).into(), None)?);
    Ok(listeners)
}

/// Binds `port` on all interfaces: a single dual-stack `[::]` socket where the system has
/// one, otherwise one socket per address family. With a `port` of 0, the system picks one.
///
/// Fails if the port is taken (an error [`port_taken`] says so of), or if not even IPv4
/// can be bound.
pub fn bind_all_interfaces(port: u16) -> io::Result<Listeners> {
    // SO_REUSEADDR lets a wildcard bind on macOS succeed although another program listens
    // on 127.0.0.1. Ask instead: Unix refuses a connection to a closed port at once.
    #[cfg(unix)]
    if port != 0 && crate::util::port_in_use(port) {
        return Err(io::ErrorKind::AddrInUse.into());
    }
    match bind_dual_stack(port) {
        Ok(listener) => {
            return Ok(Listeners {
                listeners: vec![listener],
                dual_stack: true,
            })
        }
        Err(e) if port_taken(&e) => return Err(e),
        Err(e) => debug!("Cannot bind a dual-stack socket, binding IPv4 and IPv6 separately: {e}"),
    }
    Ok(Listeners {
        listeners: bind_split(port)?,
        dual_stack: false,
    })
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    use super::*;
    use crate::tls::is_loopback;

    /// The next connection on any of `listeners`.
    fn accept_any(listeners: &[TcpListener]) -> (TcpStream, SocketAddr) {
        for _ in 0..500 {
            for listener in listeners {
                match listener.accept() {
                    Ok(accepted) => return accepted,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(e) => panic!("accept failed: {e}"),
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("no connection arrived");
    }

    /// Connects to `target` and checks that a listener accepts the connection from an
    /// address that counts as this machine, and that data flows.
    fn assert_serves(listeners: &[TcpListener], target: SocketAddr) {
        let mut client = TcpStream::connect_timeout(&target, std::time::Duration::from_secs(5))
            .unwrap_or_else(|e| panic!("connecting to {target} failed: {e}"));
        let (mut server, peer) = accept_any(listeners);
        assert!(is_loopback(peer.ip()), "{peer} should be loopback");
        server.set_nonblocking(false).unwrap();
        server.write_all(b"hi").unwrap();
        let mut reply = [0_u8; 2];
        client.read_exact(&mut reply).unwrap();
        assert_eq!(&reply, b"hi");
    }

    /// Checks that `listeners` serve `127.0.0.1` and, if the machine has an IPv6 loopback,
    /// `::1`.
    fn assert_serves_both_families(listeners: &[TcpListener]) {
        let port = listeners[0].local_addr().unwrap().port();
        assert_serves(listeners, (Ipv4Addr::LOCALHOST, port).into());
        if TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).is_ok() {
            assert_serves(listeners, (Ipv6Addr::LOCALHOST, port).into());
        }
    }

    #[test]
    fn a_dual_stack_socket_serves_ipv4_and_ipv6() {
        let listener = match bind_dual_stack(0) {
            Ok(listener) => listener,
            // no IPv6 here: `bind_all_interfaces` falls back, see the other tests
            Err(e) => {
                eprintln!("skipped, no dual-stack sockets: {e}");
                return;
            }
        };
        assert!(listener.local_addr().unwrap().is_ipv6());
        assert_serves_both_families(&[listener]);
    }

    #[test]
    fn separate_sockets_serve_ipv4_and_ipv6() {
        let listeners = bind_split(0).unwrap();
        assert!(listeners.len() <= 2);
        assert_serves_both_families(&listeners);
    }

    #[test]
    fn all_interfaces_serves_ipv4_and_ipv6() {
        let bound = bind_all_interfaces(0).unwrap();
        assert!(!bound.listeners.is_empty());
        assert!(!bound.describe().is_empty());
        assert_serves_both_families(&bound.listeners);
    }

    /// Another program holding the port on any one address, even one the core's sockets
    /// do not name, keeps the core off it. (A plain bind on Windows would succeed.)
    #[test]
    fn a_port_held_on_another_address_is_an_error() {
        let mut holders = vec![
            SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
            SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
        ];
        // macOS lets the core's `[::]` share the port with a `::1` listener, as before
        #[cfg(not(target_os = "macos"))]
        if TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).is_ok() {
            holders.push((Ipv6Addr::LOCALHOST, 0).into());
        }
        for holder in holders {
            let held = TcpListener::bind(holder).unwrap();
            let port = held.local_addr().unwrap().port();
            assert_eq!(
                bind_all_interfaces(port).err().map(|e| e.kind()),
                Some(io::ErrorKind::AddrInUse),
                "a listener on {holder} with port {port}"
            );
        }
    }

    /// A port another program holds with `SO_REUSEADDR` is taken too.
    #[test]
    fn a_port_held_with_reuse_address_is_taken() {
        for holder in [
            SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
            SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
        ] {
            let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).unwrap();
            socket.set_reuse_address(true).unwrap();
            socket.bind(&holder.into()).unwrap();
            socket.listen(1).unwrap();
            let held: TcpListener = socket.into();
            let port = held.local_addr().unwrap().port();
            let error = bind_all_interfaces(port)
                .err()
                .unwrap_or_else(|| panic!("bound port {port} that {holder} holds"));
            assert!(port_taken(&error), "{holder}, port {port}: {error:?}");
        }
    }

    /// A port in a range Windows reserved (Hyper-V and WSL reserve some) is refused with
    /// "access denied", and counts as taken.
    #[cfg(windows)]
    #[test]
    fn a_reserved_port_is_taken() {
        let output = std::process::Command::new("netsh")
            .args(["int", "ipv4", "show", "excludedportrange", "protocol=tcp"])
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&output.stdout);
        // the rows are "<start> <end>", the first one after the header's dashes
        let reserved = text.lines().find_map(|line| {
            let mut ports = line.split_whitespace().map(str::parse::<u16>);
            match (ports.next(), ports.next()) {
                (Some(Ok(start)), Some(Ok(_))) => Some(start),
                _ => None,
            }
        });
        let Some(port) = reserved else {
            eprintln!("skipped, no reserved ports here");
            return;
        };
        let error = bind_all_interfaces(port)
            .err()
            .unwrap_or_else(|| panic!("bound reserved port {port}"));
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied, "port {port}");
        assert!(port_taken(&error));
    }

    #[test]
    fn what_counts_as_a_taken_port() {
        assert!(port_taken(&io::ErrorKind::AddrInUse.into()));
        assert_eq!(
            port_taken(&io::ErrorKind::PermissionDenied.into()),
            cfg!(windows)
        );
        assert!(!port_taken(&io::ErrorKind::AddrNotAvailable.into()));
    }

    #[test]
    fn a_taken_port_is_an_error() {
        let bound = bind_all_interfaces(0).unwrap();
        let port = bound.listeners[0].local_addr().unwrap().port();
        let again = bind_all_interfaces(port);
        assert_eq!(
            again.err().map(|e| e.kind()),
            Some(io::ErrorKind::AddrInUse)
        );
    }
}
