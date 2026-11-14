//! Owner access for programs on this computer (Lodestone Launcher), proven through the
//! core's data folder instead of a password.
//!
//! Anything that can write to the data folder already controls the core: `stores/`
//! holds every user's token secret. So a program that proves it can write there may
//! have owner access, and none of the secrets cross the network:
//!
//! 1. `POST /local_claim/start` creates `local_claim/<id>/` (0700) with an empty `claim`
//!    file and returns the id, the folder (relative to the data folder, since a core in
//!    Docker sees another path than its host) and a value. To a program on this
//!    computer (a loopback address) it also says where its data folder is, so a core
//!    kept somewhere other than the default needn't be looked for. Nobody else gets the
//!    path: it names the user's folders, and it would be wrong on the other side of
//!    Docker anyway. The path is only a hint: the program still has to find the claim
//!    there, in a folder only its user can open.
//! 2. The program writes the value into `claim`, a random value of its own into `reply`
//!    and, for a core without an owner yet, `owner.json` (`{"username", "password"}`).
//!    A session claim from this computer may leave the password out (Lodestone Host's
//!    first run): the owner then has none, and the core serves only this computer until
//!    they set one (`crate::remote_access`).
//! 3. `POST /local_claim/finish` checks `claim`, creates the owner if asked, mints an
//!    owner API key and writes `result.json` (0600): the key, the program's `reply`, and
//!    the SHA-256 fingerprint of the core's certificate. A program that asks for a
//!    `session` (Lodestone Host, which runs this core as its child process) gets an owner
//!    sign-in token instead of a key, as `POST /user/login` would give the owner: it can
//!    then do what only a signed-in session may (change the password, manage API keys).
//!
//! Finding its own `reply` in `result.json` tells the program that a core read the
//! folder, but not that the server it dialed is that core: a server in between could
//! pass `start` and `finish` on to the real core and wait for the key. So the program
//! must only ever send the key over HTTPS to a server whose certificate has the
//! fingerprint in `result.json`. A core without a certificate (its TLS setup failed)
//! writes `null`, and the key should then not be used over the network at all.
//!
//! Each client address has up to [`MAX_STARTS_PER_MINUTE`] starts a minute. Up to
//! [`MAX_PENDING`] claims wait at once for each kind of address (this computer, the local
//! network, anywhere else), so a flood from the network can't push out a claim from this
//! computer; when a kind is full its oldest claim goes. A claim lasts [`CLAIM_TTL`], can
//! only be finished from the address that started it, and is used up by any failure. A
//! claim's folder is deleted [`ABANDONED_AFTER`] after it expires when it was never
//! finished (`owner.json` holds a password), and [`RESULT_TTL`] after `finish`. The
//! `allow_local_claim` setting turns it all off. Web pages can't claim at all: a request
//! with a browser's `Origin` or `Sec-Fetch-Site` header is refused, since a page in a
//! browser on this computer connects from a loopback address too.
//!
//! The file API never reads, writes, moves or deletes `local_claim/` (see
//! `handlers::global_fs`). The folders are created owner-only in one step and checked
//! before use: a real folder (not a link), owned by the core's user, with no access for
//! anyone else. On Windows, where these modes don't exist, claims are refused unless the
//! data folder is inside the user's profile folder, which only the user (and
//! administrators) can open.

use std::{
    collections::{HashMap, VecDeque},
    io::Read,
    net::IpAddr,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use axum::{
    extract::State,
    http::{header, HeaderMap},
    routing::post,
    Json, Router,
};
use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tracing::{info, warn};

use crate::{
    auth::{permission::UserPermission, user::User},
    error::{Error, ErrorKind},
    events::CausedBy,
    handlers::util::ClientIp,
    util::rand_alphanumeric,
    AppState,
};

/// The folder in the data folder that claims are made in.
pub const CLAIM_DIR: &str = "local_claim";
/// How long a claim waits for `finish`.
pub const CLAIM_TTL: Duration = Duration::from_secs(60);
/// How long after it expires the folder of a claim that was never finished is deleted.
pub const ABANDONED_AFTER: Duration = Duration::from_secs(60);
/// How long a claim's folder (and an uncollected `result.json`) stays after `finish`.
pub const RESULT_TTL: Duration = Duration::from_secs(5 * 60);
/// How many claims one client address may start in a minute.
pub const MAX_STARTS_PER_MINUTE: usize = 10;
/// How many claims may wait at once from each kind of address.
pub const MAX_PENDING: usize = 4;
/// The most client addresses whose starts are counted at once.
const MAX_TRACKED_CLIENTS: usize = 4096;
/// The largest `claim` or `reply` file read.
const MAX_SMALL_FILE: u64 = 1024;
/// The largest `owner.json` read.
const MAX_OWNER_FILE: u64 = 4096;
const ID_LEN: usize = 24;
const VALUE_LEN: usize = 48;
/// The key's name when the program's is unusable.
const DEFAULT_NAME: &str = "Lodestone Launcher";

/// The reply to `POST /local_claim/start`.
#[derive(Serialize, Deserialize, Debug)]
pub struct StartReply {
    pub id: String,
    /// `local_claim/<id>`, relative to the data folder.
    pub dir: String,
    /// What the program writes into `<dir>/claim`.
    pub value: String,
    pub core_uuid: String,
    /// Lodestone Host or the standalone core, as in `/info`.
    pub product: crate::product::CoreProduct,
    /// What started the core, as in `/info`.
    pub launched_by: crate::product::LaunchedBy,
    /// Seconds the claim waits for `finish`.
    pub expires_in: u64,
    /// The core's data folder, absolute, for a program on this computer only (a
    /// loopback address). Where the program should look for `dir` first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_folder: Option<String>,
}

/// The body of `POST /local_claim/finish`.
#[derive(Serialize, Deserialize, Debug)]
pub struct FinishRequest {
    pub id: String,
    /// The API key's name, such as "Lodestone Launcher on Peter's Mac". A claim with the
    /// same name replaces the key the last one made.
    pub name: String,
    /// Asks for an owner sign-in token (a session, as `POST /user/login` gives) instead
    /// of an API key. No key is made then.
    #[serde(default)]
    pub session: bool,
}

/// `<dir>/result.json`, written by `finish`. Holds the key, so it is never sent over the
/// network and the file is readable only by the core's OS user.
#[derive(Serialize, Deserialize)]
pub struct ClaimResult {
    /// The owner API key; `None` for a claim that asked for a session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// The owner's sign-in token, for a claim that asked for a session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_token: Option<String>,
    /// The program's `reply`, read from the folder.
    pub reply: String,
    pub owner_created: bool,
    pub owner_username: String,
    /// SHA-256 of the core's certificate, as colon-separated uppercase hex, as in `/info`.
    /// The key may only be sent over HTTPS to a server with this certificate. `null` when
    /// the core has none.
    pub tls_fingerprint_sha256: Option<String>,
}

/// `<dir>/owner.json`, written by the program for a core without an owner. Without a
/// password (`null` or left out), the owner has none: only for a session claim from this
/// computer (Lodestone Host's first run), and the core then serves only this computer
/// until the owner sets one (see `crate::remote_access`).
#[derive(Deserialize)]
struct OwnerFile {
    username: String,
    #[serde(default)]
    password: Option<String>,
}

/// The kinds of client address, each with its own waiting claims.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Origin {
    /// This computer.
    Loopback,
    /// A private, link-local or unique local address: the local network, or a Docker
    /// host reaching a core in a container.
    Private,
    /// Anything else, or an address the server didn't record.
    Other,
}

