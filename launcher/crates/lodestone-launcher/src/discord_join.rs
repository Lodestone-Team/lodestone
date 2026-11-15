//! Discord's game invites, the launcher's side: a friend hosting a server with
//! Lodestone Host shows it in their Discord status (see
//! `launcher_core::discord::invite`). Pressing Join on it, or on the "Invite to join"
//! card they sent, makes Discord hand the server's invite to this launcher
//! (`ACTIVITY_JOIN`), which opens it in the join dialog as a `lodestone://join` link
//! does. Joining still needs the person's click on Play.
//!
//! If the launcher isn't running, Discord starts it with the command left by
//! [`register`]: it opens [`WAKE_LINK`], and the launcher connects to Discord as it
//! starts, which then hands over the invite. If it was running already but not
//! connected (it waits between tries while Discord is closed), the link reaches it
//! (Windows: through the second process; macOS: from the system) and [`wake`]s the
//! thread to connect at once.
//!
//! One thread: it waits on Discord's connection without waking up, and while
//! Discord is closed it tries again from every 5 s to every 60 s.

use std::sync::{Mutex, OnceLock, mpsc};

use futures::channel::mpsc::UnboundedSender;
use launcher_core::discord::{
    self,
    invite::{CONFIRM_WAIT, LAUNCHER_EVENTS, invite_from_secret, join_loop},
    ipc::Connection,
};

/// The link Discord opens to start the launcher for a join: it only connects to
/// Discord, which then hands over the invite.
pub const WAKE_LINK: &str = "lodestone://discord";

/// Wakes the thread.
static WAKE: OnceLock<Mutex<mpsc::Sender<()>>> = OnceLock::new();

/// Whether `link` is [`WAKE_LINK`].
pub fn is_wake_link(link: &str) -> bool {
    link.trim()
        .trim_end_matches('/')
        .eq_ignore_ascii_case(WAKE_LINK)
}

/// Starts the thread, which hands each invite Discord sends to `links` (as a
/// `lodestone://join` link). Once.
pub fn start(links: UnboundedSender<String>) {
    let Some(client_id) = discord::client_id() else {
        return;
    };
    let (wake, woken) = mpsc::channel();
    if WAKE.set(Mutex::new(wake)).is_err() {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("discord-join".into())
        .spawn(move || {
            join_loop(
                woken,
                || Connection::open_subscribed(&client_id, LAUNCHER_EVENTS, CONFIRM_WAIT),
                |secret| {
                    if let Some(link) = link_for_secret(&secret) {
                        let _ = links.unbounded_send(link);
                    }
                },
            )
        });
    if let Err(err) = spawned {
        eprintln!("lodestone-launcher: couldn't start the Discord join thread: {err}");
    }
}

/// Connects to Discord at once rather than at the next try.
pub fn wake() {
    if let Some(wake) = WAKE.get()
        && let Ok(wake) = wake.lock()
    {
        let _ = wake.send(());
    }
}

/// The join dialog's link for a join secret; `None` for anything but an invite.
fn link_for_secret(secret: &str) -> Option<String> {
    invite_from_secret(secret).map(|invite| invite.deep_link())
}

/// Leaves Discord the command that starts the launcher for a join (in the background:
/// a registry or file write). Not from development builds, which would take it over
/// from the installed launcher.
pub fn register() {
    if cfg!(debug_assertions) {
        return;
    }
    let Some(client_id) = discord::client_id() else {
        return;
    };
    std::thread::spawn(move || {
        if let Err(err) = discord::invite::register_launch_command(&client_id, WAKE_LINK) {
            eprintln!("lodestone-launcher: couldn't tell Discord how to start the launcher: {err}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_opens_the_invite_and_nothing_else() {
        let link = format!(
            "https://connect.lodestone.cc/j/7KQ2M9XB4D#{}.{}",
            "3f".repeat(32),
            "c0".repeat(16)
        );
        let invite = launcher_core::cores::connect::Invite::parse(&link).unwrap();
        let secret = discord::invite::join_secret(&invite).unwrap();
        let deep = link_for_secret(&secret).expect("a link");
        assert!(crate::deep_link::is_link(&deep));
        let parsed = launcher_core::cores::connect::Invite::parse(&deep).unwrap();
        assert_eq!(parsed.link(), link);
        // a link or anything else in the secret isn't opened
        assert_eq!(link_for_secret(&link), None);
        assert_eq!(link_for_secret("lodestone://friend?link=x"), None);
    }

    #[test]
    fn the_wake_link_is_a_launcher_link_but_no_invite() {
        assert!(is_wake_link("lodestone://discord"));
        assert!(is_wake_link(" LODESTONE://discord/ "));
        assert!(!is_wake_link("lodestone://join?invite=x"));
        assert!(crate::deep_link::is_link(WAKE_LINK));
        assert!(launcher_core::cores::connect::Invite::parse(WAKE_LINK).is_err());
        assert!(!crate::deep_link::is_friend_link(WAKE_LINK));
    }
}
