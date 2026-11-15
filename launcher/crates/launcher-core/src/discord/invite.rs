//! Discord's game invites for servers shared with a Lodestone Connect invite.
//!
//! The Host's status says what is hosted, with a party (players online and the most
//! there can be) and a join secret: the server's invite. Discord then shows
//! "Ask to Join" on the host's profile and lets them send an "Invite to join" card in
//! chat. A friend who presses Join gets the secret in their Lodestone Launcher
//! (`ACTIVITY_JOIN`), which opens the invite as a `lodestone://join` link does; if the
//! launcher isn't running, Discord starts it with the command
//! [`register_launch_command`] left for it.
//!
//! Discord takes 128 characters at most in a secret, and the invite link is longer
//! (its fingerprint and token are in hex), so the secret is the same invite written
//! more tightly ([`join_secret`]). It holds the whole invite, key included: anyone
//! Discord hands it to can join, as with the link.
//!
//! The two loops here run on a thread of their own each, and sleep while there's
//! nothing to do:
//! - [`host_loop`] (the Host): connects only while there's a status to show, sends
//!   it only when it changed (at most every [`SEND_GAP`]), and while connected looks
//!   for "Ask to Join" requests once a [`POLL`]. Retries from [`RETRY_MIN`] to
//!   [`RETRY_MAX`] while Discord isn't open.
//! - [`join_loop`] (the launcher): keeps a connection that waits for `ACTIVITY_JOIN`
//!   without waking up, and retries the same way while Discord isn't open (or only
//!   an app that never sends events is).

