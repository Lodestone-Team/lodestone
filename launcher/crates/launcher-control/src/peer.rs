//! Who is on the other end of the socket.
//!
//! On Unix a user is its numeric user id, read from the socket with `getpeereid` or
//! `SO_PEERCRED`. On Windows it is the security identifier (SID) of the user that runs
//! the process on the other end, as text (`S-1-5-21-…`): the socket gives that
//! process's id (`SIO_AF_UNIX_GETPEERPID`), and its access token gives the user.

use std::io;

use crate::transport::Stream;

/// A user, as the platform names it: a uid on Unix, a SID on Windows.
#[cfg(unix)]
pub type UserId = u32;
/// A user, as the platform names it: a uid on Unix, a SID on Windows.
#[cfg(windows)]
pub type UserId = String;

/// What [`UserId`] is called in messages.
#[cfg(unix)]
pub(crate) const USER_ID_NAME: &str = "uid";
#[cfg(windows)]
pub(crate) const USER_ID_NAME: &str = "SID";

/// The user of this process.
#[cfg(unix)]
pub fn current_uid() -> UserId {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() }
}

/// The effective user id of the process connected to `stream`.
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
pub fn peer_uid(stream: &Stream) -> io::Result<UserId> {
    use std::os::fd::AsRawFd;
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    // SAFETY: the descriptor is a connected Unix socket owned by `stream`, and both
    // out-pointers are valid for writes.
    let result = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
    if result == 0 {
        Ok(uid)
    } else {
        Err(io::Error::last_os_error())
    }
}

/// The effective user id of the process connected to `stream`.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn peer_uid(stream: &Stream) -> io::Result<UserId> {
    use std::os::fd::AsRawFd;
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: the descriptor is a connected Unix socket owned by `stream`; `cred` and
    // `len` are valid for writes and `len` holds the size of `cred`.
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            (&mut cred as *mut libc::ucred).cast(),
            &mut len,
        )
    };
    if result == 0 {
        Ok(cred.uid)
    } else {
        Err(io::Error::last_os_error())
    }
}

/// The SID of this process's user. Empty if it can't be read, which matches no peer.
#[cfg(windows)]
pub fn current_uid() -> UserId {
    static CURRENT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CURRENT
        .get_or_init(|| {
            // SAFETY: GetCurrentProcess returns a pseudo handle that needs no closing.
            let process = unsafe { windows_sys::Win32::System::Threading::GetCurrentProcess() };
            windows::process_user(process).unwrap_or_default()
        })
        .clone()
}

/// The SID of the user running the process connected to `stream`.
#[cfg(windows)]
pub fn peer_uid(stream: &Stream) -> io::Result<UserId> {
    windows::peer_user(stream)
}

#[cfg(windows)]
mod windows {
    use std::{io, os::windows::io::AsRawSocket, ptr};

    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE, LocalFree},
        Networking::WinSock::{SIO_AF_UNIX_GETPEERPID, SOCKET, WSAGetLastError, WSAIoctl},
        Security::{
            Authorization::ConvertSidToStringSidW, GetTokenInformation, TOKEN_QUERY, TOKEN_USER,
            TokenUser,
        },
        System::Threading::{OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION},
    };

    use crate::transport::Stream;

    /// Closes the handle when dropped.
    struct Owned(HANDLE);

    impl Drop for Owned {
        fn drop(&mut self) {
            // SAFETY: the handle was opened by us and is closed only here.
            unsafe { CloseHandle(self.0) };
        }
    }

    pub fn peer_user(stream: &Stream) -> io::Result<String> {
        let mut pid: u32 = 0;
        let mut returned: u32 = 0;
        // SAFETY: the socket is a connected AF_UNIX socket owned by `stream`; the
        // output buffer is `pid`, valid for writes of its size, and the call is
        // synchronous (no overlapped structure or completion routine).
        let result = unsafe {
            WSAIoctl(
                stream.as_raw_socket() as SOCKET,
                SIO_AF_UNIX_GETPEERPID,
                ptr::null(),
                0,
                (&mut pid as *mut u32).cast(),
                std::mem::size_of::<u32>() as u32,
                &mut returned,
                ptr::null_mut(),
                None,
            )
        };
        if result != 0 {
            // SAFETY: no preconditions.
            return Err(io::Error::from_raw_os_error(unsafe { WSAGetLastError() }));
        }
        // SAFETY: OpenProcess has no memory preconditions; a null handle is an error.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return Err(io::Error::last_os_error());
        }
        let process = Owned(process);
        process_user(process.0)
    }

    /// The SID, as text, of the user whose token `process` runs with.
    pub fn process_user(process: HANDLE) -> io::Result<String> {
        let mut token: HANDLE = ptr::null_mut();
        // SAFETY: `process` is a valid process handle and `token` is valid for writes.
        if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = Owned(token);
        let mut len: u32 = 0;
        // SAFETY: asks for the size only: no buffer, and `len` is valid for writes.
        unsafe { GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut len) };
        if len == 0 {
            return Err(io::Error::last_os_error());
        }
        // u64s, so the TOKEN_USER at its start is aligned
        let mut buffer = vec![0u64; (len as usize).div_ceil(8)];
        // SAFETY: the buffer holds at least `len` bytes and is aligned for TOKEN_USER.
        let ok = unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                len,
                &mut len,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: GetTokenInformation wrote a TOKEN_USER at the buffer's start; its SID
        // points into the same buffer, which outlives this use.
        let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
        let mut text: windows_sys::core::PWSTR = ptr::null_mut();
        // SAFETY: `sid` is a valid SID and `text` is valid for writes.
        if unsafe { ConvertSidToStringSidW(sid, &mut text) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: on success `text` is a NUL-terminated wide string from LocalAlloc,
        // read up to its NUL and then freed once.
        let sid = unsafe {
            let len = (0..).take_while(|&i| *text.add(i) != 0).count();
            let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
            LocalFree(text.cast());
            sid
        };
        Ok(sid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn a_socket_pair_is_this_user() {
        let (a, b) = Stream::pair().unwrap();
        assert_eq!(peer_uid(&a).unwrap(), current_uid());
        assert_eq!(peer_uid(&b).unwrap(), current_uid());
    }

    #[cfg(windows)]
    #[test]
    fn a_connection_is_this_user() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("peer.sock");
        let listener = crate::transport::Listener::bind(&path).unwrap();
        let client = Stream::connect(&path).unwrap();
        let server = listener.accept().unwrap();
        let me = current_uid();
        assert!(me.starts_with("S-1-"), "{me}");
        assert_eq!(peer_uid(&client).unwrap(), me);
        assert_eq!(peer_uid(&server).unwrap(), me);
    }
}
