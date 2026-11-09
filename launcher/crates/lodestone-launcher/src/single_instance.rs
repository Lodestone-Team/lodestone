//! One launcher at a time, on Windows.
//!
//! macOS opens an app once (a second open activates it), and on Unix the control
//! socket notices a launcher already running. Windows starts a new process for each
//! double-click on the shortcut, and a slow start invites a second one. Two launchers
//! would both write the settings, accounts and instances. So the first holds a named
//! mutex, and a later one brings the first one's window forward and quits.

/// Whether this process may go on: it is the only launcher, or it can't tell.
/// Otherwise it has shown the running launcher's window, and should exit.
///
/// Call it after [`launcher_core::update::wait_for_previous`]: a launcher restarted
/// by an update goes on whatever happens, as the old one is quitting anyway.
#[cfg(windows)]
pub fn acquire(window_title: &str) -> bool {
    use windows_sys::Win32::{
        Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
        System::Threading::CreateMutexW,
    };
    let restarted = launcher_core::update::previous_pid(std::env::args_os().skip(1)).is_some();
    // Per sign-in session, like the data folder (which is per user).
    let name: Vec<u16> = "Local\\LodestoneLauncher.SingleInstance"
        .encode_utf16()
        .chain([0])
        .collect();
    // SAFETY: `name` is NUL-terminated; no security attributes; not taken (owned).
    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        return true;
    }
    // SAFETY: no arguments; reads this thread's last error, set by CreateMutexW.
    let existed = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    // The handle is kept open (never closed) for as long as the process runs: the
    // system closes it when the process exits, and then the name is free again.
    if !existed || restarted {
        return true;
    }
    show_running(window_title);
    false
}

#[cfg(not(windows))]
pub fn acquire(_window_title: &str) -> bool {
    true
}

/// Restores and brings forward the running launcher's main window. GPUI's windows
/// are of class `Zed::Window`; the main one has the app's name as its title.
#[cfg(windows)]
fn show_running(window_title: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, IsIconic, SW_RESTORE, SetForegroundWindow, ShowWindow,
    };
    let wide = |text: &str| -> Vec<u16> { text.encode_utf16().chain([0]).collect() };
    let (class, title) = (wide("Zed::Window"), wide(window_title));
    // SAFETY: both strings are NUL-terminated.
    let hwnd = unsafe { FindWindowW(class.as_ptr(), title.as_ptr()) };
    if hwnd.is_null() {
        // Still starting: its window opens shortly.
        return;
    }
    // SAFETY: `hwnd` is a window handle FindWindowW returned; these calls only ask
    // the system to show and activate it, and fail harmlessly if it is gone.
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd);
    }
}