impl Origin {
    fn of(ip: Option<IpAddr>) -> Self {
        match ip {
            Some(ip) if ip.is_loopback() => Self::Loopback,
            Some(IpAddr::V4(v4)) if v4.is_private() || v4.is_link_local() => Self::Private,
            // fc00::/7 (unique local) and fe80::/10 (link-local)
            Some(IpAddr::V6(v6))
                if (v6.segments()[0] & 0xfe00) == 0xfc00
                    || (v6.segments()[0] & 0xffc0) == 0xfe80 =>
            {
                Self::Private
            }
            _ => Self::Other,
        }
    }
}

struct Pending {
    id: String,
    value: String,
    started: Instant,
    ip: Option<IpAddr>,
    origin: Origin,
}

#[derive(Default)]
struct ClaimState {
    /// Oldest first.
    pending: Vec<Pending>,
    /// When each client address started its recent claims, oldest first.
    starts: HashMap<Option<IpAddr>, VecDeque<Instant>>,
}

/// The claims in progress, and the folder claims are made in.
pub struct LocalClaims {
    root: PathBuf,
    dir: PathBuf,
    state: tokio::sync::Mutex<ClaimState>,
}

fn denied(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::PermissionDenied,
        source: eyre!(message.into()),
    }
}

impl LocalClaims {
    /// Claims under `<data_dir>/local_claim`.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            root: data_dir.to_path_buf(),
            dir: data_dir.join(CLAIM_DIR),
            state: Default::default(),
        }
    }

    /// Removes whatever claims a previous run left. Call it at startup, before serving.
    pub async fn clean_up(&self) {
        match tokio::fs::symlink_metadata(&self.dir).await {
            Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_file() => {
                let _ = tokio::fs::remove_file(&self.dir).await;
            }
            Ok(_) => {
                if let Err(e) = tokio::fs::remove_dir_all(&self.dir).await {
                    warn!("Failed to remove {}: {e}", self.dir.display());
                }
            }
            Err(_) => {}
        }
    }

    fn claim_dir(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    async fn remove_claim(&self, id: &str) {
        let dir = self.claim_dir(id);
        if let Err(e) = tokio::fs::remove_dir_all(&dir).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!("Failed to remove {}: {e}", dir.display());
            }
        }
    }

    /// Removes claim folders no waiting claim owns: those without `result.json` once
    /// [`CLAIM_TTL`] plus [`ABANDONED_AFTER`] have passed, the rest after [`RESULT_TTL`].
    async fn sweep(&self, waiting: &[Pending]) {
        let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await else {
            return;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().into_owned();
            if waiting.iter().any(|p| p.id == name) {
                continue;
            }
            let path = entry.path();
            let finished = tokio::fs::symlink_metadata(path.join("result.json"))
                .await
                .is_ok();
            let limit = if finished {
                RESULT_TTL
            } else {
                CLAIM_TTL + ABANDONED_AFTER
            };
            let old = entry
                .metadata()
                .await
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_none_or(|age| age >= limit);
            if old {
                let _ = tokio::fs::remove_dir_all(&path).await;
            }
        }
    }

    /// Counts a start from `ip`; refuses it when the address has used its allowance.
    fn count_start(state: &mut ClaimState, ip: Option<IpAddr>, now: Instant) -> Result<(), Error> {
        let minute = Duration::from_secs(60);
        if state.starts.len() >= MAX_TRACKED_CLIENTS {
            state.starts.retain(|_, times| {
                times
                    .back()
                    .is_some_and(|t| now.duration_since(*t) < minute)
            });
        }
        if state.starts.len() >= MAX_TRACKED_CLIENTS && !state.starts.contains_key(&ip) {
            return Err(Error {
                kind: ErrorKind::TooManyRequests,
                source: eyre!("Too many claims started. Try again in a minute"),
            });
        }
        let times = state.starts.entry(ip).or_default();
        while times
            .front()
            .is_some_and(|t| now.duration_since(*t) >= minute)
        {
            times.pop_front();
        }
        if times.len() >= MAX_STARTS_PER_MINUTE {
            return Err(Error {
                kind: ErrorKind::TooManyRequests,
                source: eyre!("Too many claims started. Try again in a minute"),
            });
        }
        times.push_back(now);
        Ok(())
    }

    /// Starts a claim for the client at `ip`: creates its folder and empty `claim`.
    pub async fn start(
        &self,
        ip: Option<IpAddr>,
        core_uuid: &str,
        product: crate::product::CoreProduct,
        launched_by: crate::product::LaunchedBy,
    ) -> Result<StartReply, Error> {
        check_platform(&self.root)?;
        let mut state = self.state.lock().await;
        let now = Instant::now();
        Self::count_start(&mut state, ip, now)?;

        // expired claims stop waiting (their folders go with the sweep or their timer)
        state
            .pending
            .retain(|p| now.duration_since(p.started) < CLAIM_TTL);
        let origin = Origin::of(ip);
        if state.pending.iter().filter(|p| p.origin == origin).count() >= MAX_PENDING {
            if let Some(oldest) = state.pending.iter().position(|p| p.origin == origin) {
                let old = state.pending.remove(oldest);
                self.remove_claim(&old.id).await;
            }
        }
        self.sweep(&state.pending).await;

        let id = rand_alphanumeric(ID_LEN);
        let value = rand_alphanumeric(VALUE_LEN);
        let dir = self.claim_dir(&id);
        ensure_private_dir(&self.dir)?;
        ensure_private_dir(&dir)?;
        create_private_file(&dir.join("claim"))?;
        state.pending.push(Pending {
            id: id.clone(),
            value: value.clone(),
            started: now,
            ip,
            origin,
        });
        drop(state);

        // a claim never finished goes, with any owner.json, once it is well past its time
        let abandoned = dir.clone();
        tokio::spawn(async move {
            tokio::time::sleep(CLAIM_TTL + ABANDONED_AFTER).await;
            if tokio::fs::symlink_metadata(abandoned.join("result.json"))
                .await
                .is_err()
            {
                let _ = tokio::fs::remove_dir_all(&abandoned).await;
            }
        });
        info!("A program on this computer started claiming owner access");
        let data_folder = (origin == Origin::Loopback)
            .then(|| self.absolute_root())
            .flatten();
        Ok(StartReply {
            dir: format!("{CLAIM_DIR}/{id}"),
            id,
            value,
            core_uuid: core_uuid.to_string(),
            product,
            launched_by,
            expires_in: CLAIM_TTL.as_secs(),
            data_folder,
        })
    }

    /// The data folder as an absolute path, when it is one (not every path is text).
    fn absolute_root(&self) -> Option<String> {
        let path = std::fs::canonicalize(&self.root)
            .or_else(|_| std::path::absolute(&self.root))
            .ok()?;
        path.to_str().map(str::to_string)
    }

    /// Checks the claim `id` from the client at `ip` and, when it holds, writes the owner
    /// key into its folder.
    pub async fn finish(
        &self,
        app: &AppState,
        ip: Option<IpAddr>,
        request: FinishRequest,
    ) -> Result<(), Error> {
        let mut state = self.state.lock().await;
        // An id that isn't a waiting claim's, or a claim another address started, leaves
        // the claims alone: otherwise anyone could cancel a claim in progress.
        let found = state
            .pending
            .iter()
            .position(|p| bool::from(p.id.as_bytes().ct_eq(request.id.as_bytes())) && p.ip == ip);
        let Some(index) = found else {
            return Err(denied("No such claim. Start a new one"));
        };
        let pending = state.pending.remove(index);
        drop(state);
        let dir = self.claim_dir(&pending.id);
        let result = self
            .check_and_grant(app, &pending, &dir, &request.name, request.session)
            .await;
        match &result {
            Ok(()) => {
                // the folder, with the result, goes after a while if it isn't collected
                let dir = dir.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(RESULT_TTL).await;
                    let _ = tokio::fs::remove_dir_all(&dir).await;
                });
            }
            Err(e) => {
                warn!("A claim of owner access was refused: {}", e.source);
                self.remove_claim(&pending.id).await;
            }
        }
        result
    }

    async fn check_and_grant(
        &self,
        app: &AppState,
        pending: &Pending,
        dir: &Path,
        name: &str,
        session: bool,
    ) -> Result<(), Error> {
        if pending.started.elapsed() >= CLAIM_TTL {
            return Err(denied("The claim has expired. Start a new one"));
        }
        // the folders must still be the private ones made at the start
        verify_private_dir(&self.dir)?;
        verify_private_dir(dir)?;
        let claim = read_small(dir.join("claim"), MAX_SMALL_FILE)
            .await?
            .ok_or_else(|| denied("The claim file is missing"))?;
        if !bool::from(claim.trim().as_bytes().ct_eq(pending.value.as_bytes())) {
            return Err(denied("The claim file doesn't hold the claim's value"));
        }
        let reply = read_small(dir.join("reply"), MAX_SMALL_FILE)
            .await?
            .ok_or_else(|| denied("The reply file is missing"))?;
        let reply = reply.trim().to_string();
        if !(16..=256).contains(&reply.len()) || !reply.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(denied("The reply must be 16 to 256 letters and digits"));
        }
        let owner_file = read_small(dir.join("owner.json"), MAX_OWNER_FILE).await?;

        let (owner, owner_created) = match current_owner(app).await {
            Some(owner) => (owner, false),
            None => {
                let Some(text) = owner_file else {
                    return Err(denied(
                        "This core has no owner yet: write owner.json with the owner's \
                         username and password",
                    ));
                };
                let passwordless_allowed = session && pending.origin == Origin::Loopback;
                (create_owner(app, &text, passwordless_allowed).await?, true)
            }
        };
        let name = key_name(name);
        let (api_key, session_token, replaced) = if session {
            let token = owner.create_jwt()?;
            (None, Some(token.to_string()), 0)
        } else {
            let (created, replaced) = app
                .users_manager
                .write()
                .await
                .replace_claim_key(&owner.uid, name.clone())
                .await?;
            (Some(created.key), None, replaced)
        };
        let result = ClaimResult {
            api_key,
            session_token,
            reply,
            owner_created,
            owner_username: owner.username.clone(),
            tls_fingerprint_sha256: app.tls_info.fingerprint_sha256.clone(),
        };
        let data = serde_json::to_vec(&result).context("Failed to serialize the claim's result")?;
        crate::persist::write_atomic(dir.join("result.json"), data, Some(0o600))
            .await
            .context("Failed to write the claim's result")?;
        for file in ["claim", "reply", "owner.json"] {
            let _ = tokio::fs::remove_file(dir.join(file)).await;
        }
        if owner_created {
            info!(
                "{name} created the owner account {} through the data folder",
                owner.username
            );
        }
        if session {
            info!("{name} was signed in as the owner through the data folder");
        } else {
            info!(
                "{name} was given owner access through the data folder (API key `{name}`{})",
                if replaced > 0 {
                    ", replacing its earlier key"
                } else {
                    ""
                }
            );
        }
        Ok(())
    }
}

