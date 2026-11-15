//! "Connect as owner" for a Lodestone core on this computer: the launcher proves it can
//! write the core's data folder (`LODESTONE_PATH`), and the core gives it an owner-level
//! API key. Anyone who can write that folder already controls the core (it holds the
//! users, their token secrets and the API keys), so this grants nothing new.
//!
//! 1. `POST /local_claim/start` (no sign-in): the core makes `<LODESTONE_PATH>/
//!    local_claim/<id>/`, which only its user can open, with an empty `claim` file, and
//!    answers the folder (relative to its data folder), a value and its id.
//! 2. The launcher finds which of its candidate folders ([`candidate_folders`]), or the
//!    data folder the core names ([`reported_folder`]), holds that `claim` file, and
//!    writes into `<id>/`: the value into `claim`, a random value
//!    of its own into `reply`, and `owner.json`, an owner account it made up
//!    ([`OwnerCredentials`]), which the core uses only when it has no owner yet.
//! 3. `POST /local_claim/finish` with the id. The core checks `claim`, makes the owner
//!    if asked, mints the key and writes it, with the `reply` value and the SHA-256 of
//!    its HTTPS certificate, into `result.json` in the same folder. Nothing secret
//!    crosses the network.
//! 4. The launcher reads `result.json`. Its own `reply` value there shows that the
//!    claim was answered by a core that can read that folder, but not that it is the
//!    server on the port: a server there could pass `start` and `finish` on to the
//!    real core. So the key is tied to the core by its certificate instead: the
//!    launcher pins the fingerprint from `result.json` and checks, over HTTPS to the
//!    same host and port, that the core presenting it has the claimed id. The core is
//!    saved at that HTTPS address with the pin, and its key is only ever sent over a
//!    connection to that certificate ([`CoreClient::strict`]). A core without an HTTPS
//!    certificate can't be claimed.
//!
//! The folder is walked one step at a time without following links (on Unix, by file
//! descriptor), `local_claim/` and `<id>/` must belong to this user and be closed to
//! others, and `claim` must be a plain file with a single link. Afterwards only the
//! claim's own files are deleted, then its folder, never a folder the server named.
//!
//! A core in Docker works the same when its data folder is a folder of this computer
//! mounted into the container: the launcher only ever touches files inside it. A volume
//! Docker keeps to itself can't be found, and the claim says so.
//!
//! What the launcher gets is kept in plain text in `<data>/cores/
//! local_credentials.json`, readable only by this user ([`save_credentials`]). It
//! exists only because this user can already write the core's folder, so a copy next
//! to the launcher gives nothing away that the folder didn't.
//!
//! Lodestone Host, which runs its core as a child process and so knows its data
//! folder, claims a session instead of a key ([`claim_session`]): an owner sign-in
//! token, tied to the core's certificate the same way, kept only in memory.

use std::{
    collections::BTreeMap,
    fmt, fs,
    io::{self, Write as _},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use url::Url;

use super::{
    CoreClient, CoreError, CoreInfo, client,
    tls::{CertificatePin, PinnedHttp},
};
use crate::{Secret, game::download::Sleep};

/// The folder in a core's data folder where claims happen.
pub const CLAIM_DIR: &str = "local_claim";
/// How long the launcher waits for `result.json` after finishing.
pub const RESULT_WAIT: Duration = Duration::from_secs(2);
const RESULT_POLL: Duration = Duration::from_millis(100);
/// `result.json` is a few hundred bytes; anything larger is not one.
const MAX_RESULT_BYTES: u64 = 16 * 1024;
/// The credentials file, in `<data>/cores/`.
pub const CREDENTIALS_FILE: &str = "local_credentials.json";
/// The files a claim's folder holds. Only these are ever deleted from it.
const CLAIM_FILES: [&str; 4] = ["claim", "reply", "owner.json", "result.json"];

/// Why a claim failed. The messages never contain a key or password.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClaimError {
    /// No core answers on this computer's default port.
    #[error("No Lodestone core is running on this computer.")]
    NoCore,
    /// The core refused: its owner turned "Let apps on this computer claim owner
    /// access" off, or it is too old to know claims.
    #[error(
        "The core refused to give owner access: turn on \"Let apps on this computer claim owner access\" in the core's settings, or update the core."
    )]
    Refused,
    /// None of the folders holds the claim the core made: the data folder is elsewhere,
    /// or inside a Docker volume this computer can't see.
    #[error(
        "Couldn't find this core's data folder. Choose the folder the core keeps its data in (its LODESTONE_PATH)."
    )]
    FolderNotFound,
    /// The answer in `result.json` isn't the launcher's, or the core at the HTTPS
    /// address isn't the claimed one.
    #[error("The core that answered isn't the one whose folder was found, so nothing was saved.")]
    NotThisCore,
    /// The core has no HTTPS certificate, so its key couldn't be tied to it.
    #[error(
        "This core has no HTTPS certificate, so {app} can't make sure its owner key only ever goes to it. Nothing was saved.",
        app = crate::wording::app_name()
    )]
    NoCertificate,
    /// The core didn't write `result.json` in time.
    #[error("The core didn't finish the claim. Try again.")]
    NoResult,
    #[error("{0}")]
    Core(CoreError),
    #[error("Couldn't use the core's data folder: {0}")]
    Files(String),
    #[error(
        "The core sent something {app} doesn't understand: {0}",
        app = crate::wording::app_name()
    )]
    Invalid(String),
}

impl From<io::Error> for ClaimError {
    fn from(err: io::Error) -> Self {
        ClaimError::Files(err.to_string())
    }
}

/// `POST /local_claim/start`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ClaimStart {
    pub id: String,
    /// `local_claim/<id>`, relative to the core's data folder.
    pub dir: String,
    /// What the launcher writes into `claim`. Not a secret: only the folder's owner can
    /// write it there.
    pub value: String,
    pub core_uuid: String,
    #[serde(default)]
    pub expires_in: Option<u64>,
    /// The core's data folder, absolute, which a core tells programs on its own
    /// computer. Only a hint ([`reported_folder`]): the claim must still be found there.
    #[serde(default)]
    pub data_folder: Option<String>,
}

/// `result.json`. No `Debug`: it holds the key.
#[derive(Deserialize)]
struct ClaimResult {
    /// The owner API key, for a key claim.
    #[serde(default)]
    api_key: Option<String>,
    /// The owner's sign-in token, for a session claim ([`claim_session`]).
    #[serde(default)]
    session_token: Option<String>,
    reply: String,
    #[serde(default)]
    owner_created: bool,
    #[serde(default)]
    owner_username: Option<String>,
    /// The core's HTTPS certificate, `AB:CD:…`; `None` for a core without one.
    #[serde(default)]
    tls_fingerprint_sha256: Option<String>,
}

/// The owner account the launcher makes up, for a core that has none yet.
#[derive(Clone)]
pub struct OwnerCredentials {
    pub username: String,
    pub password: Secret,
}

impl fmt::Debug for OwnerCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnerCredentials")
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

const LOWER_ALNUM: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
const PASSWORD_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789-_.!@#%";

/// `len` characters from `alphabet`, from the system's random generator (without bias:
/// bytes that would favour the first characters are thrown away).
fn random_string(alphabet: &[u8], len: usize) -> String {
    let limit = 256 - 256 % alphabet.len();
    let mut out = String::with_capacity(len);
    while out.len() < len {
        for byte in crate::random_bytes::<64>() {
            if (byte as usize) < limit && out.len() < len {
                out.push(alphabet[byte as usize % alphabet.len()] as char);
            }
        }
    }
    out
}

impl OwnerCredentials {
    /// `owner_` and five lowercase letters or digits, and a 32-character password.
    pub fn generate() -> Self {
        Self {
            username: format!("owner_{}", random_string(LOWER_ALNUM, 5)),
            password: Secret::new(random_string(PASSWORD_CHARS, 32)),
        }
    }