use std::{
    sync::mpsc::{Receiver, RecvTimeoutError},
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use lodestone_connect::Invite;

use super::ipc::{Activity, Connection, Event, IpcError, JoinRequest, MAX_TEXT, Pipe};

/// The first wait before connecting again; it doubles up to [`RETRY_MAX`].
pub const RETRY_MIN: Duration = Duration::from_secs(5);
pub const RETRY_MAX: Duration = Duration::from_secs(60);
/// The shortest time between two statuses (Discord takes five every 20 s).
pub const SEND_GAP: Duration = Duration::from_secs(5);
/// How often the Host looks for "Ask to Join" requests while showing an invite.
pub const POLL: Duration = Duration::from_secs(1);
/// How long to wait for Discord to confirm a subscription, per app.
pub const CONFIRM_WAIT: Duration = Duration::from_secs(1);

/// What the Host subscribes to, and the launcher.
pub const HOST_EVENTS: &[&str] = &["ACTIVITY_JOIN_REQUEST"];
pub const LAUNCHER_EVENTS: &[&str] = &["ACTIVITY_JOIN"];

/// The version of [`join_secret`]'s form.
const SECRET_VERSION: &str = "1";

/// `invite` as a join secret: `1/<service>/<code>/<fingerprint and token, base64url>`,
/// about 100 characters. `None` if it would be longer than Discord takes (a test
/// service with a long address).
pub fn join_secret(invite: &Invite) -> Option<String> {
    let mut key = Vec::with_capacity(invite.fingerprint.len() + invite.token.len());
    key.extend_from_slice(&invite.fingerprint);
    key.extend_from_slice(&invite.token);
    let secret = format!(
        "{SECRET_VERSION}/{}/{}/{}",
        invite.service.link(),
        invite.code,
        URL_SAFE_NO_PAD.encode(key)
    );
    (secret.len() <= MAX_TEXT).then_some(secret)
}

/// The invite in a join secret from [`join_secret`]; `None` for anything else.
pub fn invite_from_secret(secret: &str) -> Option<Invite> {
    let secret = secret.trim();
    if secret.len() > MAX_TEXT {
        return None;
    }
    let rest = secret.strip_prefix(SECRET_VERSION)?.strip_prefix('/')?;
    let (place, rest) = rest.split_once('/')?;
    let (code, key) = rest.split_once('/')?;
    let key = URL_SAFE_NO_PAD.decode(key).ok()?;
    let (fingerprint, token) = key.split_at_checked(32)?;
    // Through the link, so the invite gets the same checks a pasted link does.
    let service = lodestone_connect::Service::from_link(place)?;
    let link = format!(
        "{}/j/{code}#{}.{}",
        service.base_url(),
        hex(fingerprint),
        hex(token)
    );
    Invite::parse(&link).ok()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// What [`host_loop`] is told. A few a minute at most, so the size doesn't matter.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum HostCommand {
    /// The status to show, or none (closes the connection, which clears it).
    Show(Option<Activity>),
    /// Let in someone who asked to join ([`JoinRequest::user_id`]).
    Accept(String),
}

/// The Host's status: shows what it's told and hands "Ask to Join" requests to
/// `on_request`. `connect` opens a connection already subscribed to
/// [`HOST_EVENTS`]. Returns once every sender of `commands` is gone.
pub fn host_loop<S: Pipe>(
    commands: Receiver<HostCommand>,
    pid: u32,
    mut connect: impl FnMut() -> Result<Connection<S>, IpcError>,
    mut on_request: impl FnMut(JoinRequest),
) {
    let mut wanted: Option<Activity> = None;
    // What this connection shows: `None` before the first status sent on it.
    let mut shown: Option<Option<Activity>> = None;
    let mut connection: Option<Connection<S>> = None;
    let mut retry = RETRY_MIN;
    let mut next_try = Instant::now();
    let mut last_sent: Option<Instant> = None;
    loop {
        let now = Instant::now();
        let wait = if wanted.is_none() {
            None
        } else if connection.is_none() {
            Some(next_try.saturating_duration_since(now))
        } else if shown.as_ref() != Some(&wanted) {
            Some(last_sent.map_or(Duration::ZERO, |sent| {
                (sent + SEND_GAP).saturating_duration_since(now)
            }))
        } else {
            Some(POLL)
        };
        let first = match wait {
            None => match commands.recv() {
                Ok(command) => Some(command),
                Err(_) => return,
            },
            Some(wait) => match commands.recv_timeout(wait) {
                Ok(command) => Some(command),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            },
        };
        let mut accepts = Vec::new();
        for command in first.into_iter().chain(commands.try_iter()) {
            match command {
                HostCommand::Show(activity) => wanted = activity,
                HostCommand::Accept(user_id) => accepts.push(user_id),
            }
        }
        if wanted.is_none() {
            // Closing the connection clears the status.
            connection = None;
            shown = None;
            retry = RETRY_MIN;
            next_try = Instant::now();
            continue;
        }
        if connection.is_none() {
            if Instant::now() < next_try {
                continue;
            }
            match connect() {
                Ok(opened) => {
                    connection = Some(opened);
                    shown = None;
                    retry = RETRY_MIN;
                }
                Err(_) => {
                    next_try = Instant::now() + retry;
                    retry = (retry * 2).min(RETRY_MAX);
                    continue;
                }
            }
        }
        let Some(open) = connection.as_mut() else {
            continue;
        };
        let served: Result<(), IpcError> = (|| {
            for user_id in &accepts {
                open.send_join_invite(user_id)?;
            }
            let due = last_sent.is_none_or(|sent| sent.elapsed() >= SEND_GAP);
            if shown.as_ref() != Some(&wanted) && due {
                open.set_activity(pid, wanted.as_ref())?;
                shown = Some(wanted.clone());
                last_sent = Some(Instant::now());
            }
            while let Some(event) = open.poll_event()? {
                if let Event::JoinRequest(request) = event {
                    on_request(request);
                }
            }
            Ok(())
        })();
        if served.is_err() {
            connection = None;
            next_try = Instant::now() + retry;
            retry = (retry * 2).min(RETRY_MAX);
        }
    }
}

/// The launcher's side: hands each join secret Discord sends (`ACTIVITY_JOIN`) to
/// `on_join`. `connect` opens a connection subscribed to [`LAUNCHER_EVENTS`], and
/// says whether Discord confirmed it. A message on `wake` connects again at once
/// instead of after the wait (Discord started the launcher for a join while it was
/// already running). Returns once every sender of `wake` is gone and the connection
/// failed.
pub fn join_loop<S: Pipe>(
    wake: Receiver<()>,
    mut connect: impl FnMut() -> Result<(Connection<S>, bool), IpcError>,
    mut on_join: impl FnMut(String),
) {
    let mut retry = RETRY_MIN;
    loop {
        if let Ok((mut connection, confirmed)) = connect() {
            // An app that never sends events (arRPC) is let go: look again later, as
            // the official app may start meanwhile.
            if confirmed && connection.wait_without_limit().is_ok() {
                retry = RETRY_MIN;
                loop {
                    match connection.next_event() {
                        Ok(Event::Join { secret }) => on_join(secret),
                        Ok(Event::SubscribeRefused(_)) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
            }
        }
        match wake.recv_timeout(retry) {
            Err(RecvTimeoutError::Disconnected) => return,
            Ok(()) => retry = RETRY_MIN,
            Err(RecvTimeoutError::Timeout) => retry = (retry * 2).min(RETRY_MAX),
        }
        while wake.try_recv().is_ok() {}
    }
}

/// Tells Discord how to start the app for a join while it isn't running: it opens
/// `link` (`lodestone://…`), which the app is registered for already.
/// - Windows: the `discord-<client_id>` URL scheme in the user's registry, which runs
///   this executable with `link`.
/// - macOS: `~/Library/Application Support/discord/games/<client_id>.json`, whose
///   command Discord opens (only if Discord's folder is there).
///
/// Writes only what changed. Returns whether it wrote anything.
pub fn register_launch_command(client_id: &str, link: &str) -> std::io::Result<bool> {
    if client_id.is_empty() || !client_id.bytes().all(|b| b.is_ascii_digit()) {
        return Ok(false);
    }
    register(client_id, link)
}

#[cfg(windows)]
fn register(client_id: &str, link: &str) -> std::io::Result<bool> {
    let exe = std::env::current_exe()?;
    let exe = exe.display().to_string();
    let command = format!("\"{exe}\" {link}");
    let root = format!("Software\\Classes\\discord-{client_id}");
    let values = [
        (
            root.clone(),
            "",
            format!("URL:Run game {client_id} protocol"),
        ),
        (root.clone(), "URL Protocol", String::new()),
        (format!("{root}\\DefaultIcon"), "", exe),
        (format!("{root}\\shell\\open\\command"), "", command),
    ];
    let mut changed = false;
    for (key, name, value) in values {
        changed |= registry::set_string(&key, name, &value)?;
    }
    Ok(changed)
}

#[cfg(target_os = "macos")]
fn register(client_id: &str, link: &str) -> std::io::Result<bool> {
    let Some(home) = std::env::var_os("HOME") else {
        return Ok(false);
    };
    let discord = std::path::Path::new(&home).join("Library/Application Support/discord");
    if !discord.is_dir() {
        return Ok(false);
    }
    let games = discord.join("games");
    let file = games.join(format!("{client_id}.json"));
    let content = serde_json::json!({ "command": link }).to_string();
    if std::fs::read_to_string(&file).is_ok_and(|old| old == content) {
        return Ok(false);
    }
    std::fs::create_dir_all(&games)?;
    std::fs::write(&file, content)?;
    Ok(true)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn register(_client_id: &str, _link: &str) -> std::io::Result<bool> {
    Ok(false)
}

#[cfg(windows)]
mod registry {
    use std::io;

    use windows_sys::Win32::{
        Foundation::ERROR_SUCCESS,
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE,
            REG_SZ, RRF_RT_REG_SZ, RegCloseKey, RegCreateKeyExW, RegGetValueW, RegSetValueExW,
        },
    };

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Sets `name` (`""` for the default value) under `HKEY_CURRENT_USER\subkey`,
    /// making the key if needed, unless it already has `value`. Whether it changed.
    pub fn set_string(subkey: &str, name: &str, value: &str) -> io::Result<bool> {
        let subkey = wide(subkey);
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: the strings are NUL-terminated UTF-16; the key is closed below.
        let created = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_QUERY_VALUE | KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        if created != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(created as i32));
        }
        let name = wide(name);
        let result = (|| {
            if read(key, &name).as_deref() == Some(value) {
                return Ok(false);
            }
            let value = wide(value);
            // SAFETY: `value` is NUL-terminated UTF-16; its size in bytes counts the NUL.
            let set = unsafe {
                RegSetValueExW(
                    key,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    value.as_ptr().cast(),
                    (value.len() * 2) as u32,
                )
            };
            if set != ERROR_SUCCESS {
                return Err(io::Error::from_raw_os_error(set as i32));
            }
            Ok(true)
        })();
        // SAFETY: `key` was opened above.
        unsafe { RegCloseKey(key) };
        result
    }

    fn read(key: HKEY, name: &[u16]) -> Option<String> {
        let mut size = 0u32;
        // SAFETY: asks only for the size.
        let status = unsafe {
            RegGetValueW(
                key,
                std::ptr::null(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
        // SAFETY: `buffer` holds `size` bytes.
        let status = unsafe {
            RegGetValueW(
                key,
                std::ptr::null(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..len]))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use serde_json::{Value, json};

    use super::*;
    use crate::discord::ipc::{Party, Secrets, fake};

    fn invite(link: &str) -> Invite {
        Invite::parse(link).expect("an invite")
    }

    fn real_link() -> String {
        format!(
            "https://connect.lodestone.cc/j/7KQ2M9XB4D#{}.{}",
            "3f".repeat(32),
            "c0".repeat(16)
        )
    }

    #[test]
    fn the_invite_link_is_too_long_for_discord_and_the_secret_is_not() {
        let link = real_link();
        assert!(link.len() > MAX_TEXT, "{}", link.len());
        let parsed = invite(&link);
        let secret = join_secret(&parsed).expect("a secret");
        assert!(secret.len() <= MAX_TEXT, "{secret}");
        assert!(
            secret.starts_with("1/connect.lodestone.cc/7KQ2M9XB4D/"),
            "{secret}"
        );
        let back = invite_from_secret(&secret).expect("the invite again");
        assert_eq!(back.link(), link);
        // a test service, by address
        let test = invite(&format!(
            "http://127.0.0.1:18790/j/ABCDEFGH23#{}.{}",
            "01".repeat(32),
            "02".repeat(16)
        ));
        let secret = join_secret(&test).unwrap();
        assert_eq!(invite_from_secret(&secret).unwrap().link(), test.link());
    }

    #[test]
    fn secrets_that_are_not_invites_are_refused() {
        let secret = join_secret(&invite(&real_link())).unwrap();
        for bad in [
            String::new(),
            "x".into(),
            real_link(),
            secret.replacen("1/", "2/", 1),
            secret[..secret.len() - 4].to_string(),
            secret.replace("7KQ2M9XB4D", "7KQ-M9XB4D"),
            format!("{secret}{}", "A".repeat(MAX_TEXT)),
        ] {
            assert!(invite_from_secret(&bad).is_none(), "{bad}");
        }
    }

    fn hosting(players: u32) -> Activity {
        Activity {
            details: Some("Hosting Survival".into()),
            party: Party::new("instance-1", players, 20),
            secrets: Secrets::join("1/connect.lodestone.cc/X/abc"),
            ..Default::default()
        }
    }

    /// Shows the status, sends it again only when it changed, hands on requests,
    /// lets someone in, and clears it by closing the connection.
    #[test]
    fn the_host_loop_shows_changes_and_answers_requests() {
        let listener = fake::Listener::new();
        let (commands, rx) = mpsc::channel();
        let (requests_tx, requests) = mpsc::channel();
        std::thread::scope(|scope| {
            let listener = &listener;
            let worker = scope.spawn(move || {
                host_loop(
                    rx,
                    9,
                    || listener.connect("42"),
                    |r| requests_tx.send(r).unwrap(),
                )
            });
            commands.send(HostCommand::Show(Some(hosting(1)))).unwrap();
            let mut discord = listener.accept();
            let set = discord.command();
            assert_eq!(set["cmd"], "SET_ACTIVITY");
            assert_eq!(set["args"]["pid"], 9);
            assert_eq!(set["args"]["activity"]["party"]["size"], json!([1, 20]));
            discord.answer(&set, Value::Null);
            // the same status again: nothing sent
            commands.send(HostCommand::Show(Some(hosting(1)))).unwrap();
            // someone asks to join
            discord.send(json!({
                "cmd": "DISPATCH", "evt": "ACTIVITY_JOIN_REQUEST",
                "data": { "user": { "id": "7", "username": "alex" } },
            }));
            let request = requests.recv_timeout(Duration::from_secs(10)).unwrap();
            assert_eq!(request.name, "alex");
            commands.send(HostCommand::Accept(request.user_id)).unwrap();
            let invite = discord.command();
            assert_eq!(invite["cmd"], "SEND_ACTIVITY_JOIN_INVITE");
            assert_eq!(invite["args"]["user_id"], "7");
            discord.answer(&invite, Value::Null);
            // a player joined: sent again (after the gap since the last one)
            commands.send(HostCommand::Show(Some(hosting(2)))).unwrap();
            let set = discord.command();
            assert_eq!(set["args"]["activity"]["party"]["size"], json!([2, 20]));
            discord.answer(&set, Value::Null);
            // sharing stopped: the connection closes
            commands.send(HostCommand::Show(None)).unwrap();
            let (op, _) = discord.read();
            assert_eq!(op, 2, "a CLOSE frame");
            drop(commands);
            worker.join().unwrap();
        });
    }

    /// Against the Discord app running on this computer (shows nothing):
    /// `cargo test -p launcher-core live_discord -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the Discord app"]
    fn live_discord_confirms_the_subscriptions() {
        let id = crate::discord::LODESTONE_CLIENT_ID;
        for events in [LAUNCHER_EVENTS, HOST_EVENTS] {
            let (_connection, confirmed) =
                Connection::open_subscribed(id, events, CONFIRM_WAIT).expect("Discord is open");
            assert!(confirmed, "no Discord app confirmed {events:?}");
        }
    }

    #[test]
    fn the_join_loop_hands_on_secrets_and_lets_unconfirmed_apps_go() {
        let listener = fake::Listener::new();
        let (wake, rx) = mpsc::channel();
        let (joins_tx, joins) = mpsc::channel();
        std::thread::scope(|scope| {
            let listener = &listener;
            let mut attempts = 0;
            scope.spawn(move || {
                join_loop(
                    rx,
                    || {
                        attempts += 1;
                        // the first app is arRPC: not confirmed
                        listener.connect("42").map(|c| (c, attempts > 1))
                    },
                    |secret| joins_tx.send(secret).unwrap(),
                )
            });
            let arrpc = listener.accept();
            // let go, and tried again at once when woken
            wake.send(()).unwrap();
            let mut discord = listener.accept();
            drop(arrpc);
            discord.send(json!({
                "cmd": "DISPATCH", "evt": "ACTIVITY_JOIN", "data": { "secret": "1/x/Y/z" },
            }));
            assert_eq!(
                joins.recv_timeout(Duration::from_secs(10)).unwrap(),
                "1/x/Y/z"
            );
            // Discord quits; with the waker gone the loop ends
            drop(wake);
            drop(discord);
        });
    }
}
