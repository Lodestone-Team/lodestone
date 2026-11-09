//! The socket itself: a Unix domain socket on every platform.
//!
//! On macOS and Linux these are `std`'s [`UnixStream`](std::os::unix::net::UnixStream)
//! and [`UnixListener`](std::os::unix::net::UnixListener). Windows has had `AF_UNIX`
//! sockets since Windows 10 1803, but `std` doesn't expose them there, so [`windows`]
//! wraps a `socket2` socket with the few methods the server and client use, under the
//! same names.

#[cfg(unix)]
pub use std::os::unix::net::{UnixListener as Listener, UnixStream as Stream};

#[cfg(windows)]
pub use windows::{Listener, Stream};

/// Whether the file at `path` is a socket (a stale one, if nothing answers on it).
#[cfg(unix)]
pub(crate) fn is_socket(path: &std::path::Path) -> bool {
    use std::os::unix::fs::FileTypeExt;
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_socket())
}

/// Whether the file at `path` is a socket (a stale one, if nothing answers on it): on
/// Windows, a reparse point tagged `IO_REPARSE_TAG_AF_UNIX`.
#[cfg(windows)]
pub(crate) fn is_socket(path: &std::path::Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        Storage::FileSystem::{
            FILE_ATTRIBUTE_REPARSE_POINT, FindClose, FindFirstFileW, WIN32_FIND_DATAW,
        },
    };
    /// `IO_REPARSE_TAG_AF_UNIX`, from winnt.h.
    const AF_UNIX_TAG: u32 = 0x8000_0023;
    // a pattern would match other files
    if path.to_string_lossy().contains(['*', '?']) {
        return false;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: WIN32_FIND_DATAW is plain data, valid when zeroed.
    let mut data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
    // SAFETY: `wide` is NUL-terminated and `data` is valid for writes.
    let handle = unsafe { FindFirstFileW(wide.as_ptr(), &mut data) };
    if handle == INVALID_HANDLE_VALUE {
        return false;
    }
    // SAFETY: a search handle from FindFirstFileW, closed once.
    unsafe { FindClose(handle) };
    data.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 && data.dwReserved0 == AF_UNIX_TAG
}

#[cfg(windows)]
mod windows {
    use std::{
        io::{self, Read, Write},
        net::Shutdown,
        os::windows::io::{AsRawSocket, RawSocket},
        path::Path,
        time::Duration,
    };

    use socket2::{Domain, SockAddr, Socket, Type};

    /// A connected `AF_UNIX` stream socket.
    #[derive(Debug)]
    pub struct Stream(Socket);

    impl Stream {
        /// Connects to the socket at `path`.
        pub fn connect(path: impl AsRef<Path>) -> io::Result<Stream> {
            let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
            socket.connect(&SockAddr::unix(path)?)?;
            Ok(Stream(socket))
        }

        /// Another handle to the same socket, for reading on one thread and writing on
        /// another.
        pub fn try_clone(&self) -> io::Result<Stream> {
            self.0.try_clone().map(Stream)
        }

        /// How long a read waits; `None` waits forever. A read that times out fails
        /// with [`io::ErrorKind::TimedOut`] or [`io::ErrorKind::WouldBlock`].
        pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
            if timeout == Some(Duration::ZERO) {
                // as std: Windows would take a zero timeout as "forever"
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "a zero timeout is not allowed",
                ));
            }
            self.0.set_read_timeout(timeout)
        }

        /// Shuts the socket down, for every handle to it. With [`Shutdown::Both`] it
        /// also wakes a read that another handle is waiting in, as on Unix: Windows
        /// lets that read wait on, so its I/O is cancelled (the read fails).
        pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
            let result = self.0.shutdown(how);
            if how == Shutdown::Both {
                // SAFETY: the socket is a valid handle owned by `self`; a null
                // OVERLAPPED cancels all of its pending I/O, whichever handle (clones
                // share the file object) and thread started it.
                unsafe {
                    windows_sys::Win32::System::IO::CancelIoEx(
                        self.0.as_raw_socket() as windows_sys::Win32::Foundation::HANDLE,
                        std::ptr::null(),
                    )
                };
            }
            result
        }
    }

    impl AsRawSocket for Stream {
        fn as_raw_socket(&self) -> RawSocket {
            self.0.as_raw_socket()
        }
    }

    impl Read for Stream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            (&self.0).read(buf)
        }
    }

    impl Read for &Stream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            (&self.0).read(buf)
        }
    }

    impl Write for Stream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            (&self.0).write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Write for &Stream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            (&self.0).write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A listening `AF_UNIX` socket.
    #[derive(Debug)]
    pub struct Listener(Socket);

    impl Listener {
        /// Creates the socket file at `path` and listens on it. Fails when a file is
        /// there already; the file stays after the listener closes.
        pub fn bind(path: impl AsRef<Path>) -> io::Result<Listener> {
            let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
            socket.bind(&SockAddr::unix(path)?)?;
            socket.listen(128)?;
            Ok(Listener(socket))
        }

        /// Waits for the next connection.
        pub fn accept(&self) -> io::Result<Stream> {
            self.0.accept().map(|(socket, _)| Stream(socket))
        }

        /// The connections, as they come; never ends.
        pub fn incoming(&self) -> impl Iterator<Item = io::Result<Stream>> + '_ {
            std::iter::repeat_with(move || self.accept())
        }
    }
}