    /// An owner without a password, for Lodestone Host's first run ([`claim_session`]
    /// only: the core refuses it in a key claim). Nobody can sign in to it with a
    /// password, and the core serves only this computer until the owner sets one up
    /// (`AdminClient::set_up_owner_account`).
    pub fn without_password(username: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: Secret::new(String::new()),
        }
    }

    /// Whether it has a password (see [`Self::without_password`]).
    pub fn has_password(&self) -> bool {
        !self.password.expose().is_empty()
    }
}

/// What a claim gave.
pub struct Claimed {
    pub info: CoreInfo,
    /// The core's HTTPS address (the claim's host and port).
    pub url: Url,
    /// The core's certificate, which its key is only ever sent to.
    pub pin: CertificatePin,
    pub api_key: Secret,
    /// The key's public id (`lsk_<id>_…`).
    pub key_id: Option<String>,
    /// The folder the claim was made in.
    pub folder: PathBuf,
    /// The owner's username, as the core said.
    pub owner_username: Option<String>,
    /// The account the launcher made, when the core had no owner.
    pub created_owner: Option<OwnerCredentials>,
}

/// A claim the core answered, before its certificate was checked over HTTPS.
pub struct Unverified {
    pub info: CoreInfo,
    /// The address the claim was made over.
    pub url: Url,
    pub fingerprint: String,
    pub api_key: Secret,
    pub folder: PathBuf,
    pub owner_username: Option<String>,
    pub created_owner: Option<OwnerCredentials>,
}

/// The public id of a key `lsk_<id>_<secret>`.
pub fn key_id(key: &str) -> Option<String> {
    let rest = key.strip_prefix("lsk_")?;
    let (id, secret) = rest.split_once('_')?;
    (!id.is_empty() && !secret.is_empty()).then(|| id.to_string())
}

/// Where a core on this computer may keep its data: the folders the user chose, then
/// `$LODESTONE_PATH`, then `~/.lodestone` (also the desktop client's), each once.
pub fn candidate_folders(chosen: &[PathBuf]) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = chosen.to_vec();
    if let Some(path) = std::env::var_os("LODESTONE_PATH").filter(|v| !v.is_empty()) {
        folders.push(PathBuf::from(path));
    }
    if let Some(home) = dirs::home_dir() {
        folders.push(home.join(".lodestone"));
    }
    let mut seen = Vec::new();
    folders.retain(|folder| {
        let key = fs::canonicalize(folder).unwrap_or_else(|_| folder.clone());
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
    folders
}

/// The claim's id, checked against `dir` (`local_claim/<id>`), so the launcher never
/// touches anything outside that folder.
fn claim_id(start: &ClaimStart) -> Result<&str, ClaimError> {
    let id_ok = !start.id.is_empty()
        && start.id.len() <= 128
        && start
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !id_ok || start.dir.trim_end_matches('/') != format!("{CLAIM_DIR}/{}", start.id) {
        return Err(ClaimError::Invalid(format!(
            "the claim's folder {:?} isn't {CLAIM_DIR}/<id>",
            start.dir
        )));
    }
    Ok(&start.id)
}

/// A claim's folder, opened one step at a time without following links.
#[cfg(unix)]
mod claim_dir {
    use std::{
        ffi::CString,
        fs::File,
        io::{self, Read as _, Write as _},
        mem::MaybeUninit,
        os::{
            fd::{AsRawFd as _, FromRawFd as _, OwnedFd},
            unix::ffi::OsStrExt as _,
        },
        path::Path,
    };

    pub struct ClaimDir {
        /// `local_claim/`, to remove the claim's folder from.
        parent: OwnedFd,
        /// `local_claim/<id>/`.
        dir: OwnedFd,
        id: CString,
    }

    fn name(value: &str) -> io::Result<CString> {
        CString::new(value).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad name"))
    }

    fn denied(why: &str) -> io::Error {
        io::Error::new(io::ErrorKind::PermissionDenied, why.to_string())
    }

    /// `openat`, closed on exec. `mode` is used only with `O_CREAT`.
    fn open_at(dir: Option<&OwnedFd>, name: &CString, flags: libc::c_int) -> io::Result<OwnedFd> {
        let base = dir.map_or(libc::AT_FDCWD, |dir| dir.as_raw_fd());
        // SAFETY: `name` is NUL-terminated and outlives the call; the mode argument is
        // read only with O_CREAT. A non-negative result is a new descriptor we own.
        let fd = unsafe {
            libc::openat(
                base,
                name.as_ptr(),
                flags | libc::O_CLOEXEC,
                0o600 as libc::c_uint,
            )
        };
        if fd < 0 {
            Err(io::Error::last_os_error())
        } else {
            // SAFETY: `fd` was just opened and nothing else owns it.
            Ok(unsafe { OwnedFd::from_raw_fd(fd) })
        }
    }

    fn stat(fd: &OwnedFd) -> io::Result<libc::stat> {
        let mut st = MaybeUninit::<libc::stat>::zeroed();
        // SAFETY: `st` is valid for writes of a `stat`; `fd` is open.
        if unsafe { libc::fstat(fd.as_raw_fd(), st.as_mut_ptr()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fstat succeeded, so it filled `st`.
        Ok(unsafe { st.assume_init() })
    }

    fn uid() -> libc::uid_t {
        // SAFETY: getuid has no preconditions and can't fail.
        unsafe { libc::getuid() }
    }

    fn is_kind(st: &libc::stat, kind: libc::mode_t) -> bool {
        (st.st_mode & libc::S_IFMT) == kind
    }

    /// A folder of this user that nobody else may use.
    fn check_private_dir(fd: &OwnedFd) -> io::Result<()> {
        let st = stat(fd)?;
        if !is_kind(&st, libc::S_IFDIR) || st.st_uid != uid() || st.st_mode & 0o077 != 0 {
            return Err(denied(
                "the claim's folder isn't a folder of this user, closed to others",
            ));
        }
        Ok(())
    }

    /// A plain file with a single link.
    fn plain(st: &libc::stat) -> bool {
        is_kind(st, libc::S_IFREG) && st.st_nlink == 1
    }

    const DIR_FLAGS: libc::c_int = libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW;

    impl ClaimDir {
        /// `<root>/local_claim/<id>/`. `root` may be a link (the user's choice); the
        /// steps below it may not, and both must be this user's and closed to others.
        pub fn open(root: &Path, id: &str) -> io::Result<Self> {
            let root = CString::new(root.as_os_str().as_bytes())
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad path"))?;
            let root = open_at(None, &root, libc::O_RDONLY | libc::O_DIRECTORY)?;
            let parent = open_at(Some(&root), &name(super::CLAIM_DIR)?, DIR_FLAGS)?;
            check_private_dir(&parent)?;
            let id = name(id)?;
            let dir = open_at(Some(&parent), &id, DIR_FLAGS)?;
            check_private_dir(&dir)?;
            Ok(Self { parent, dir, id })
        }

        /// Whether `claim` is still the empty plain file of this user the core made.
        pub fn claim_is_fresh(&self) -> io::Result<bool> {
            let fd = open_at(
                Some(&self.dir),
                &name("claim")?,
                libc::O_RDONLY | libc::O_NOFOLLOW,
            )?;
            let st = stat(&fd)?;
            Ok(plain(&st) && st.st_uid == uid() && st.st_size == 0)
        }

        /// Writes `value` into `claim`, checked again on the open file.
        pub fn write_claim(&self, value: &[u8]) -> io::Result<()> {
            let fd = open_at(
                Some(&self.dir),
                &name("claim")?,
                libc::O_WRONLY | libc::O_NOFOLLOW,
            )?;
            let st = stat(&fd)?;
            if !plain(&st) || st.st_uid != uid() {
                return Err(denied("claim isn't a plain file of this user"));
            }
            let mut file = File::from(fd);
            file.set_len(0)?;
            file.write_all(value)?;
            file.sync_all()
        }

        /// Makes the new file `name` (it must not exist), readable only by this user.
        pub fn create(&self, file: &str, bytes: &[u8]) -> io::Result<()> {
            let fd = open_at(
                Some(&self.dir),
                &name(file)?,
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW,
            )?;
            let mut file = File::from(fd);
            file.write_all(bytes)?;
            file.sync_all()
        }

        /// The file `name`, if it is there: a plain file of at most `limit` bytes.
        pub fn read(&self, file: &str, limit: u64) -> io::Result<Option<Vec<u8>>> {
            let fd = match open_at(
                Some(&self.dir),
                &name(file)?,
                libc::O_RDONLY | libc::O_NOFOLLOW,
            ) {
                Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
                result => result?,
            };
            let st = stat(&fd)?;
            if !plain(&st) {
                return Err(denied("the file isn't a plain file"));
            }
            if st.st_size as u64 > limit {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "too large"));
            }
            let mut bytes = Vec::new();
            File::from(fd).take(limit + 1).read_to_end(&mut bytes)?;
            Ok(Some(bytes))
        }

        /// Deletes the file `name`, if it is there.
        pub fn remove(&self, file: &str) {
            if let Ok(file) = name(file) {
                // SAFETY: `file` is NUL-terminated; the descriptor is open.
                unsafe { libc::unlinkat(self.dir.as_raw_fd(), file.as_ptr(), 0) };
            }
        }

        /// Deletes the claim's own files, then its folder if that left it empty.
        pub fn cleanup(self) {
            for file in super::CLAIM_FILES {
                self.remove(file);
            }
            // SAFETY: `id` is NUL-terminated; the descriptor is open.
            unsafe {
                libc::unlinkat(
                    self.parent.as_raw_fd(),
                    self.id.as_ptr(),
                    libc::AT_REMOVEDIR,
                )
            };
        }
    }
}

/// A claim's folder, by path, checking each step isn't a link.
#[cfg(not(unix))]
mod claim_dir {
    use std::{
        fs::{self, OpenOptions},
        io::{self, Read as _, Write as _},
        path::{Path, PathBuf},
    };

    pub struct ClaimDir {
        dir: PathBuf,
    }

    fn real_dir(path: &Path) -> bool {
        fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
    }

    impl ClaimDir {
        pub fn open(root: &Path, id: &str) -> io::Result<Self> {
            let parent = root.join(super::CLAIM_DIR);
            let dir = parent.join(id);
            if !real_dir(&parent) || !real_dir(&dir) {
                return Err(io::Error::new(io::ErrorKind::NotFound, "no claim folder"));
            }
            Ok(Self { dir })
        }

        pub fn claim_is_fresh(&self) -> io::Result<bool> {
            let meta = fs::symlink_metadata(self.dir.join("claim"))?;
            Ok(meta.is_file() && meta.len() == 0)
        }

        pub fn write_claim(&self, value: &[u8]) -> io::Result<()> {
            let path = self.dir.join("claim");
            if !fs::symlink_metadata(&path)?.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "not a file",
                ));
            }
            let mut file = OpenOptions::new().write(true).truncate(true).open(&path)?;
            file.write_all(value)?;
            file.sync_all()
        }

        pub fn create(&self, file: &str, bytes: &[u8]) -> io::Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.dir.join(file))?;
            file.write_all(bytes)?;
            file.sync_all()
        }

        pub fn read(&self, file: &str, limit: u64) -> io::Result<Option<Vec<u8>>> {
            let path = self.dir.join(file);
            let meta = match fs::symlink_metadata(&path) {
                Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
                result => result?,
            };
            if !meta.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "not a file",
                ));
            }
            if meta.len() > limit {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "too large"));
            }
            let mut bytes = Vec::new();
            fs::File::open(&path)?
                .take(limit + 1)
                .read_to_end(&mut bytes)?;
            Ok(Some(bytes))
        }

        pub fn remove(&self, file: &str) {
            let _ = fs::remove_file(self.dir.join(file));
        }

        pub fn cleanup(self) {
            for file in super::CLAIM_FILES {
                self.remove(file);
            }
            let _ = fs::remove_dir(&self.dir);
        }
    }
}

