//! Links opened from outside the launcher: `lodestone://join?invite=…`, which the invite
//! page at connect.lodestone.cc opens (see `servers::friends`), and
//! `lodestone://friend?link=<percent-encoded friend link>`, which its friend link page
//! opens (see `servers::friend_list`): that one opens Add friend with the link's card,
//! and never adds anyone by itself.
//!
//! - macOS hands the link to the running app (starting it first if needed), through
//!   GPUI's open-URLs callback. The app bundle declares the `lodestone` URL scheme
//!   (`CFBundleURLTypes`, written by `scripts/publish_release.py`).
//! - Windows starts `lodestone-launcher.exe "lodestone://…"` (the installer registers
//!   the scheme for the user). When a launcher is already running, the new process
//!   passes the link to it over the control socket (`friends.open`) and quits.
//!
//! Either way the link ends up in one channel, and the window opens the join dialog
//! for each link that comes out of it. The dialog only shows the server: joining needs
//! the user's click on Play.

use std::{path::Path, time::Duration};

use futures::{StreamExt as _, channel::mpsc};
use gpui::{App, WindowHandle};

use crate::shell::LauncherShell;

/// The scheme the launcher opens.
pub const SCHEME: &str = "lodestone://";

/// The longest link taken, so a stray argument can't be huge.
const MAX_LINK: usize = 4096;

/// Whether `text` is a link for the launcher to open.
pub fn is_link(text: &str) -> bool {
    let text = text.trim();
    text.len() <= MAX_LINK
        && text
            .get(..SCHEME.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(SCHEME))
}

/// Whether `text` is a friend link (`https://…/f/…` or `lodestone://friend?link=…`),
/// for Add friend rather than the join dialog.
pub fn is_friend_link(text: &str) -> bool {
    text.len() <= MAX_LINK && launcher_core::cores::friends::FriendLink::parse(text).is_ok()
}

/// The link among the command line's arguments (Windows passes it as one).
pub fn from_args(args: impl IntoIterator<Item = std::ffi::OsString>) -> Option<String> {
    args.into_iter()
        .filter_map(|arg| arg.into_string().ok())
        .find(|arg| is_link(arg))
        .map(|arg| arg.trim().to_string())
}

/// Where links wait until the window is open.
pub struct Links {
    tx: mpsc::UnboundedSender<String>,
    rx: Option<mpsc::UnboundedReceiver<String>>,
}

impl Default for Links {
    fn default() -> Self {
        let (tx, rx) = mpsc::unbounded();
        Self { tx, rx: Some(rx) }
    }
}

impl Links {
    /// Something to hand links to, from any thread.
    pub fn sender(&self) -> mpsc::UnboundedSender<String> {
        self.tx.clone()
    }

    /// Opens each link in `window`'s join dialog, bringing the window forward: those
    /// that came before it opened, then those that come later.
    pub fn serve(mut self, window: WindowHandle<LauncherShell>, cx: &mut App) {
        let Some(mut rx) = self.rx.take() else {
            return;
        };
        cx.spawn(async move |cx| {
            while let Some(link) = rx.next().await {
                if !is_link(&link) {
                    continue;
                }
                let opened = window.update(cx, |shell, window, cx| {
                    window.activate_window();
                    shell.open_invite(link, cx);
                });
                if opened.is_err() {
                    break;
                }
            }
        })
        .detach();
    }
}

/// Hands `link` to the launcher that is already running (Windows: the second process
/// started for the link). Tries for a few seconds, as that launcher may still be
/// starting. Returns whether it took the link.
pub fn forward(link: &str) -> bool {
    let Some(data_dir) = launcher_control::paths::data_dir() else {
        return false;
    };
    forward_to(&launcher_control::paths::socket_path(&data_dir), link)
}

fn forward_to(socket: &Path, link: &str) -> bool {
    for _ in 0..20 {
        if let Ok(mut client) = launcher_control::client::Client::connect(socket) {
            let params = serde_json::json!({ "link": link });
            return matches!(client.call("friends.open", params), Ok(Ok(_)));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friend_links_are_told_apart_from_invites() {
        // made the way the friend list makes them: the code comes from the secret
        let service = launcher_core::cores::friends::Service::parse("http://127.0.0.1:18790")
            .expect("a service");
        let link = friend_link(&service);
        assert!(is_friend_link(&link));
        let encoded: String = url_encode(&link);
        let deep = format!("lodestone://friend?link={encoded}");
        assert!(is_link(&deep));
        assert!(is_friend_link(&deep));
        // an invite, a broken friend link and a deep link inside a deep link aren't
        assert!(!is_friend_link("https://connect.lodestone.cc/j/ABCDEFGH23#ab.cd"));
        assert!(!is_friend_link("https://connect.lodestone.cc/f/ABC#00.11"));
        let nested = format!("lodestone://friend?link={}", url_encode(&deep));
        assert!(!is_friend_link(&nested));
    }

    fn url_encode(text: &str) -> String {
        url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
    }

    /// A well-formed friend link for `service`.
    fn friend_link(service: &launcher_core::cores::friends::Service) -> String {
        crate::servers::friend_list::test_link(service)
    }

    #[test]
    fn finds_the_link_among_the_arguments() {
        let args = |list: &[&str]| {
            list.iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>()
        };
        assert_eq!(from_args(args(&[])), None);
        assert_eq!(from_args(args(&["--after-update", "12"])), None);
        assert_eq!(
            from_args(args(&["lodestone://join?invite=x"])).as_deref(),
            Some("lodestone://join?invite=x")
        );
        assert_eq!(
            from_args(args(&["--flag", "LODESTONE://join?invite=y "])).as_deref(),
            Some("LODESTONE://join?invite=y")
        );
        assert!(!is_link("https://connect.lodestone.cc/j/ABC"));
        // a friend deep link is one of the launcher's too
        assert!(is_link("lodestone://friend?link=x"));
        assert!(!is_link(&format!("lodestone://{}", "a".repeat(MAX_LINK))));
    }
}
