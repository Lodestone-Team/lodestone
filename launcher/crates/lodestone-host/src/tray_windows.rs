//! The notification area icon on Windows, through the shell's `Shell_NotifyIconW`.
//!
//! A thread of its own owns a message-only window that receives the icon's clicks: a
//! left click shows the Host, a right click opens the menu (Show, Quit). The icon is
//! added again when Explorer restarts (`TaskbarCreated`), and removed when the
//! [`Item`] is dropped.

use std::{
    ffi::c_void,
    sync::{Mutex, OnceLock},
};

use futures::channel::mpsc::UnboundedSender;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM},
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Shell::{
            NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
        },
        WindowsAndMessaging::{
            AppendMenuW, CreateIconFromResourceEx, CreatePopupMenu, CreateWindowExW,
            DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos,
            GetMessageW, HICON, HWND_MESSAGE, LR_DEFAULTCOLOR, MF_SEPARATOR, MF_STRING, MSG,
            PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW,
            SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage,
            WM_APP, WM_CLOSE, WM_DESTROY, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW,
        },
    },
};

use super::TrayCommand;

/// The message the icon sends its clicks with.
const WM_TRAY: u32 = WM_APP + 1;
/// The menu's commands.
const ID_SHOW: usize = 1;
const ID_QUIT: usize = 2;

/// What the window procedure needs: where commands go, and the menu's labels.
struct Shared {
    tx: UnboundedSender<TrayCommand>,
    show: Vec<u16>,
    quit: Vec<u16>,
    icon: HICON,
    taskbar_created: u32,
}

// SAFETY: the icon handle is only used on the tray's thread; the rest is Send.
unsafe impl Send for Shared {}

static SHARED: OnceLock<Mutex<Option<Shared>>> = OnceLock::new();

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn with_shared<T>(f: impl FnOnce(&Shared) -> T) -> Option<T> {
    let slot = SHARED.get()?;
    let guard = slot.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().map(f)
}

fn send(command: TrayCommand) {
    with_shared(|shared| {
        let _ = shared.tx.unbounded_send(command);
    });
}

/// The icon's data, for adding or removing it.
fn icon_data(hwnd: HWND, icon: HICON) -> NOTIFYICONDATAW {
    // SAFETY: NOTIFYICONDATAW is plain data; zero is its empty value.
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    data.uCallbackMessage = WM_TRAY;
    data.hIcon = icon;
    let tip = crate::APP_NAME.encode_utf16().collect::<Vec<_>>();
    let len = tip.len().min(data.szTip.len() - 1);
    data.szTip[..len].copy_from_slice(&tip[..len]);
    data
}

fn add_icon(hwnd: HWND) {
    let icon = with_shared(|shared| shared.icon).unwrap_or(std::ptr::null_mut());
    let data = icon_data(hwnd, icon);
    // SAFETY: `data` is filled in for this window.
    unsafe { Shell_NotifyIconW(NIM_ADD, &data) };
}

/// The right-click menu, at the pointer; sends the chosen command.
fn show_menu(hwnd: HWND) {
    let Some((show, quit)) = with_shared(|shared| (shared.show.clone(), shared.quit.clone()))
    else {
        return;
    };
    // SAFETY: the menu is made, shown and destroyed here, on the window's thread;
    // the labels outlive the calls.
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return;
        }
        AppendMenuW(menu, MF_STRING, ID_SHOW, show.as_ptr());
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(menu, MF_STRING, ID_QUIT, quit.as_ptr());
        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&mut point);
        // so the menu closes when the user clicks elsewhere
        SetForegroundWindow(hwnd);
        let chosen = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        DestroyMenu(menu);
        match chosen as usize {
            ID_SHOW => send(TrayCommand::Show),
            ID_QUIT => send(TrayCommand::Quit),
            _ => {}
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_TRAY => {
            match lparam as u32 {
                WM_LBUTTONUP => send(TrayCommand::Show),
                WM_RBUTTONUP => show_menu(hwnd),
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            let data = icon_data(hwnd, std::ptr::null_mut());
            // SAFETY: removes this window's icon, then the window.
            unsafe {
                Shell_NotifyIconW(NIM_DELETE, &data);
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            // SAFETY: ends this thread's message loop.
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => {
            if with_shared(|shared| shared.taskbar_created) == Some(message) {
                // Explorer restarted: the icon is gone, add it again
                add_icon(hwnd);
                return 0;
            }
            // SAFETY: the default handling of every other message.
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
    }
}

/// The icon's window: dropping it removes the icon.
pub struct Item {
    hwnd: usize,
}

impl Drop for Item {
    fn drop(&mut self) {
        // SAFETY: asks the tray's thread to remove the icon and close its window.
        unsafe { PostMessageW(self.hwnd as HWND, WM_CLOSE, 0, 0) };
    }
}

pub fn install(show: &str, quit: &str, tx: UnboundedSender<TrayCommand>) -> Option<Item> {
    let png: &'static [u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/images/host-icon-32.png"
    ));
    let (show, quit) = (wide(show), wide(quit));
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Option<usize>>();
    std::thread::Builder::new()
        .name("tray".into())
        .spawn(move || {
            // SAFETY: Win32 calls on this thread, with values made here; the class
            // name and title outlive the window.
            unsafe {
                // a PNG makes an icon since Windows Vista
                let icon = CreateIconFromResourceEx(
                    png.as_ptr(),
                    png.len() as u32,
                    1,
                    0x0003_0000,
                    0,
                    0,
                    LR_DEFAULTCOLOR,
                );
                let taskbar_created = RegisterWindowMessageW(wide("TaskbarCreated").as_ptr());
                let slot = SHARED.get_or_init(|| Mutex::new(None));
                *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(Shared {
                    tx,
                    show,
                    quit,
                    icon,
                    taskbar_created,
                });
                let instance = GetModuleHandleW(std::ptr::null());
                let class = wide("LodestoneHostTray");
                let mut wc: WNDCLASSW = std::mem::zeroed();
                wc.lpfnWndProc = Some(window_proc);
                wc.hInstance = instance;
                wc.lpszClassName = class.as_ptr();
                RegisterClassW(&wc);
                let title = wide(crate::APP_NAME);
                let hwnd = CreateWindowExW(
                    0,
                    class.as_ptr(),
                    title.as_ptr(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    std::ptr::null_mut(),
                    instance,
                    std::ptr::null::<c_void>(),
                );
                if hwnd.is_null() {
                    let _ = ready_tx.send(None);
                    return;
                }
                add_icon(hwnd);
                let _ = ready_tx.send(Some(hwnd as usize));
                let mut message: MSG = std::mem::zeroed();
                while GetMessageW(&mut message, std::ptr::null_mut(), 0, 0) > 0 {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        })
        .ok()?;
    let hwnd = ready_rx.recv().ok()??;
    Some(Item { hwnd })
}