pub use claim_dir::ClaimDir;

/// The data folder the core named in `start`, when the launcher may look in it: an
/// absolute path without `..`. Whoever answers on the port names it, so it gets the
/// same checks as any folder ([`ClaimDir::open`]): on Unix the claim's folders must
/// belong to this user and be closed to others, so a folder another user prepared is
/// never written to. Windows has no such check here, so there the folder must be inside
/// this user's profile folder (`home`), which only the user can open.
pub fn reported_folder(start: &ClaimStart, home: Option<&Path>) -> Option<PathBuf> {
    let path = PathBuf::from(start.data_folder.as_deref().filter(|p| !p.is_empty())?);
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return None;
    }
    if cfg!(unix) {
        return Some(path);
    }
    let home = fs::canonicalize(home?).ok()?;
    let real = fs::canonicalize(&path).ok()?;
    (real.starts_with(&home) && real != home).then_some(path)
}

/// The first of `folders`, then the folder the core named ([`reported_folder`]), that
/// holds the claim the core just made, and its folder.
pub fn find_claim_folder(
    folders: &[PathBuf],
    start: &ClaimStart,
) -> Result<(PathBuf, ClaimDir), ClaimError> {
    let id = claim_id(start)?;
    let reported = reported_folder(start, dirs::home_dir().as_deref());
    for folder in folders.iter().chain(reported.as_ref()) {
        if let Ok(dir) = ClaimDir::open(folder, id)
            && dir.claim_is_fresh().unwrap_or(false)
        {
            return Ok((folder.clone(), dir));
        }
    }
    Err(ClaimError::FolderNotFound)
}

/// Writes the launcher's side of the claim: the value into `claim`, a new `reply` and
/// a new `owner.json`. Returns the reply value.
#[cfg(test)]
fn write_claim(
    dir: &ClaimDir,
    start: &ClaimStart,
    owner: &OwnerCredentials,
) -> Result<String, ClaimError> {
    write_claim_with(dir, start, Some(owner))
}

/// [`write_claim`], with `owner.json` only when there is an owner to make.
fn write_claim_with(
    dir: &ClaimDir,
    start: &ClaimStart,
    owner: Option<&OwnerCredentials>,
) -> Result<String, ClaimError> {
    dir.write_claim(start.value.as_bytes())?;
    let reply = random_string(LOWER_ALNUM, 32);
    dir.create("reply", reply.as_bytes())?;
    if let Some(owner) = owner {
        // without a password: `null`, an owner who has none
        let password = owner.has_password().then(|| owner.password.expose());
        let owner_json = serde_json::json!({
            "username": owner.username,
            "password": password,
        });
        dir.create("owner.json", owner_json.to_string().as_bytes())?;
    }
    Ok(reply)
}

/// `result.json`, once the core has written it.
fn read_result(dir: &ClaimDir) -> Result<Option<ClaimResult>, ClaimError> {
    let Some(bytes) = dir.read("result.json", MAX_RESULT_BYTES)? else {
        return Ok(None);
    };
    if bytes.iter().all(u8::is_ascii_whitespace) {
        // made but not filled yet
        return Ok(None);
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|err| ClaimError::Invalid(format!("result.json: {err}")))
}

/// Checks the result against the launcher's reply value, and returns the core's
/// certificate fingerprint.
#[cfg(test)]
fn check_result(result: &ClaimResult, reply: &str) -> Result<String, ClaimError> {
    check_result_for(result, reply, false)
}