/// The owner, if the core has one.
async fn current_owner(app: &AppState) -> Option<User> {
    app.users_manager
        .read()
        .await
        .as_ref()
        .values()
        .find(|user| user.is_owner)
        .cloned()
}

/// Creates the owner from `owner.json`, as `POST /setup/{key}` does, using up the
/// first-time setup key. Without a password only when `passwordless_allowed`. Errors
/// never repeat the file's contents.
async fn create_owner(
    app: &AppState,
    text: &str,
    passwordless_allowed: bool,
) -> Result<User, Error> {
    let owner: OwnerFile =
        serde_json::from_str(text).map_err(|_| denied("owner.json isn't valid"))?;
    let username = crate::auth::user::valid_username(&owner.username).map_err(denied)?;
    match &owner.password {
        Some(password) => crate::remote_access::owner_password(password).map_err(denied)?,
        None if passwordless_allowed => {}
        None => return Err(denied("owner.json must have the owner's password")),
    }
    // held until the owner is saved, as in setup_owner
    let mut setup_key = app.first_time_setup_key.lock().await;
    if current_owner(app).await.is_some() {
        return Err(denied("This core already has an owner"));
    }
    let user = match owner.password {
        Some(password) => tokio::task::spawn_blocking(move || {
            User::new(username, &password, true, false, UserPermission::default())
        })
        .await
        .context("Password hashing task failed")?,
        None => User::owner_without_password(username),
    };
    app.users_manager
        .write()
        .await
        .add_user(user.clone(), CausedBy::System)
        .await?;
    setup_key.take();
    drop(setup_key);
    crate::remote_access::owner_created(app, user.has_password()).await;
    Ok(user)
}

/// The key's name: control characters dropped, at most 64 characters.
fn key_name(name: &str) -> String {
    let name: String = name
        .chars()
        .filter(|c| !c.is_control())
        .take(crate::auth::api_key::MAX_API_KEY_NAME_CHARS)
        .collect();
    let name = name.trim();
    if name.is_empty() {
        DEFAULT_NAME.to_string()
    } else {
        name.to_string()
    }
}

