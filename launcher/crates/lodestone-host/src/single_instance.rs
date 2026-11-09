//! One Host at a time: a second start shows the running Host's window and exits.
//! (Two Hosts would start two cores on the same data folder; the second core would
//! fail on its lock anyway.)
//!
//! - Unix: a socket in the Host's folder (`host.sock`). The first Host listens on it;
//!   a later one connects, asks it to show its window and exits.
//! - Windows: a named mutex, and the running Host's window is brought forward by its
//!   title, as the launcher does.

use futures::channel::mpsc::UnboundedSender;

/// What a second start asks of the running Host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShowRequest;

/// Whether this process may go on as the only Host. When another runs, it has been
/// asked to show its window, and this one should exit. Requests from later starts go
/// to `tx`.
#[cfg(unix)]
pub fn acquire(tx: UnboundedSender<ShowRequest>) -> bool {
    use std::{
        io::{Read as _, Write as _},
        os::unix::net::{UnixListener, UnixStream},
    };
    let dir = crate::core_process::host_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return true;
    }
    let path = dir.join("host.sock");
    if let Ok(mut stream) = UnixStream::connect(&path) {
        let _ = stream.write_all(b"show\n");
        return false;
    }
    // nothing answers: a socket left by a Host that crashed
    let _ = std::fs::remove_file(&path);
    let Ok(listener) = UnixListener::bind(&path) else {
        return true;
    };
    let _ = std::thread::Builder::new()
        .name("lodestone-host-single-instance".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                let mut buf = [0u8; 16];
                let read = stream.read(&mut buf).unwrap_or(0);
                if buf[..read].starts_with(b"show") && tx.unbounded_send(ShowRequest).is_err() {
                    break;
                }
            }
        });
    true
}

#[cfg(windows)]
pub fn acquire(_tx: UnboundedSender<ShowRequest>) -> bool {
    use windows_sys::Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
        System::Threading::CreateMutexW,
    };
    let name: Vec<u16> = "Local\\LodestoneHost.SingleInstance"
        .encode_utf16()
        .chain([0])
        .collect();
    // SAFETY: `name` is NUL-terminated; no security attributes; not taken (owned).
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        return true;
    }
    // SAFETY: reads this thread's last error, set by CreateMutexW.
    let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    // The handle stays open for the life of the process.
    if !existed {
        return true;
    }
    show_running(crate::APP_NAME);
    false
}

/// Restores and brings forward the running Host's window (GPUI's windows are of class
/// `Zed::Window`).
#[cfg(windows)]
fn show_running(window_title: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, IsIconic, SW_RESTORE, SW_SHOW, SetForegroundWindow, ShowWindow,
    };
    let wide = |text: &str| -> Vec<u16> { text.encode_utf16().chain([0]).collect() };
    let (class, title) = (wide("Zed::Window"), wide(window_title));
    // SAFETY: both strings are NUL-terminated.
    let hwnd = unsafe { FindWindowW(class.as_ptr(), title.as_ptr()) };
    if hwnd.is_null() {
        return;
    }
    // SAFETY: a window handle FindWindowW returned; these only ask the system to show
    // and activate it, and fail harmlessly if it is gone.
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        } else {
            ShowWindow(hwnd, SW_SHOW);
        }
        SetForegroundWindow(hwnd);
    }
}