/// [`check_result`], for a key claim or (`session`) a session claim.
fn check_result_for(
    result: &ClaimResult,
    reply: &str,
    session: bool,
) -> Result<String, ClaimError> {
    if !constant_time_eq(result.reply.as_bytes(), reply.as_bytes()) {
        return Err(ClaimError::NotThisCore);
    }
    if session {
        if result
            .session_token
            .as_deref()
            .is_none_or(|token| token.trim().is_empty())
        {
            return Err(ClaimError::Invalid(
                "the core sent no sign-in token (is it too old?)".into(),
            ));
        }
    } else if result.api_key.as_deref().and_then(key_id).is_none() {
        return Err(ClaimError::Invalid(
            "the key isn't a Lodestone API key".into(),
        ));
    }
    let fingerprint = result
        .tls_fingerprint_sha256
        .as_deref()
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .ok_or(ClaimError::NoCertificate)?;
    let hex: String = fingerprint.chars().filter(|c| *c != ':').collect();
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ClaimError::Invalid(
            "the certificate fingerprint isn't a SHA-256".into(),
        ));
    }
    Ok(fingerprint.to_string())
}

/// Compares in constant time (for the length of the shorter input).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The cores answering on this computer's default port, on `127.0.0.1` and `::1`,
/// each once (one core usually answers on both).
pub async fn discover(http: Arc<dyn http_client::HttpClient>) -> Vec<(Url, CoreInfo)> {
    let mut found: Vec<(Url, CoreInfo)> = Vec::new();
    for address in ["http://127.0.0.1", "http://[::1]"] {
        let Ok(url) = client::parse_address(address) else {
            continue;
        };
        if let Ok(info) = CoreClient::new(http.clone(), url.clone()).info().await
            && !found.iter().any(|(_, seen)| seen.uuid == info.uuid)
        {
            found.push((url, info));
        }
    }
    found
}

/// Claims owner access to the core `client` talks to, looking for its data folder in
/// `folders` ([`candidate_folders`]), and ties the key to the core's certificate
/// ([`verify`]). `name` names the key: "Lodestone Launcher on …".
pub async fn claim(
    client: &CoreClient,
    folders: &[PathBuf],
    name: &str,
    sleep: Sleep,
) -> Result<Claimed, ClaimError> {
    verify(claim_unverified(client, folders, name, sleep).await?).await
}

/// The claim up to `result.json`: everything but the HTTPS check. The claim's files
/// are deleted however it ends.
pub async fn claim_unverified(
    client: &CoreClient,
    folders: &[PathBuf],
    name: &str,
    sleep: Sleep,
) -> Result<Unverified, ClaimError> {
    let info = client.info().await.map_err(|err| match err {
        CoreError::Unreachable(_) => ClaimError::NoCore,
        err => ClaimError::Core(err),
    })?;
    let start = client.local_claim_start().await.map_err(refused)?;
    if start.core_uuid != info.uuid {
        return Err(ClaimError::NotThisCore);
    }
    let (folder, dir) = find_claim_folder(folders, &start)?;
    let owner = OwnerCredentials::generate();
    let outcome = finish(client, &dir, &start, Some(&owner), name, false, sleep).await;
    // owner.json holds the password and result.json the key: they go, whatever happened
    dir.cleanup();
    let (result, fingerprint) = outcome?;
    Ok(Unverified {
        url: client.base().clone(),
        fingerprint,
        api_key: Secret::new(result.api_key.unwrap_or_default()),
        folder,
        owner_username: result.owner_username,
        created_owner: result.owner_created.then_some(owner),
        info,
    })
}

async fn finish(
    client: &CoreClient,
    dir: &ClaimDir,
    start: &ClaimStart,
    owner: Option<&OwnerCredentials>,
    name: &str,
    session: bool,
    sleep: Sleep,
) -> Result<(ClaimResult, String), ClaimError> {
    let reply = write_claim_with(dir, start, owner)?;
    client
        .local_claim_finish_with(&start.id, name, session)
        .await
        .map_err(|err| match err {
            // the claim started, so claims are on: a refusal now is about the claim
            // itself (an owner account the core won't take), and says why
            CoreError::Forbidden(reason) if session => {
                ClaimError::Core(CoreError::Forbidden(reason))
            }
            err => refused(err),
        })?;
    let mut waited = Duration::ZERO;
    loop {
        if let Some(result) = read_result(dir)? {
            let fingerprint = check_result_for(&result, &reply, session)?;
            return Ok((result, fingerprint));
        }
        if waited >= RESULT_WAIT {
            return Err(ClaimError::NoResult);
        }
        sleep(RESULT_POLL).await;
        waited += RESULT_POLL;
    }
}

/// Owner access to a core this program runs itself, as a sign-in session.
pub struct OwnerSession {
    pub info: CoreInfo,
    /// The core's HTTPS address (the claim's host and port).
    pub url: Url,
    /// The core's certificate, which the token is only ever sent to.
    pub pin: CertificatePin,
    /// The owner's sign-in token (a session, as `POST /user/login` gives).
    pub token: Secret,
    /// The owner's username, as the core said.
    pub owner_username: Option<String>,
    /// Whether this claim made the owner account.
    pub owner_created: bool,
}

impl std::fmt::Debug for OwnerSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnerSession")
            .field("url", &self.url.as_str())
            .field("owner_username", &self.owner_username)
            .finish_non_exhaustive()
    }
}

impl OwnerSession {
    /// A client that sends the token only to the core's certificate, over HTTPS.
    pub fn client(&self) -> Result<CoreClient, ClaimError> {
        let pinned = Arc::new(
            PinnedHttp::new(Some(self.pin.clone()))
                .map_err(|err| ClaimError::Invalid(err.to_string()))?,
        );
        Ok(CoreClient::new(pinned.http(), self.url.clone())
            .with_pinned(pinned)
            .strict())
    }
}

/// Signs in as the owner of the core `client` talks to, whose data folder is exactly
/// `folder`: for a program that runs the core itself (Lodestone Host). `owner` is
/// the owner account to make when the core has none yet (its `is_setup` is false);
/// a core without an owner refuses a claim without one. The token is tied to the
/// core's certificate, as a key claim's key is ([`verify`]). The claim's files are
/// deleted however it ends.
pub async fn claim_session(
    client: &CoreClient,
    folder: &Path,
    owner: Option<&OwnerCredentials>,
    name: &str,
    sleep: Sleep,
) -> Result<OwnerSession, ClaimError> {
    let info = client.info().await.map_err(|err| match err {
        CoreError::Unreachable(_) => ClaimError::NoCore,
        err => ClaimError::Core(err),
    })?;
    let start = client.local_claim_start().await.map_err(refused)?;
    if start.core_uuid != info.uuid {
        return Err(ClaimError::NotThisCore);
    }
    let id = claim_id(&start)?;
    let dir = ClaimDir::open(folder, id).map_err(|_| ClaimError::FolderNotFound)?;
    if !dir.claim_is_fresh().unwrap_or(false) {
        return Err(ClaimError::FolderNotFound);
    }
    let outcome = finish(client, &dir, &start, owner, name, true, sleep).await;
    // owner.json holds the password and result.json the token: they go, whatever happened
    dir.cleanup();
    let (result, fingerprint) = outcome?;
    let token = Secret::new(result.session_token.unwrap_or_default());
    let url = https_url(client.base())?;
    let host = url
        .host_str()
        .ok_or_else(|| ClaimError::Invalid("the core's address has no host".into()))?;
    let session = OwnerSession {
        pin: CertificatePin::new(host, &fingerprint),
        url,
        token,
        owner_username: result.owner_username,
        owner_created: result.owner_created,
        info,
    };
    // the core at the HTTPS address, with that certificate, must be the claimed one
    let verified = session.client()?.info().await.map_err(ClaimError::Core)?;
    if verified.uuid != session.info.uuid {
        return Err(ClaimError::NotThisCore);
    }
    Ok(session)
}

/// `url` over HTTPS: the same host and port.
pub fn https_url(url: &Url) -> Result<Url, ClaimError> {
    let mut https = url.clone();
    https
        .set_scheme("https")
        .map_err(|()| ClaimError::Invalid(format!("{url} has no HTTPS form")))?;
    if https.port().is_none() {
        // the default port of http (80) isn't https's
        https
            .set_port(url.port_or_known_default())
            .map_err(|()| ClaimError::Invalid(format!("{url} has no port")))?;
    }
    Ok(https)
}