/// The text of `path`, `None` if it doesn't exist. Opened without following a symbolic
/// link (and without waiting on a FIFO), then checked through the open handle: a regular
/// file of at most `max` bytes, in UTF-8. Errors name the file, never its contents.
async fn read_small(path: PathBuf, max: u64) -> Result<Option<String>, Error> {
    tokio::task::spawn_blocking(move || read_small_blocking(&path, max))
        .await
        .context("Reading a claim file failed")?
}

fn read_small_blocking(path: &Path, max: u64) -> Result<Option<String>, Error> {
    let name = file_name(path);
    let file = match open_no_follow(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(denied(format!("{name} can't be opened as a plain file"))),
    };
    let metadata = file
        .metadata()
        .map_err(|_| denied(format!("{name} can't be read")))?;
    if !metadata.file_type().is_file() {
        return Err(denied(format!("{name} isn't a plain file")));
    }
    if metadata.len() > max {
        return Err(denied(format!("{name} is too large")));
    }
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| denied(format!("{name} can't be read")))?;
    if bytes.len() as u64 > max {
        return Err(denied(format!("{name} is too large")));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| denied(format!("{name} isn't text")))
}

/// Opens `path` for reading without following a final symbolic link: `O_NOFOLLOW` (an
/// error for a link) and `O_NONBLOCK` (so a FIFO can't hold the request up) on Unix, the
/// link itself on Windows (`FILE_FLAG_OPEN_REPARSE_POINT`), which then isn't a file.
fn open_no_follow(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_OPEN_REPARSE_POINT
        options.custom_flags(0x0020_0000);
    }
    options.open(path)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Refuses claims where the folders can't be made private. On Windows that is a data
/// folder outside the user's profile folder (`%USERPROFILE%`), whose permissions only the
/// user and administrators have; elsewhere the folders' modes do it.
fn check_platform(root: &Path) -> Result<(), Error> {
    #[cfg(windows)]
    {
        let inside = std::env::var_os("USERPROFILE")
            .and_then(|profile| std::fs::canonicalize(profile).ok())
            .zip(std::fs::canonicalize(root).ok())
            .is_some_and(|(profile, root)| root.starts_with(profile));
        if !inside {
            return Err(denied(
                "On Windows, owner access can only be claimed when the core's data folder is \
                 inside your user folder, where only your account can read it. Sign in with \
                 a password instead",
            ));
        }
    }
    #[cfg(not(windows))]
    let _ = root;
    Ok(())
}

/// Creates `dir` owner-only in one step (mode 0700 on Unix), or checks the existing one:
/// a real folder (a link in its place is removed), owned by the core's user (otherwise
/// refused), with no access for anyone else (otherwise narrowed).
pub(crate) fn ensure_private_dir(dir: &Path) -> Result<(), Error> {
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = std::fs::symlink_metadata(dir)
                .map_err(|e| internal(format!("Failed to check {}: {e}", dir.display())))?;
            if !metadata.is_dir() {
                // a link or file in the folder's place: replace it with a real folder
                std::fs::remove_file(dir)
                    .map_err(|e| internal(format!("Failed to remove {}: {e}", dir.display())))?;
                builder
                    .create(dir)
                    .map_err(|e| internal(format!("Failed to create {}: {e}", dir.display())))?;
            }
        }
        Err(e) => return Err(internal(format!("Failed to create {}: {e}", dir.display()))),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = std::fs::symlink_metadata(dir)
            .map_err(|e| internal(format!("Failed to check {}: {e}", dir.display())))?;
        if metadata.uid() == effective_uid() && metadata.mode() & 0o077 != 0 {
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| internal(format!("Failed to restrict {}: {e}", dir.display())))?;
        }
    }
    verify_private_dir(dir)
}