/// Ties the claim to the core's certificate: pins the fingerprint from `result.json`
/// and checks, over HTTPS to the claim's host and port and only to that certificate,
/// that the core there is the claimed one.
pub async fn verify(claim: Unverified) -> Result<Claimed, ClaimError> {
    let url = https_url(&claim.url)?;
    let host = url
        .host_str()
        .ok_or_else(|| ClaimError::Invalid("the core's address has no host".into()))?;
    let pin = CertificatePin::new(host, &claim.fingerprint);
    let pinned = Arc::new(
        PinnedHttp::new(Some(pin.clone())).map_err(|err| ClaimError::Invalid(err.to_string()))?,
    );
    let client = CoreClient::new(pinned.http(), url.clone())
        .with_pinned(pinned)
        .strict();
    let info = client.info().await.map_err(ClaimError::Core)?;
    if info.uuid != claim.info.uuid {
        return Err(ClaimError::NotThisCore);
    }
    Ok(Claimed {
        url,
        pin,
        key_id: key_id(claim.api_key.expose()),
        api_key: claim.api_key,
        folder: claim.folder,
        owner_username: claim.owner_username,
        created_owner: claim.created_owner,
        info,
    })
}

fn refused(err: CoreError) -> ClaimError {
    match err {
        CoreError::Forbidden(_) | CoreError::NotFound(_) => ClaimError::Refused,
        CoreError::Unreachable(_) => ClaimError::NoCore,
        err => ClaimError::Core(err),
    }
}

/// What the launcher keeps for a claimed core, in plain text.
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalCredentials {
    /// The core's HTTPS address; the key is only used for a saved core at exactly it.
    pub url: String,
    /// The core's certificate the key is tied to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_fingerprint: Option<String>,
    /// The owner-level API key; `None` once signed out (the owner account stays).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// The owner account, when the launcher made it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_username: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_password: Option<String>,
}

impl fmt::Debug for LocalCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalCredentials")
            .field("url", &self.url)
            .field("owner_username", &self.owner_username)
            .finish_non_exhaustive()
    }
}

impl LocalCredentials {
    /// The key, for a saved core at `url` pinned to `fingerprint`: only when that is
    /// exactly where and what it was claimed for, on this computer.
    pub fn key_for(&self, url: &str, fingerprint: Option<&str>) -> Option<Secret> {
        let key = self.api_key.as_ref()?;
        let parsed = Url::parse(url).ok()?;
        let loopback = match parsed.host()? {
            url::Host::Ipv4(ip) => ip.is_loopback(),
            url::Host::Ipv6(ip) => ip.is_loopback(),
            url::Host::Domain(name) => name.eq_ignore_ascii_case("localhost"),
        };
        let pinned = match (self.tls_fingerprint.as_deref(), fingerprint) {
            (Some(ours), Some(theirs)) => super::tls::same_fingerprint(ours, theirs),
            _ => false,
        };
        (parsed.scheme() == "https" && loopback && pinned && self.url == url)
            .then(|| Secret::new(key.clone()))
    }
}

/// `local_credentials.json`, by core id.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CredentialsFile {
    #[serde(default)]
    pub cores: BTreeMap<String, LocalCredentials>,
}

impl CredentialsFile {
    /// Keeps what a claim gave for `uuid`. An owner account the launcher made before
    /// stays when this claim made none.
    pub fn record(&mut self, uuid: &str, claimed: &Claimed) {
        let entry = self.cores.entry(uuid.to_string()).or_default();
        entry.url = client::display_address(&claimed.url);
        entry.tls_fingerprint = Some(claimed.pin.sha256.clone());
        entry.api_key = Some(claimed.api_key.expose().to_string());
        if let Some(owner) = &claimed.created_owner {
            entry.owner_username = Some(owner.username.clone());
            entry.owner_password = Some(owner.password.expose().to_string());
        }
    }

    /// Forgets the key for `uuid`, keeping the owner account (its only copy).
    /// Returns whether anything changed.
    pub fn forget_key(&mut self, uuid: &str) -> bool {
        let Some(entry) = self.cores.get_mut(uuid) else {
            return false;
        };
        let had = entry.api_key.take().is_some();
        if entry.owner_password.is_none() {
            self.cores.remove(uuid);
        }
        had
    }
}

/// `<data>/cores/local_credentials.json`.
pub fn credentials_path(data_root: &Path) -> PathBuf {
    data_root.join("cores").join(CREDENTIALS_FILE)
}

/// The file, or an empty one when there is none.
pub fn load_credentials(path: &Path) -> io::Result<CredentialsFile> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(CredentialsFile::default()),
        Err(err) => Err(err),
    }
}

/// Saves the file atomically, readable only by this user.
pub fn save_credentials(path: &Path, file: &CredentialsFile) -> io::Result<()> {
    let text = serde_json::to_string_pretty(file).map_err(io::Error::other)?;
    crate::storage::write_atomic_with(path, |out| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            out.set_permissions(fs::Permissions::from_mode(0o600))?;
        }
        out.write_all(text.as_bytes())
    })
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use super::*;

    const FINGERPRINT: &str = "3F:9A:0C:11:D2:E8:4B:77:56:A1:C0:FF:EE:42:B1:9D:8E:7A:6C:5B:4D:3E:2F:1A:0B:9C:8D:7E:6F:5A:4B:3C";

    fn start(id: &str) -> ClaimStart {
        ClaimStart {
            id: id.into(),
            dir: format!("{CLAIM_DIR}/{id}"),
            value: "v4lue".into(),
            core_uuid: "core".into(),
            expires_in: Some(60),
            data_folder: None,
        }
    }

    /// Makes `path` a folder only its user may use, as the core makes its claims.
    fn private_dir(path: &Path) {
        fs::create_dir_all(path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }

    /// A data folder with the claim the core makes.
    fn core_folder(root: &Path, id: &str) -> PathBuf {
        private_dir(&root.join(CLAIM_DIR));
        let dir = root.join(CLAIM_DIR).join(id);
        private_dir(&dir);
        File::create(dir.join("claim")).unwrap();
        dir
    }

    fn open(root: &Path, id: &str) -> ClaimDir {
        ClaimDir::open(root, id).unwrap()
    }

    #[test]
    fn owner_accounts_look_as_they_should() {
        let owner = OwnerCredentials::generate();
        let suffix = owner.username.strip_prefix("owner_").unwrap();
        assert_eq!(suffix.len(), 5);
        assert!(
            suffix
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        );
        let password = owner.password.expose();
        assert_eq!(password.len(), 32);
        assert!(password.bytes().all(|b| PASSWORD_CHARS.contains(&b)));
        assert_ne!(OwnerCredentials::generate().password, owner.password);
        assert!(!format!("{owner:?}").contains(password));
    }

    #[test]
    fn key_ids_come_from_the_key() {
        assert_eq!(key_id("lsk_ab12_secret").as_deref(), Some("ab12"));
        for bad in ["lsk_ab12", "lsk__x", "lsk_ab_", "nope"] {
            assert_eq!(key_id(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_folder_with_the_claim_is_found() {
        let tmp = tempfile::tempdir().unwrap();
        let elsewhere = tmp.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        let core = tmp.path().join("core");
        let dir = core_folder(&core, "c1");
        let (found, _) =
            find_claim_folder(&[elsewhere.clone(), core.clone()], &start("c1")).unwrap();
        assert_eq!(found, core);
        assert!(matches!(
            find_claim_folder(&[elsewhere], &start("c1")),
            Err(ClaimError::FolderNotFound)
        ));
        // a claim that was already answered isn't this one
        fs::write(dir.join("claim"), "taken").unwrap();
        assert!(matches!(
            find_claim_folder(&[core], &start("c1")),
            Err(ClaimError::FolderNotFound)
        ));
    }

    #[test]
    fn the_folder_the_core_names_is_looked_in_last() {
        let tmp = tempfile::tempdir().unwrap();
        let elsewhere = tmp.path().join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        let core = tmp.path().join("core");
        core_folder(&core, "c1");
        let mut told = start("c1");
        told.data_folder = Some(core.to_str().unwrap().into());
        // on Windows only inside the profile folder, which a temp folder may not be
        if cfg!(unix) {
            let (found, _) = find_claim_folder(&[elsewhere], &told).unwrap();
            assert_eq!(found, core);
        }
        // a folder the core names but that doesn't hold the claim is no help
        told.data_folder = Some(tmp.path().to_str().unwrap().into());
        assert!(matches!(
            find_claim_folder(&[], &told),
            Err(ClaimError::FolderNotFound)
        ));
    }

    #[test]
    fn only_a_plain_absolute_folder_is_taken_from_the_core() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        let inside = home.join("lodestone");
        fs::create_dir_all(&inside).unwrap();
        let told = |path: &str| ClaimStart {
            data_folder: Some(path.into()),
            ..start("c1")
        };
        let inside_text = inside.to_str().unwrap().to_string();
        assert_eq!(
            reported_folder(&told(&inside_text), Some(&home)),
            Some(inside.clone())
        );
        assert_eq!(reported_folder(&start("c1"), Some(&home)), None);
        assert_eq!(reported_folder(&told(""), Some(&home)), None);
        assert_eq!(reported_folder(&told("lodestone"), Some(&home)), None);
        assert_eq!(reported_folder(&told("./lodestone"), Some(&home)), None);
        let climbing = home.join("lodestone").join("..").join("..");
        assert_eq!(
            reported_folder(&told(climbing.to_str().unwrap()), Some(&home)),
            None
        );
        // Windows: inside the profile folder only
        if !cfg!(unix) {
            let outside = tmp.path().join("shared");
            fs::create_dir_all(&outside).unwrap();
            assert_eq!(
                reported_folder(&told(outside.to_str().unwrap()), Some(&home)),
                None
            );
            assert_eq!(
                reported_folder(&told(home.to_str().unwrap()), Some(&home)),
                None
            );
            assert_eq!(reported_folder(&told(&inside_text), None), None);
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_named_folder_someone_else_could_open_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let core = tmp.path().join("core");
        let dir = core_folder(&core, "c1");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o777)).unwrap();
        let told = ClaimStart {
            data_folder: Some(core.to_str().unwrap().into()),
            ..start("c1")
        };
        assert!(matches!(
            find_claim_folder(&[], &told),
            Err(ClaimError::FolderNotFound)
        ));
    }

    #[test]
    fn the_cores_folder_name_is_checked() {
        let mut bad = start("c1");
        bad.dir = "../../etc".into();
        assert!(matches!(
            find_claim_folder(&[PathBuf::from("/tmp")], &bad),
            Err(ClaimError::Invalid(_))
        ));
        let mut bad = start("../x");
        bad.dir = format!("{CLAIM_DIR}/../x");
        assert!(matches!(claim_id(&bad), Err(ClaimError::Invalid(_))));
    }

    #[cfg(unix)]
    #[test]
    fn folders_open_to_others_are_refused() {
        use std::os::unix::fs::PermissionsExt as _;
        let tmp = tempfile::tempdir().unwrap();
        let core = tmp.path().join("core");
        core_folder(&core, "c1");
        for open_to_others in [core.join(CLAIM_DIR), core.join(CLAIM_DIR).join("c1")] {
            fs::set_permissions(&open_to_others, fs::Permissions::from_mode(0o770)).unwrap();
            assert!(
                ClaimDir::open(&core, "c1").is_err(),
                "{}",
                open_to_others.display()
            );
            fs::set_permissions(&open_to_others, fs::Permissions::from_mode(0o700)).unwrap();
        }
        assert!(ClaimDir::open(&core, "c1").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn a_claim_file_with_another_link_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let core = tmp.path().join("core");
        let dir = core_folder(&core, "c1");
        fs::hard_link(dir.join("claim"), tmp.path().join("elsewhere")).unwrap();
        let claim = open(&core, "c1");
        assert!(!claim.claim_is_fresh().unwrap());
        assert!(claim.write_claim(b"v4lue").is_err());
        assert_eq!(fs::read(tmp.path().join("elsewhere")).unwrap(), b"");
    }

    #[cfg(unix)]
    #[test]
    fn links_are_not_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let core = tmp.path().join("core");
        let target = tmp.path().join("target");
        private_dir(&core.join(CLAIM_DIR));
        private_dir(&target);
        File::create(target.join("claim")).unwrap();
        // the claim's folder is a link elsewhere
        std::os::unix::fs::symlink(&target, core.join(CLAIM_DIR).join("c1")).unwrap();
        assert!(ClaimDir::open(&core, "c1").is_err());
        // local_claim itself is a link
        let other = tmp.path().join("other");
        fs::create_dir_all(&other).unwrap();
        std::os::unix::fs::symlink(core.join(CLAIM_DIR), other.join(CLAIM_DIR)).unwrap();
        assert!(ClaimDir::open(&other, "c1").is_err());
        // the claim file is a link
        let dir = core.join(CLAIM_DIR).join("c2");
        private_dir(&dir);
        std::os::unix::fs::symlink(target.join("claim"), dir.join("claim")).unwrap();
        let claim = open(&core, "c2");
        assert!(claim.claim_is_fresh().is_err());
        assert!(write_claim(&claim, &start("c2"), &OwnerCredentials::generate()).is_err());
        // a planted reply link isn't written through
        core_folder(&core, "c3");
        let dir = core.join(CLAIM_DIR).join("c3");
        std::os::unix::fs::symlink(target.join("elsewhere"), dir.join("reply")).unwrap();
        assert!(
            write_claim(
                &open(&core, "c3"),
                &start("c3"),
                &OwnerCredentials::generate()
            )
            .is_err()
        );
        assert!(!target.join("elsewhere").exists());
        // nor is result.json read through a link
        core_folder(&core, "c4");
        let dir = core.join(CLAIM_DIR).join("c4");
        fs::write(target.join("result.json"), "{}").unwrap();
        std::os::unix::fs::symlink(target.join("result.json"), dir.join("result.json")).unwrap();
        assert!(read_result(&open(&core, "c4")).is_err());
    }

    #[test]
    fn the_launcher_writes_its_side() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = core_folder(tmp.path(), "c1");
        let owner = OwnerCredentials::generate();
        let reply = write_claim(&open(tmp.path(), "c1"), &start("c1"), &owner).unwrap();
        assert_eq!(fs::read_to_string(dir.join("claim")).unwrap(), "v4lue");
        assert_eq!(fs::read_to_string(dir.join("reply")).unwrap(), reply);
        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join("owner.json")).unwrap()).unwrap();
        assert_eq!(written["username"], owner.username.as_str());
        assert_eq!(written["password"], owner.password.expose());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(dir.join("owner.json"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "owner.json is readable only by its user");
        }
    }

    /// Lodestone Host's first run: an owner without a password is `"password": null`.
    #[test]
    fn an_owner_without_a_password_is_written_as_null() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = core_folder(tmp.path(), "c1");
        let owner = OwnerCredentials::without_password("peter");
        assert!(!owner.has_password());
        assert!(OwnerCredentials::generate().has_password());
        write_claim(&open(tmp.path(), "c1"), &start("c1"), &owner).unwrap();
        let written: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join("owner.json")).unwrap()).unwrap();
        assert_eq!(written["username"], "peter");
        assert!(written["password"].is_null());
    }

    #[test]
    fn cleaning_up_deletes_only_the_claims_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = core_folder(tmp.path(), "c1");
        write_claim(
            &open(tmp.path(), "c1"),
            &start("c1"),
            &OwnerCredentials::generate(),
        )
        .unwrap();
        fs::write(dir.join("result.json"), "{}").unwrap();
        fs::write(dir.join("keep.txt"), "not the claim's").unwrap();
        open(tmp.path(), "c1").cleanup();
        assert!(dir.join("keep.txt").exists(), "an unknown file stays");
        for file in CLAIM_FILES {
            assert!(!dir.join(file).exists(), "{file}");
        }
        fs::remove_file(dir.join("keep.txt")).unwrap();
        open(tmp.path(), "c1").cleanup();
        assert!(!dir.exists(), "the empty folder goes");
        assert!(tmp.path().join(CLAIM_DIR).exists());
    }

    #[test]
    fn the_result_must_carry_the_launchers_reply_and_a_certificate() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = core_folder(tmp.path(), "c1");
        let claim = open(tmp.path(), "c1");
        assert!(read_result(&claim).unwrap().is_none(), "not written yet");
        let write = |fingerprint: serde_json::Value| {
            let result = serde_json::json!({
                "api_key": "lsk_k1_s3cret",
                "reply": "mine",
                "owner_created": true,
                "owner_username": "owner_ab12c",
                "tls_fingerprint_sha256": fingerprint,
            });
            fs::write(dir.join("result.json"), result.to_string()).unwrap();
        };
        write(FINGERPRINT.into());
        let result = read_result(&claim).unwrap().unwrap();
        assert_eq!(check_result(&result, "mine").unwrap(), FINGERPRINT);
        assert_eq!(
            check_result(&result, "other").err(),
            Some(ClaimError::NotThisCore)
        );
        write(serde_json::Value::Null);
        let result = read_result(&claim).unwrap().unwrap();
        assert_eq!(
            check_result(&result, "mine").err(),
            Some(ClaimError::NoCertificate)
        );
        write("AB:CD".into());
        let result = read_result(&claim).unwrap().unwrap();
        assert!(matches!(
            check_result(&result, "mine"),
            Err(ClaimError::Invalid(_))
        ));
        fs::write(dir.join("result.json"), "x".repeat(20_000)).unwrap();
        assert!(read_result(&claim).is_err(), "too large");
    }

    #[test]
    fn a_session_result_carries_a_token_and_no_key_is_needed() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = core_folder(tmp.path(), "c1");
        let claim = open(tmp.path(), "c1");
        let write = |result: serde_json::Value| {
            fs::write(dir.join("result.json"), result.to_string()).unwrap();
        };
        write(serde_json::json!({
            "session_token": "eyJhbGciOiJIUzUxMiJ9.e30.sig",
            "reply": "mine",
            "owner_created": false,
            "owner_username": "boss",
            "tls_fingerprint_sha256": FINGERPRINT,
        }));
        let result = read_result(&claim).unwrap().unwrap();
        assert_eq!(
            check_result_for(&result, "mine", true).unwrap(),
            FINGERPRINT
        );
        // a key claim needs its key
        assert!(matches!(
            check_result_for(&result, "mine", false),
            Err(ClaimError::Invalid(_))
        ));
        assert_eq!(
            check_result_for(&result, "other", true).err(),
            Some(ClaimError::NotThisCore)
        );
        // an older core answers a session claim with a key
        write(serde_json::json!({
            "api_key": "lsk_k1_s3cret",
            "reply": "mine",
            "owner_created": false,
            "owner_username": "boss",
            "tls_fingerprint_sha256": FINGERPRINT,
        }));
        let result = read_result(&claim).unwrap().unwrap();
        assert!(matches!(
            check_result_for(&result, "mine", true),
            Err(ClaimError::Invalid(_))
        ));
    }

    #[test]
    fn a_session_claim_writes_no_owner_file_without_an_owner() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = core_folder(tmp.path(), "c1");
        write_claim_with(&open(tmp.path(), "c1"), &start("c1"), None).unwrap();
        assert!(dir.join("claim").exists());
        assert!(dir.join("reply").exists());
        assert!(!dir.join("owner.json").exists());
    }

    #[test]
    fn the_key_is_tied_to_the_https_address_and_certificate() {
        assert_eq!(
            https_url(&Url::parse("http://127.0.0.1:16662/").unwrap())
                .unwrap()
                .as_str(),
            "https://127.0.0.1:16662/"
        );
        let entry = LocalCredentials {
            url: "https://127.0.0.1:16662".into(),
            tls_fingerprint: Some(FINGERPRINT.into()),
            api_key: Some("lsk_k1_s3cret".into()),
            ..Default::default()
        };
        let lower = FINGERPRINT.to_lowercase().replace(':', "");
        assert!(
            entry
                .key_for("https://127.0.0.1:16662", Some(&lower))
                .is_some()
        );
        // another address, plain HTTP, another certificate, or none: no key
        assert!(
            entry
                .key_for("https://127.0.0.1:16700", Some(FINGERPRINT))
                .is_none()
        );
        assert!(
            entry
                .key_for("http://127.0.0.1:16662", Some(FINGERPRINT))
                .is_none()
        );
        assert!(
            entry
                .key_for("https://127.0.0.1:16662", Some("AB"))
                .is_none()
        );
        assert!(entry.key_for("https://127.0.0.1:16662", None).is_none());
        let elsewhere = LocalCredentials {
            url: "https://192.168.1.5:16662".into(),
            ..entry.clone()
        };
        assert!(
            elsewhere
                .key_for("https://192.168.1.5:16662", Some(FINGERPRINT))
                .is_none()
        );
    }

    fn claimed(owner: &OwnerCredentials, tmp: &Path) -> Claimed {
        Claimed {
            info: CoreInfo {
                version: "0.6".into(),
                uuid: "core".into(),
                core_name: "Home".into(),
                is_setup: true,
                product: Default::default(),
                launched_by: Default::default(),
            },
            url: Url::parse("https://127.0.0.1:16662/").unwrap(),
            pin: CertificatePin::new("127.0.0.1", FINGERPRINT),
            api_key: Secret::new("lsk_k1_s3cret"),
            key_id: Some("k1".into()),
            folder: tmp.into(),
            owner_username: Some(owner.username.clone()),
            created_owner: Some(owner.clone()),
        }
    }

    #[test]
    fn credentials_round_trip_and_stay_private() {
        let tmp = tempfile::tempdir().unwrap();
        let path = credentials_path(tmp.path());
        assert_eq!(load_credentials(&path).unwrap(), CredentialsFile::default());
        let owner = OwnerCredentials::generate();
        let claimed = claimed(&owner, tmp.path());
        let mut file = CredentialsFile::default();
        file.record("core", &claimed);
        save_credentials(&path, &file).unwrap();
        let loaded = load_credentials(&path).unwrap();
        assert_eq!(loaded, file);
        let entry = &loaded.cores["core"];
        assert_eq!(entry.url, "https://127.0.0.1:16662");
        assert_eq!(entry.tls_fingerprint.as_deref(), Some(FINGERPRINT));
        assert_eq!(entry.api_key.as_deref(), Some("lsk_k1_s3cret"));
        assert_eq!(
            entry.owner_password.as_deref(),
            Some(owner.password.expose())
        );
        assert!(!format!("{entry:?}").contains("s3cret"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        // a later claim that made no owner keeps the account
        let again = Claimed {
            created_owner: None,
            api_key: Secret::new("lsk_k2_other"),
            ..claimed
        };
        file.record("core", &again);
        assert_eq!(
            file.cores["core"].owner_password.as_deref(),
            Some(owner.password.expose())
        );
        // signing out forgets the key, not the account
        assert!(file.forget_key("core"));
        assert_eq!(file.cores["core"].api_key, None);
        assert!(file.cores["core"].owner_password.is_some());
    }

    #[test]
    fn signing_out_forgets_an_entry_without_an_account() {
        let mut file = CredentialsFile::default();
        file.cores.insert(
            "core".into(),
            LocalCredentials {
                url: "https://127.0.0.1:16662".into(),
                api_key: Some("lsk_k_s".into()),
                ..Default::default()
            },
        );
        assert!(file.forget_key("core"));
        assert!(file.cores.is_empty());
        assert!(!file.forget_key("core"));
    }

    #[test]
    fn candidates_are_listed_once() {
        let tmp = tempfile::tempdir().unwrap();
        let chosen = tmp.path().to_path_buf();
        let folders = candidate_folders(&[chosen.clone(), chosen.clone()]);
        assert_eq!(folders.iter().filter(|f| **f == chosen).count(), 1);
        assert_eq!(folders[0], chosen);
    }

    /// A core that answers claims, playing its side on `root` (its data folder).
    struct FakeCore {
        root: PathBuf,
        /// What `result.json` says the launcher's reply was: `None` echoes it.
        forged_reply: Option<String>,
        /// The claim setting is off.
        refuse: bool,
        has_owner: bool,
        /// The core has no HTTPS certificate.
        no_certificate: bool,
    }

    impl http_client::HttpClient for FakeCore {
        fn user_agent(&self) -> Option<&http_client::http::HeaderValue> {
            None
        }

        fn proxy(&self) -> Option<&Url> {
            None
        }

        fn send(
            &self,
            req: http_client::http::Request<http_client::AsyncBody>,
        ) -> futures::future::BoxFuture<
            'static,
            anyhow::Result<http_client::http::Response<http_client::AsyncBody>>,
        > {
            let path = req.uri().path().to_string();
            let dir = self.root.join(CLAIM_DIR).join("c1");
            let (status, body) = match path.as_str() {
                "/api/v1/info" => (
                    200,
                    r#"{"version": "0.6.0", "uuid": "core-1", "core_name": "Home", "is_setup": true}"#
                        .to_string(),
                ),
                "/api/v1/local_claim/start" if self.refuse => (
                    403,
                    r#"{"kind": "PermissionDenied", "causes": ["off"]}"#.to_string(),
                ),
                "/api/v1/local_claim/start" => {
                    core_folder(&self.root, "c1");
                    (
                        200,
                        r#"{"id": "c1", "dir": "local_claim/c1", "value": "v4lue", "core_uuid": "core-1", "expires_in": 60}"#
                            .to_string(),
                    )
                }
                "/api/v1/local_claim/finish" => {
                    assert_eq!(fs::read_to_string(dir.join("claim")).unwrap(), "v4lue");
                    let reply = fs::read_to_string(dir.join("reply")).unwrap();
                    let owner: serde_json::Value =
                        serde_json::from_str(&fs::read_to_string(dir.join("owner.json")).unwrap())
                            .unwrap();
                    let username = if self.has_owner {
                        "alice".to_string()
                    } else {
                        owner["username"].as_str().unwrap().to_string()
                    };
                    let result = serde_json::json!({
                        "api_key": "lsk_k1_s3cret",
                        "reply": self.forged_reply.clone().unwrap_or(reply),
                        "owner_created": !self.has_owner,
                        "owner_username": username,
                        "tls_fingerprint_sha256": (!self.no_certificate).then_some(FINGERPRINT),
                    });
                    fs::write(dir.join("result.json"), result.to_string()).unwrap();
                    (200, r#"{"ok": true}"#.to_string())
                }
                other => panic!("unexpected request {other}"),
            };
            let response = http_client::http::Response::builder()
                .status(status)
                .body(http_client::AsyncBody::from(body))
                .unwrap();
            Box::pin(async move { Ok(response) })
        }
    }

    fn run_claim(core: FakeCore) -> (Result<Unverified, ClaimError>, PathBuf) {
        let root = core.root.clone();
        let core = Arc::new(core);
        let client = CoreClient::new(core, Url::parse("http://127.0.0.1:16662").unwrap());
        let (sleep, _) = crate::game::download::recording_sleep();
        let elsewhere = root.parent().unwrap().join("elsewhere");
        let result = futures::executor::block_on(claim_unverified(
            &client,
            &[elsewhere, root.clone()],
            "Lodestone Launcher on test",
            sleep,
        ));
        (result, root)
    }

    fn fake(root: &Path) -> FakeCore {
        FakeCore {
            root: root.to_path_buf(),
            forged_reply: None,
            refuse: false,
            has_owner: false,
            no_certificate: false,
        }
    }

    #[test]
    fn a_claim_on_a_new_core_makes_the_owner_and_gets_a_key() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("lodestone");
        let (result, root) = run_claim(fake(&root));
        let claimed = result.unwrap();
        assert_eq!(claimed.info.uuid, "core-1");
        assert_eq!(claimed.api_key.expose(), "lsk_k1_s3cret");
        assert_eq!(claimed.fingerprint, FINGERPRINT);
        assert_eq!(claimed.folder, root);
        let owner = claimed.created_owner.expect("the launcher made the owner");
        assert_eq!(
            claimed.owner_username.as_deref(),
            Some(owner.username.as_str())
        );
        assert!(
            !root.join(CLAIM_DIR).join("c1").exists(),
            "the claim's folder, with the password, is removed"
        );
    }

    #[test]
    fn a_claim_on_a_core_with_an_owner_only_gets_a_key() {
        let tmp = tempfile::tempdir().unwrap();
        let mut core = fake(&tmp.path().join("lodestone"));
        core.has_owner = true;
        let claimed = run_claim(core).0.unwrap();
        assert!(claimed.created_owner.is_none());
        assert_eq!(claimed.owner_username.as_deref(), Some("alice"));
    }

    #[test]
    fn a_result_without_the_launchers_reply_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let mut core = fake(&tmp.path().join("lodestone"));
        core.forged_reply = Some("someone else".into());
        let (result, root) = run_claim(core);
        assert_eq!(result.err(), Some(ClaimError::NotThisCore));
        assert!(!root.join(CLAIM_DIR).join("c1").exists());
    }

    #[test]
    fn a_core_without_a_certificate_is_refused_and_its_files_go() {
        let tmp = tempfile::tempdir().unwrap();
        let mut core = fake(&tmp.path().join("lodestone"));
        core.no_certificate = true;
        let (result, root) = run_claim(core);
        assert_eq!(result.err(), Some(ClaimError::NoCertificate));
        let dir = root.join(CLAIM_DIR).join("c1");
        assert!(
            !dir.join("result.json").exists(),
            "the key's file is deleted"
        );
        assert!(
            !dir.join("owner.json").exists(),
            "the password's file is deleted"
        );
    }

    #[test]
    fn a_core_that_refuses_says_so() {
        let tmp = tempfile::tempdir().unwrap();
        let mut core = fake(&tmp.path().join("lodestone"));
        core.refuse = true;
        assert_eq!(run_claim(core).0.err(), Some(ClaimError::Refused));
    }

    #[test]
    fn cores_say_what_they_are() {
        let info = |json: &str| serde_json::from_str::<CoreInfo>(json).unwrap().product;
        let base = r#""version": "1", "uuid": "u", "core_name": "Home""#;
        assert_eq!(
            info(&format!("{{{base}}}")),
            super::super::CoreProduct::Core
        );
        assert_eq!(
            info(&format!(r#"{{{base}, "product": "host"}}"#)),
            super::super::CoreProduct::Host
        );
        assert_eq!(
            info(&format!(r#"{{{base}, "product": "something-new"}}"#)),
            super::super::CoreProduct::Core
        );
        let started = |json: &str| serde_json::from_str::<CoreInfo>(json).unwrap().launched_by;
        use super::super::LaunchedBy;
        assert_eq!(started(&format!("{{{base}}}")), LaunchedBy::Unknown);
        for (value, expected) in [
            ("host", LaunchedBy::Host),
            ("lodestone_cli", LaunchedBy::LodestoneCli),
            ("docker", LaunchedBy::Docker),
            ("direct", LaunchedBy::Direct),
            ("systemd", LaunchedBy::Unknown),
        ] {
            assert_eq!(
                started(&format!(r#"{{{base}, "launched_by": "{value}"}}"#)),
                expected,
                "{value}"
            );
        }
    }

    #[test]
    fn both_loopback_addresses_find_one_core_once() {
        let tmp = tempfile::tempdir().unwrap();
        let core = Arc::new(fake(tmp.path()));
        let found = futures::executor::block_on(discover(core));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1.uuid, "core-1");
        assert_eq!(found[0].0.as_str(), "http://127.0.0.1:16662/");
    }
}