/// Checks that `dir` is a real folder owned by the core's user that nobody else can open.
fn verify_private_dir(dir: &Path) -> Result<(), Error> {
    let metadata = std::fs::symlink_metadata(dir)
        .map_err(|_| denied(format!("{} is missing", file_name(dir))))?;
    if !metadata.is_dir() {
        return Err(denied(format!("{} isn't a folder", file_name(dir))));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != effective_uid() {
            return Err(denied(format!(
                "{} belongs to another user",
                file_name(dir)
            )));
        }
        if metadata.mode() & 0o077 != 0 {
            return Err(denied(format!(
                "{} can be opened by other users",
                file_name(dir)
            )));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn effective_uid() -> u32 {
    // SAFETY: geteuid takes no arguments, can't fail and touches no memory.
    unsafe { libc::geteuid() }
}

fn internal(message: String) -> Error {
    Error {
        kind: ErrorKind::Internal,
        source: eyre!(message),
    }
}

/// Creates the new file `path`, empty and readable only by the core's OS user. Fails if
/// anything (a link included) is already there.
fn create_private_file(path: &Path) -> Result<(), Error> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map(|_| ())
        .map_err(|e| internal(format!("Failed to create {}: {e}", path.display())))
}

/// Refuses a request a web page sent. The core allows every origin (CORS), and a page in
/// a browser on this computer connects from a loopback address, so without this any site
/// could read the data folder's path from `start` and use up this computer's claims.
/// Browsers send `Origin` with every POST and `Sec-Fetch-Site` with every request (`none`
/// only for one the person made themselves, such as a typed address), and no page can
/// leave them out; Lodestone's own programs send neither.
fn refuse_web_pages(headers: &HeaderMap) -> Result<(), Error> {
    let from_page = headers.contains_key(header::ORIGIN)
        || headers
            .get("sec-fetch-site")
            .is_some_and(|site| site.as_bytes() != b"none");
    if from_page {
        return Err(denied(
            "Owner access can't be claimed from a web page. Use Lodestone Host or \
             Lodestone Launcher on this computer",
        ));
    }
    Ok(())
}

async fn start_claim(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
) -> Result<Json<StartReply>, Error> {
    refuse_web_pages(&headers)?;
    if !state.global_settings.lock().await.allow_local_claim() {
        return Err(denied(
            "Claiming owner access from this computer is turned off in Core Settings",
        ));
    }
    Ok(Json(
        state
            .local_claims
            .start(ip, &state.uuid, state.product, state.launched_by)
            .await?,
    ))
}

async fn finish_claim(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    Json(request): Json<FinishRequest>,
) -> Result<Json<serde_json::Value>, Error> {
    refuse_web_pages(&headers)?;
    if !state.global_settings.lock().await.allow_local_claim() {
        return Err(denied(
            "Claiming owner access from this computer is turned off in Core Settings",
        ));
    }
    state.local_claims.finish(&state, ip, request).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// `POST /local_claim/start` and `POST /local_claim/finish`. No sign-in: the proof is the
/// data folder. Never for web pages (`refuse_web_pages`).
pub fn get_local_claim_routes(state: AppState) -> Router {
    Router::new()
        .route("/local_claim/start", post(start_claim))
        .route("/local_claim/finish", post(finish_claim))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        global_settings::GlobalSettingsData,
        product::{CoreProduct, LaunchedBy},
        test_util::{serve_api, test_app_state},
    };
    use reqwest::StatusCode;
    use serde_json::{json, Value};
    use std::net::Ipv4Addr;

    const NAME: &str = "Lodestone Launcher on Test\u{7}Mac";
    const LOOPBACK: Option<IpAddr> = Some(IpAddr::V4(Ipv4Addr::LOCALHOST));

    struct Core {
        dir: tempfile::TempDir,
        state: AppState,
        base: String,
        client: reqwest::Client,
    }

    async fn core_with(settings: GlobalSettingsData, owner: bool) -> Core {
        crate::test_util::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(dir.path(), settings).await;
        state.tls_info = std::sync::Arc::new(crate::tls::TlsInfo {
            mode: crate::tls::TlsMode::None,
            fingerprint_sha256: Some("AB:CD".to_string()),
        });
        if owner {
            let user = User::new(
                "boss".to_string(),
                "a long enough password",
                true,
                false,
                Default::default(),
            );
            state
                .users_manager
                .write()
                .await
                .add_user(user, CausedBy::System)
                .await
                .unwrap();
        } else {
            *state.first_time_setup_key.lock().await = Some("setup".to_string());
        }
        let base = serve_api(state.clone()).await;
        Core {
            dir,
            state,
            base,
            client: reqwest::Client::new(),
        }
    }

    async fn core(owner: bool) -> Core {
        core_with(GlobalSettingsData::default(), owner).await
    }

    impl Core {
        async fn post(&self, path: &str, body: Value) -> (StatusCode, Value) {
            let response = self
                .client
                .post(format!("{}{path}", self.base))
                .json(&body)
                .send()
                .await
                .unwrap();
            let status = response.status();
            let text = response.text().await.unwrap();
            (status, serde_json::from_str(&text).unwrap_or(Value::Null))
        }

        async fn start(&self) -> StartReply {
            let (status, body) = self.post("/local_claim/start", json!({})).await;
            assert_eq!(status, StatusCode::OK, "{body}");
            serde_json::from_value(body).unwrap()
        }

        fn folder(&self, start: &StartReply) -> PathBuf {
            self.dir.path().join(&start.dir)
        }

        /// Does what the launcher does: writes the value and a reply.
        fn answer(&self, start: &StartReply, value: &str) {
            let folder = self.folder(start);
            std::fs::write(folder.join("claim"), value).unwrap();
            std::fs::write(folder.join("reply"), "ReplyFromTheLauncher123").unwrap();
        }

        async fn finish(&self, id: &str) -> (StatusCode, Value) {
            self.post("/local_claim/finish", json!({ "id": id, "name": NAME }))
                .await
        }

        fn result(&self, start: &StartReply) -> ClaimResult {
            let text = std::fs::read_to_string(self.folder(start).join("result.json")).unwrap();
            serde_json::from_str(&text).unwrap()
        }

        /// A whole claim through the API, as the launcher makes it.
        async fn claim(&self) -> ClaimResult {
            let start = self.start().await;
            self.answer(&start, &start.value);
            assert_eq!(self.finish(&start.id).await.0, StatusCode::OK);
            self.result(&start)
        }

        async fn start_from(&self, ip: IpAddr) -> Result<StartReply, Error> {
            self.state
                .local_claims
                .start(Some(ip), "uuid", CoreProduct::Core, LaunchedBy::Direct)
                .await
        }
    }

    #[cfg(unix)]
    fn mode(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[tokio::test]
    async fn an_existing_core_gives_an_owner_key_through_the_folder() {
        let core = core(true).await;
        let start = core.start().await;
        assert_eq!(start.id.len(), ID_LEN);
        assert_eq!(start.value.len(), VALUE_LEN);
        assert_eq!(start.dir, format!("local_claim/{}", start.id));
        assert_eq!(start.core_uuid, core.state.uuid);
        assert_eq!(start.product, CoreProduct::Core);
        assert_eq!(start.launched_by, LaunchedBy::Direct);
        assert_eq!(start.expires_in, 60);
        let folder = core.folder(&start);
        assert_eq!(std::fs::read(folder.join("claim")).unwrap(), b"");
        #[cfg(unix)]
        {
            assert_eq!(mode(&folder), 0o700);
            assert_eq!(mode(folder.parent().unwrap()), 0o700);
            assert_eq!(mode(&folder.join("claim")), 0o600);
        }

        core.answer(&start, &format!("{}\n", start.value));
        // an owner.json is ignored when the core has an owner
        std::fs::write(
            folder.join("owner.json"),
            r#"{"username":"owner_abcde","password":"another long password"}"#,
        )
        .unwrap();
        let (status, body) = core.finish(&start.id).await;
        assert_eq!(status, StatusCode::OK);
        // nothing secret over the network
        assert_eq!(body, json!({ "ok": true }));

        let result = core.result(&start);
        assert_eq!(result.reply, "ReplyFromTheLauncher123");
        assert!(!result.owner_created);
        assert_eq!(result.owner_username, "boss");
        // what the launcher pins the core's HTTPS to before sending the key anywhere
        assert_eq!(result.tls_fingerprint_sha256.as_deref(), Some("AB:CD"));
        #[cfg(unix)]
        assert_eq!(mode(&folder.join("result.json")), 0o600);
        for gone in ["claim", "reply", "owner.json"] {
            assert!(!folder.join(gone).exists(), "{gone}");
        }
        let users = core.state.users_manager.read().await;
        let user = users.try_auth(result.api_key.as_deref().unwrap()).unwrap();
        assert!(user.is_owner);
        assert_eq!(user.username, "boss");
        let keys = users.list_api_keys(&user.uid);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "Lodestone Launcher on TestMac");
        assert_eq!(keys[0].expires_at, None);
        assert_eq!(keys[0].scope, crate::auth::api_key::ApiKeyScope::Full);
        // only one owner
        assert_eq!(users.as_ref().values().filter(|u| u.is_owner).count(), 1);
    }

    #[tokio::test]
    async fn a_new_core_gets_its_owner_from_the_folder() {
        let core = core(false).await;
        let start = core.start().await;
        core.answer(&start, &start.value);
        std::fs::write(
            core.folder(&start).join("owner.json"),
            r#"{"username":"owner_x7k2q","password":"Zq8-strong-generated-pw"}"#,
        )
        .unwrap();
        assert_eq!(core.finish(&start.id).await.0, StatusCode::OK);
        let result = core.result(&start);
        assert!(result.owner_created);
        assert_eq!(result.owner_username, "owner_x7k2q");
        assert!(core.state.first_time_setup_key.lock().await.is_none());
        let users = core.state.users_manager.read().await;
        let owner = users.get_user_by_username("owner_x7k2q").unwrap();
        assert!(owner.is_owner);
        assert!(
            users
                .try_auth(result.api_key.as_deref().unwrap())
                .unwrap()
                .is_owner
        );
    }

    /// Lodestone Host asks for a session: an owner sign-in token, and no API key.
    #[tokio::test]
    async fn a_session_claim_signs_in_the_owner_without_a_key() {
        for owner in [true, false] {
            let core = core(owner).await;
            let start = core.start().await;
            core.answer(&start, &start.value);
            if !owner {
                std::fs::write(
                    core.folder(&start).join("owner.json"),
                    r#"{"username":"host_owner","password":"Zq8-strong-chosen-pw"}"#,
                )
                .unwrap();
            }
            let (status, body) = core
                .post(
                    "/local_claim/finish",
                    json!({ "id": start.id, "name": "Lodestone Host", "session": true }),
                )
                .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            assert_eq!(body, json!({ "ok": true }));
            let result = core.result(&start);
            assert!(result.api_key.is_none());
            assert_eq!(result.owner_created, !owner);
            let token = result.session_token.expect("a session token");
            #[cfg(unix)]
            assert_eq!(mode(&core.folder(&start).join("result.json")), 0o600);

            let users = core.state.users_manager.read().await;
            let user = users.try_auth(&token).unwrap();
            assert!(user.is_owner);
            assert!(user.require_session().is_ok(), "a session, not a key");
            assert!(users.list_api_keys(&user.uid).is_empty(), "no key made");
            drop(users);

            // what only a session may do works with it
            let response = core
                .client
                .get(format!("{}/user/api_keys", core.base))
                .bearer_auth(&token)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    /// Lodestone Host's first run: the owner without a password, and the core only for
    /// this computer from then on.
    #[tokio::test]
    async fn a_session_claim_makes_an_owner_without_a_password() {
        let core = core(false).await;
        assert!(core.state.remote_access.allowed());
        let start = core.start().await;
        core.answer(&start, &start.value);
        std::fs::write(
            core.folder(&start).join("owner.json"),
            r#"{"username":" peter ","password":null}"#,
        )
        .unwrap();
        let (status, body) = core
            .post(
                "/local_claim/finish",
                json!({ "id": start.id, "name": "Lodestone Host", "session": true }),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let result = core.result(&start);
        assert!(result.owner_created);
        assert_eq!(result.owner_username, "peter");
        let token = result.session_token.unwrap();
        assert!(core.state.first_time_setup_key.lock().await.is_none());

        let users = core.state.users_manager.read().await;
        let owner = users.try_auth(&token).unwrap();
        assert!(owner.is_owner);
        assert!(!owner.has_password());
        assert!(users.login("peter", "").is_err());
        assert!(users.login("peter", "any password at all").is_err());
        drop(users);
        // explicit and saved: only this computer from now on
        assert!(!core.state.remote_access.allowed());
        let stored: Value = serde_json::from_slice(
            &std::fs::read(core.dir.path().join("global_settings.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(stored["allow_remote_access"], false);
        // a later claim signs in the same owner
        let again = core.claim().await;
        assert!(!again.owner_created);
        assert_eq!(again.owner_username, "peter");
    }

    /// Only a session claim may leave the password out: a launcher's claim (a key) can't
    /// make a core nobody can sign in to with a password.
    #[tokio::test]
    async fn a_key_claim_must_give_the_owners_password() {
        let core = core(false).await;
        let start = core.start().await;
        core.answer(&start, &start.value);
        std::fs::write(
            core.folder(&start).join("owner.json"),
            r#"{"username":"peter"}"#,
        )
        .unwrap();
        assert_eq!(core.finish(&start.id).await.0, StatusCode::FORBIDDEN);
        assert!(core.state.first_time_setup_key.lock().await.is_some());
        assert!(core.state.remote_access.allowed());
    }

    /// An owner made with a password (an older Host) serves other devices, as before,
    /// even on a core that started local-only.
    #[tokio::test]
    async fn an_owner_with_a_password_lets_other_devices_in() {
        let core = core_with(
            GlobalSettingsData {
                allow_remote_access: false,
                ..Default::default()
            },
            false,
        )
        .await;
        let start = core.start().await;
        core.answer(&start, &start.value);
        std::fs::write(
            core.folder(&start).join("owner.json"),
            r#"{"username":"host_owner","password":"Zq8-strong-chosen-pw"}"#,
        )
        .unwrap();
        let (status, _) = core
            .post(
                "/local_claim/finish",
                json!({ "id": start.id, "name": "Lodestone Host", "session": true }),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert!(core.state.remote_access.allowed());
    }

    /// A launcher's claim (no `session`) still gets a key and no token.
    #[tokio::test]
    async fn a_key_claim_gets_no_session_token() {
        let core = core(true).await;
        let result = core.claim().await;
        assert!(result.api_key.is_some());
        assert!(result.session_token.is_none());
        let text = serde_json::to_value(&result).unwrap();
        assert!(text.get("session_token").is_none());
    }

    #[tokio::test]
    async fn a_new_core_without_owner_json_refuses() {
        let core = core(false).await;
        let start = core.start().await;
        core.answer(&start, &start.value);
        assert_eq!(core.finish(&start.id).await.0, StatusCode::FORBIDDEN);
        assert!(!core.folder(&start).exists());
    }

    #[tokio::test]
    async fn owner_json_problems_never_echo_its_contents() {
        let core = core(false).await;
        let start = core.start().await;
        core.answer(&start, &start.value);
        std::fs::write(
            core.folder(&start).join("owner.json"),
            r#"{"username":"owner_x7k2q","password":SECRETVALUE}"#,
        )
        .unwrap();
        let (status, body) = core.finish(&start.id).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let text = body.to_string();
        assert!(text.contains("owner.json isn't valid"), "{text}");
        assert!(!text.contains("SECRETVALUE"), "{text}");
        assert!(core.state.first_time_setup_key.lock().await.is_some());
    }

    #[tokio::test]
    async fn a_weak_owner_password_is_refused() {
        let core = core(false).await;
        let start = core.start().await;
        core.answer(&start, &start.value);
        std::fs::write(
            core.folder(&start).join("owner.json"),
            r#"{"username":"owner_x7k2q","password":"short"}"#,
        )
        .unwrap();
        assert_eq!(core.finish(&start.id).await.0, StatusCode::FORBIDDEN);
        assert!(core.state.first_time_setup_key.lock().await.is_some());
    }

    #[tokio::test]
    async fn a_wrong_value_is_refused_and_uses_the_claim_up() {
        let core = core(true).await;
        let start = core.start().await;
        core.answer(&start, "not the value");
        assert_eq!(core.finish(&start.id).await.0, StatusCode::FORBIDDEN);
        assert!(!core.folder(&start).exists());
        // used up: the right value doesn't help now
        assert_eq!(core.finish(&start.id).await.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn an_untouched_claim_file_is_refused() {
        let core = core(true).await;
        let start = core.start().await;
        std::fs::write(core.folder(&start).join("reply"), "ReplyFromTheLauncher123").unwrap();
        assert_eq!(core.finish(&start.id).await.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_bad_reply_is_refused() {
        let core = core(true).await;
        let start = core.start().await;
        std::fs::write(core.folder(&start).join("claim"), &start.value).unwrap();
        std::fs::write(core.folder(&start).join("reply"), "short").unwrap();
        assert_eq!(core.finish(&start.id).await.0, StatusCode::FORBIDDEN);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlinked_claim_files_are_refused() {
        for file in ["claim", "reply", "owner.json"] {
            let core = core(false).await;
            let start = core.start().await;
            core.answer(&start, &start.value);
            let folder = core.folder(&start);
            let elsewhere = core.dir.path().join("elsewhere");
            let contents = match file {
                "claim" => start.value.clone(),
                "reply" => "ReplyFromTheLauncher123".to_string(),
                _ => {
                    r#"{"username":"owner_x7k2q","password":"Zq8-strong-generated-pw"}"#.to_string()
                }
            };
            std::fs::write(&elsewhere, contents).unwrap();
            let _ = std::fs::remove_file(folder.join(file));
            std::os::unix::fs::symlink(&elsewhere, folder.join(file)).unwrap();
            assert_eq!(
                core.finish(&start.id).await.0,
                StatusCode::FORBIDDEN,
                "{file}"
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_fifo_is_refused_without_hanging() {
        let core = core(true).await;
        let start = core.start().await;
        let folder = core.folder(&start);
        std::fs::remove_file(folder.join("claim")).unwrap();
        let path =
            std::ffi::CString::new(folder.join("claim").to_string_lossy().as_bytes().to_vec())
                .unwrap();
        // SAFETY: a valid C string and a plain mode.
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        std::fs::write(folder.join("reply"), "ReplyFromTheLauncher123").unwrap();
        let finished = tokio::time::timeout(Duration::from_secs(10), core.finish(&start.id)).await;
        assert_eq!(finished.unwrap().0, StatusCode::FORBIDDEN);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_claim_folder_must_stay_private() {
        let core = core(true).await;
        let start = core.start().await;
        core.answer(&start, &start.value);
        // opened up to others after the start
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(core.folder(&start), std::fs::Permissions::from_mode(0o755))
            .unwrap();
        assert_eq!(core.finish(&start.id).await.0, StatusCode::FORBIDDEN);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_planted_claim_folder_is_made_private_or_replaced() {
        use std::os::unix::fs::PermissionsExt;
        // a world-readable local_claim/ of the core's own user is narrowed
        let core = core(true).await;
        let claims = core.dir.path().join(CLAIM_DIR);
        std::fs::create_dir(&claims).unwrap();
        std::fs::set_permissions(&claims, std::fs::Permissions::from_mode(0o777)).unwrap();
        core.claim().await;
        assert_eq!(mode(&claims), 0o700);

        // a link in its place is replaced by a real folder, and the target is untouched
        let core = core_with(GlobalSettingsData::default(), true).await;
        let target = core.dir.path().join("somewhere-else");
        std::fs::create_dir(&target).unwrap();
        std::os::unix::fs::symlink(&target, core.dir.path().join(CLAIM_DIR)).unwrap();
        let start = core.start().await;
        let claims = core.dir.path().join(CLAIM_DIR);
        assert!(!std::fs::symlink_metadata(&claims)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(core.folder(&start).starts_with(&claims));
        assert_eq!(std::fs::read_dir(&target).unwrap().count(), 0);
    }

    #[tokio::test]
    async fn an_expired_claim_is_refused() {
        let core = core(true).await;
        let start = core.start().await;
        core.answer(&start, &start.value);
        core.state.local_claims.state.lock().await.pending[0].started -= CLAIM_TTL;
        assert_eq!(core.finish(&start.id).await.0, StatusCode::FORBIDDEN);
        assert!(!core.folder(&start).exists());
    }

    #[tokio::test]
    async fn a_claim_is_used_once() {
        let core = core(true).await;
        let start = core.start().await;
        core.answer(&start, &start.value);
        assert_eq!(core.finish(&start.id).await.0, StatusCode::OK);
        core.answer(&start, &start.value);
        assert_eq!(core.finish(&start.id).await.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_new_claim_replaces_the_key_of_the_same_name() {
        let core = core(true).await;
        let first = core.claim().await;
        let second = core.claim().await;
        let users = core.state.users_manager.read().await;
        assert!(
            users.try_auth(first.api_key.as_deref().unwrap()).is_none(),
            "the old key is revoked"
        );
        let owner = users.try_auth(second.api_key.as_deref().unwrap()).unwrap();
        assert_eq!(users.list_api_keys(&owner.uid).len(), 1);
    }

    #[tokio::test]
    async fn the_key_limit_doesnt_block_a_claim() {
        let core = core(true).await;
        {
            let mut users = core.state.users_manager.write().await;
            let owner = users.get_user_by_username("boss").unwrap();
            for i in 0..crate::auth::api_key::MAX_API_KEYS_PER_USER {
                users
                    .create_api_key(
                        &owner.uid,
                        format!("key {i}"),
                        None,
                        crate::auth::api_key::ApiKeyScope::Full,
                        CausedBy::System,
                    )
                    .await
                    .unwrap();
            }
        }
        let result = core.claim().await;
        assert!(core
            .state
            .users_manager
            .read()
            .await
            .try_auth(result.api_key.as_deref().unwrap())
            .is_some());
    }

    #[tokio::test]
    async fn claims_wait_side_by_side() {
        let core = core(true).await;
        let first = core.start().await;
        let second = core.start().await;
        assert_ne!(first.id, second.id);
        // a second start doesn't touch the first
        assert!(core.folder(&first).exists());
        core.answer(&first, &first.value);
        assert_eq!(core.finish(&first.id).await.0, StatusCode::OK);
        core.answer(&second, &second.value);
        assert_eq!(core.finish(&second.id).await.0, StatusCode::OK);
    }

    #[tokio::test]
    async fn an_unknown_id_leaves_the_claims_alone() {
        let core = core(true).await;
        let start = core.start().await;
        assert_eq!(
            core.finish("not-a-claim-id-at-all-xx").await.0,
            StatusCode::FORBIDDEN
        );
        core.answer(&start, &start.value);
        assert_eq!(core.finish(&start.id).await.0, StatusCode::OK);
    }

    #[tokio::test]
    async fn only_this_computer_is_told_the_data_folder() {
        let core = core(true).await;
        let local = core.start().await;
        let folder = PathBuf::from(local.data_folder.expect("told to this computer"));
        assert!(folder.is_absolute());
        assert!(folder.join(&local.dir).join("claim").is_file());
        for ip in ["192.168.1.20", "172.17.0.1", "203.0.113.9"] {
            let start = core.start_from(ip.parse().unwrap()).await.unwrap();
            assert_eq!(start.data_folder, None, "{ip}");
        }
        // and the JSON leaves it out rather than sending null
        let lan = core.start_from("10.0.0.2".parse().unwrap()).await.unwrap();
        assert!(!serde_json::to_string(&lan).unwrap().contains("data_folder"));
    }

    #[tokio::test]
    async fn web_pages_cant_claim() {
        let core = core(true).await;
        let post = |path: &str, headers: &[(&str, &str)], body: Value| {
            let mut request = core.client.post(format!("{}{path}", core.base)).json(&body);
            for (name, value) in headers {
                request = request.header(*name, *value);
            }
            request.send()
        };
        // what a page's fetch() sends, from another site or from the core's own origin
        let pages: [&[(&str, &str)]; 4] = [
            &[("Origin", "https://evil.example")],
            &[("Origin", "null")],
            &[("Sec-Fetch-Site", "cross-site"), ("Sec-Fetch-Mode", "cors")],
            &[
                ("Origin", "https://127.0.0.1:16662"),
                ("Sec-Fetch-Site", "same-origin"),
            ],
        ];
        for headers in pages {
            let response = post("/local_claim/start", headers, json!({}))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{headers:?}");
            let body: Value = response.json().await.unwrap();
            assert!(body.get("data_folder").is_none(), "{headers:?}: {body}");
        }
        // nothing was started, so the page used none of this computer's allowance
        assert!(core.state.local_claims.state.lock().await.starts.is_empty());
        assert!(!core.dir.path().join(CLAIM_DIR).exists());

        // nor can a page finish a claim a program started
        let start = core.start().await;
        core.answer(&start, &start.value);
        let response = post(
            "/local_claim/finish",
            &[("Origin", "https://evil.example")],
            json!({ "id": start.id, "name": NAME }),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert!(!core.folder(&start).join("result.json").exists());
        // the program's own requests (no browser headers) still work, as does a request
        // a person made themselves
        let response = post(
            "/local_claim/finish",
            &[("Sec-Fetch-Site", "none")],
            json!({ "id": start.id, "name": NAME }),
        )
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(core.folder(&start).join("result.json").is_file());
    }

    #[tokio::test]
    async fn only_the_address_that_started_a_claim_finishes_it() {
        let core = core(true).await;
        let lan: IpAddr = "192.168.1.20".parse().unwrap();
        let start = core.start_from(lan).await.unwrap();
        core.answer(&start, &start.value);
        let request = || FinishRequest {
            id: start.id.clone(),
            name: NAME.to_string(),
            session: false,
        };
        let claims = &core.state.local_claims;
        // from this computer (the API), it isn't found, and stays
        assert!(claims
            .finish(&core.state, LOOPBACK, request())
            .await
            .is_err());
        claims
            .finish(&core.state, Some(lan), request())
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_network_flood_cant_push_out_a_claim_from_this_computer() {
        let core = core(true).await;
        let local = core.start().await;
        for i in 0..(MAX_PENDING * 3) {
            let ip: IpAddr = format!("192.168.1.{}", i + 1).parse().unwrap();
            core.start_from(ip).await.unwrap();
            let ip: IpAddr = format!("203.0.113.{}", i + 1).parse().unwrap();
            core.start_from(ip).await.unwrap();
        }
        let pending = &core.state.local_claims.state.lock().await.pending;
        for origin in [Origin::Loopback, Origin::Private, Origin::Other] {
            assert!(pending.iter().filter(|p| p.origin == origin).count() <= MAX_PENDING);
        }
        assert!(pending.iter().any(|p| p.id == local.id));
        assert!(core.folder(&local).exists());
    }

    #[tokio::test]
    async fn a_full_pool_drops_its_oldest_claim() {
        let core = core(true).await;
        let first = core.start().await;
        for _ in 0..MAX_PENDING {
            core.start().await;
        }
        assert!(!core.folder(&first).exists());
        assert_eq!(core.finish(&first.id).await.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn starts_are_limited_per_address() {
        let core = core(true).await;
        for _ in 0..MAX_STARTS_PER_MINUTE {
            core.start().await;
        }
        let (status, _) = core.post("/local_claim/start", json!({})).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        // another address still can
        core.start_from("10.0.0.5".parse().unwrap()).await.unwrap();
    }

    #[test]
    fn origins() {
        let of = |ip: &str| Origin::of(Some(ip.parse().unwrap()));
        assert_eq!(of("127.0.0.1"), Origin::Loopback);
        assert_eq!(of("::1"), Origin::Loopback);
        assert_eq!(of("192.168.1.2"), Origin::Private);
        assert_eq!(of("10.1.2.3"), Origin::Private);
        assert_eq!(of("172.17.0.1"), Origin::Private);
        assert_eq!(of("169.254.1.1"), Origin::Private);
        assert_eq!(of("fd12::1"), Origin::Private);
        assert_eq!(of("fe80::1"), Origin::Private);
        assert_eq!(of("203.0.113.9"), Origin::Other);
        assert_eq!(of("2001:db8::1"), Origin::Other);
        assert_eq!(Origin::of(None), Origin::Other);
    }

    #[tokio::test]
    async fn abandoned_claims_are_swept_after_their_time() {
        let core = core(true).await;
        let claims = core.dir.path().join(CLAIM_DIR);
        std::fs::create_dir_all(claims.join("abandoned")).unwrap();
        std::fs::write(claims.join("abandoned/owner.json"), "{}").unwrap();
        std::fs::create_dir_all(claims.join("finished")).unwrap();
        std::fs::write(claims.join("finished/result.json"), "{}").unwrap();
        let aged = std::time::SystemTime::now() - (CLAIM_TTL + ABANDONED_AFTER);
        for dir in ["abandoned", "finished"] {
            std::fs::File::open(claims.join(dir))
                .unwrap()
                .set_modified(aged)
                .unwrap();
        }
        core.start().await;
        assert!(!claims.join("abandoned").exists(), "unfinished: gone");
        assert!(
            claims.join("finished").exists(),
            "finished: kept until RESULT_TTL"
        );
    }

    #[tokio::test]
    async fn the_setting_turns_it_off() {
        let core = core_with(
            GlobalSettingsData {
                allow_local_claim: false,
                ..Default::default()
            },
            true,
        )
        .await;
        let (status, _) = core.post("/local_claim/start", json!({})).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(!core.dir.path().join(CLAIM_DIR).exists());
    }

    #[tokio::test]
    async fn clean_up_removes_old_claims() {
        let core = core(true).await;
        let start = core.start().await;
        assert!(core.folder(&start).exists());
        core.state.local_claims.clean_up().await;
        assert!(!core.dir.path().join(CLAIM_DIR).exists());
    }

    #[test]
    fn key_names_are_cleaned_up() {
        assert_eq!(
            key_name("  Lodestone\u{7} Launcher  "),
            "Lodestone Launcher"
        );
        assert_eq!(key_name(""), DEFAULT_NAME);
        assert_eq!(key_name("\u{1}\u{2}"), DEFAULT_NAME);
        assert_eq!(key_name(&"é".repeat(100)).chars().count(), 64);
    }
}
