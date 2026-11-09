//! "Connect as owner" for a Lodestone core on this computer: the launcher proves it can
//! write the core's data folder (`LODESTONE_PATH`), and the core gives it an owner-level
//! API key. Anyone who can write that folder already controls the core (it holds the
//! users, their token secrets and the API keys), so this grants nothing new.
//!
//! 1. `POST /local_claim/start` (no sign-in): the core makes `<LODESTONE_PATH>/
//!    local_claim/<id>/`, which only its user can open, with an empty `claim` file, and
//!    answers the folder (relative to its data folder), a value and its id.
//! 2. The launcher finds which of its candidate folders ([`candidate_folders`]) holds
//!    that `claim` file, and writes into `<id>/`: the value into `claim`, a random value
//!    of its own into `reply`, and `owner.json`, an owner account it made up
//!    ([`OwnerCredentials`]), which the core uses only when it has no owner yet.
//! 3. `POST /local_claim/finish` with the id. The core checks `claim`, makes the owner
//!    if asked, mints the key and writes it, with the `reply` value, into
//!    `result.json` in the same folder. Nothing secret crosses the network.
//! 4. The launcher reads `result.json`. Its own `reply` value there proves that the
//!    server on the port is the core behind that folder: nothing without the folder
//!    could have read it.
//!
//! A core in Docker works the same when its data folder is a folder of this computer
//! mounted into the container: the launcher only ever touches files inside it. A volume
//! Docker keeps to itself can't be found, and the claim says so.
//!
//! What the launcher gets is kept in plain text in `<data>/cores/
//! local_credentials.json`, readable only by this user ([`save_credentials`]). It
//! exists only because this user can already write the core's folder, so a copy next
//! to the launcher gives nothing away that the folder didn't.

use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read as _, Write as _},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use url::Url;

use super::{CoreClient, CoreError, CoreInfo, client};
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
    /// The answer in `result.json` isn't the launcher's: the server on the port isn't
    /// the core behind that folder.
    #[error("The core that answered isn't the one whose folder was found, so nothing was saved.")]
    NotThisCore,
    /// The core didn't write `result.json` in time.
    #[error("The core didn't finish the claim. Try again.")]
    NoResult,
    #[error("{0}")]
    Core(CoreError),
    #[error("Couldn't use the core's data folder: {0}")]
    Files(String),
    #[error("The core sent something Lodestone Launcher doesn't understand: {0}")]
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
}

/// `result.json`. No `Debug`: it holds the key.
#[derive(Deserialize)]
struct ClaimResult {
    api_key: String,
    reply: String,
    #[serde(default)]
    owner_created: bool,
    #[serde(default)]
    owner_username: Option<String>,
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
}

/// What a claim gave.
pub struct Claimed {
    pub info: CoreInfo,
    pub url: Url,
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

/// `dir` from the core, checked to be `local_claim/<id>` for this claim's id, so the
/// launcher never writes outside that folder.
fn claim_subdir(start: &ClaimStart) -> Result<PathBuf, ClaimError> {
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
    Ok(Path::new(CLAIM_DIR).join(&start.id))
}

/// Whether `path` is a real folder (not a link to one).
fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
}

/// Whether `path` is a regular file (not a link to one).
fn is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file())
}

/// The claim's folder in the first of `folders` that holds the empty `claim` file the
/// core just made. Links are not followed on the way down.
pub fn find_claim_folder(folders: &[PathBuf], start: &ClaimStart) -> Result<PathBuf, ClaimError> {
    let sub = claim_subdir(start)?;
    folders
        .iter()
        .map(|folder| folder.join(&sub))
        .find(|dir| {
            dir.parent().is_some_and(is_real_dir)
                && is_real_dir(dir)
                && is_regular_file(&dir.join("claim"))
                && fs::symlink_metadata(dir.join("claim")).is_ok_and(|m| m.len() == 0)
        })
        .ok_or(ClaimError::FolderNotFound)
}

/// Opens a file for writing without following a link at its name.
fn open_for_write(path: &Path, create_new: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true);
    if create_new {
        options.create_new(true);
    } else {
        options.truncate(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    options.open(path)
}

/// Writes the launcher's side of the claim into `dir`: the value into the existing
/// `claim`, a new `reply` and `owner.json`. Returns the reply value.
pub fn write_claim(
    dir: &Path,
    start: &ClaimStart,
    owner: &OwnerCredentials,
) -> Result<String, ClaimError> {
    let claim = dir.join("claim");
    if !is_regular_file(&claim) {
        return Err(ClaimError::FolderNotFound);
    }
    open_for_write(&claim, false)?.write_all(start.value.as_bytes())?;
    let reply = random_string(LOWER_ALNUM, 32);
    open_for_write(&dir.join("reply"), true)?.write_all(reply.as_bytes())?;
    let owner_json = serde_json::json!({
        "username": owner.username,
        "password": owner.password.expose(),
    });
    open_for_write(&dir.join("owner.json"), true)?.write_all(owner_json.to_string().as_bytes())?;
    Ok(reply)
}

/// `result.json` in `dir`, once it is there: a regular file of a sensible size.
fn read_result(dir: &Path) -> Result<Option<ClaimResult>, ClaimError> {
    let path = dir.join("result.json");
    let Ok(meta) = fs::symlink_metadata(&path) else {
        return Ok(None);
    };
    if !meta.is_file() {
        return Err(ClaimError::Files(
            "result.json in the claim's folder isn't a file".into(),
        ));
    }
    if meta.len() > MAX_RESULT_BYTES {
        return Err(ClaimError::Invalid("result.json is too large".into()));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut text = String::new();
    options
        .open(&path)?
        .take(MAX_RESULT_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.trim().is_empty() {
        // written but not filled yet
        return Ok(None);
    }
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|err| ClaimError::Invalid(format!("result.json: {err}")))
}

/// Checks the result against the launcher's reply value.
fn check_result(result: &ClaimResult, reply: &str) -> Result<(), ClaimError> {
    if !subtle_eq(result.reply.as_bytes(), reply.as_bytes()) {
        return Err(ClaimError::NotThisCore);
    }
    if key_id(&result.api_key).is_none() {
        return Err(ClaimError::Invalid(
            "the key isn't a Lodestone API key".into(),
        ));
    }
    Ok(())
}

/// Compares in constant time (for the length of the shorter input).
fn subtle_eq(a: &[u8], b: &[u8]) -> bool {
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
/// `folders` ([`candidate_folders`]). `name` names the key: "Lodestone Launcher on …".
pub async fn claim(
    client: &CoreClient,
    folders: &[PathBuf],
    name: &str,
    sleep: Sleep,
) -> Result<Claimed, ClaimError> {
    let info = client.info().await.map_err(|err| match err {
        CoreError::Unreachable(_) => ClaimError::NoCore,
        err => ClaimError::Core(err),
    })?;
    let start = client.local_claim_start().await.map_err(refused)?;
    if start.core_uuid != info.uuid {
        return Err(ClaimError::NotThisCore);
    }
    let dir = find_claim_folder(folders, &start)?;
    let owner = OwnerCredentials::generate();
    let outcome = finish(client, &dir, &start, &owner, name, sleep).await;
    // the folder held the owner's password: it goes, whatever happened
    let _ = fs::remove_dir_all(&dir);
    let result = outcome?;
    let folder = dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_default();
    Ok(Claimed {
        url: client.base().clone(),
        key_id: key_id(&result.api_key),
        api_key: Secret::new(result.api_key),
        folder,
        owner_username: result.owner_username,
        created_owner: result.owner_created.then_some(owner),
        info,
    })
}

async fn finish(
    client: &CoreClient,
    dir: &Path,
    start: &ClaimStart,
    owner: &OwnerCredentials,
    name: &str,
    sleep: Sleep,
) -> Result<ClaimResult, ClaimError> {
    let reply = write_claim(dir, start, owner)?;
    client
        .local_claim_finish(&start.id, name)
        .await
        .map_err(refused)?;
    let mut waited = Duration::ZERO;
    loop {
        if let Some(result) = read_result(dir)? {
            check_result(&result, &reply)?;
            return Ok(result);
        }
        if waited >= RESULT_WAIT {
            return Err(ClaimError::NoResult);
        }
        sleep(RESULT_POLL).await;
        waited += RESULT_POLL;
    }
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
    pub url: String,
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
        entry.url = claimed.url.as_str().trim_end_matches('/').to_string();
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
    use super::*;

    fn start(id: &str) -> ClaimStart {
        ClaimStart {
            id: id.into(),
            dir: format!("{CLAIM_DIR}/{id}"),
            value: "v4lue".into(),
            core_uuid: "core".into(),
            expires_in: Some(60),
        }
    }

    /// A data folder with the claim the core makes.
    fn core_folder(root: &Path, id: &str) -> PathBuf {
        let dir = root.join(CLAIM_DIR).join(id);
        fs::create_dir_all(&dir).unwrap();
        File::create(dir.join("claim")).unwrap();
        dir
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
        let found = find_claim_folder(&[elsewhere.clone(), core.clone()], &start("c1")).unwrap();
        assert_eq!(found, dir);
        assert_eq!(
            find_claim_folder(&[elsewhere], &start("c1")),
            Err(ClaimError::FolderNotFound)
        );
        // a claim that was already answered isn't this one
        fs::write(dir.join("claim"), "taken").unwrap();
        assert_eq!(
            find_claim_folder(&[core], &start("c1")),
            Err(ClaimError::FolderNotFound)
        );
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
        assert!(matches!(claim_subdir(&bad), Err(ClaimError::Invalid(_))));
    }

    #[cfg(unix)]
    #[test]
    fn links_are_not_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let core = tmp.path().join("core");
        let target = tmp.path().join("target");
        fs::create_dir_all(core.join(CLAIM_DIR)).unwrap();
        fs::create_dir_all(&target).unwrap();
        File::create(target.join("claim")).unwrap();
        // the claim's folder is a link elsewhere
        std::os::unix::fs::symlink(&target, core.join(CLAIM_DIR).join("c1")).unwrap();
        assert_eq!(
            find_claim_folder(std::slice::from_ref(&core), &start("c1")),
            Err(ClaimError::FolderNotFound)
        );
        // the claim file is a link
        let dir = core.join(CLAIM_DIR).join("c2");
        fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink(target.join("claim"), dir.join("claim")).unwrap();
        assert_eq!(
            find_claim_folder(std::slice::from_ref(&core), &start("c2")),
            Err(ClaimError::FolderNotFound)
        );
        assert!(write_claim(&dir, &start("c2"), &OwnerCredentials::generate()).is_err());
        // a planted reply link isn't written through
        let dir = core_folder(&core, "c3");
        std::os::unix::fs::symlink(target.join("elsewhere"), dir.join("reply")).unwrap();
        assert!(write_claim(&dir, &start("c3"), &OwnerCredentials::generate()).is_err());
        assert!(!target.join("elsewhere").exists());
        // nor is result.json read through a link
        let dir = core_folder(&core, "c4");
        fs::write(target.join("result.json"), "{}").unwrap();
        std::os::unix::fs::symlink(target.join("result.json"), dir.join("result.json")).unwrap();
        assert!(read_result(&dir).is_err());
    }

    #[test]
    fn the_launcher_writes_its_side() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = core_folder(tmp.path(), "c1");
        let owner = OwnerCredentials::generate();
        let reply = write_claim(&dir, &start("c1"), &owner).unwrap();
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

    #[test]
    fn the_result_must_carry_the_launchers_reply() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = core_folder(tmp.path(), "c1");
        assert!(read_result(&dir).unwrap().is_none(), "not written yet");
        fs::write(
            dir.join("result.json"),
            r#"{"api_key": "lsk_k1_s3cret", "reply": "mine", "owner_created": true, "owner_username": "owner_ab12c"}"#,
        )
        .unwrap();
        let result = read_result(&dir).unwrap().unwrap();
        assert!(check_result(&result, "mine").is_ok());
        assert_eq!(check_result(&result, "other"), Err(ClaimError::NotThisCore));
        fs::write(dir.join("result.json"), "x".repeat(20_000)).unwrap();
        assert!(read_result(&dir).is_err(), "too large");
    }

    #[test]
    fn credentials_round_trip_and_stay_private() {
        let tmp = tempfile::tempdir().unwrap();
        let path = credentials_path(tmp.path());
        assert_eq!(load_credentials(&path).unwrap(), CredentialsFile::default());
        let owner = OwnerCredentials::generate();
        let claimed = Claimed {
            info: CoreInfo {
                version: "0.6".into(),
                uuid: "core".into(),
                core_name: "Home".into(),
                is_setup: true,
                product: Default::default(),
                launched_by: Default::default(),
            },
            url: Url::parse("http://127.0.0.1:16662/").unwrap(),
            api_key: Secret::new("lsk_k1_s3cret"),
            key_id: Some("k1".into()),
            folder: tmp.path().into(),
            owner_username: Some(owner.username.clone()),
            created_owner: Some(owner.clone()),
        };
        let mut file = CredentialsFile::default();
        file.record("core", &claimed);
        save_credentials(&path, &file).unwrap();
        let loaded = load_credentials(&path).unwrap();
        assert_eq!(loaded, file);
        let entry = &loaded.cores["core"];
        assert_eq!(entry.url, "http://127.0.0.1:16662");
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
                url: "http://127.0.0.1:16662".into(),
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
        made_owner: std::sync::Mutex<Option<String>>,
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
                    fs::create_dir_all(&dir).unwrap();
                    File::create(dir.join("claim")).unwrap();
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
                        let name = owner["username"].as_str().unwrap().to_string();
                        *self.made_owner.lock().unwrap() = Some(name.clone());
                        name
                    };
                    let result = serde_json::json!({
                        "api_key": "lsk_k1_s3cret",
                        "reply": self.forged_reply.clone().unwrap_or(reply),
                        "owner_created": !self.has_owner,
                        "owner_username": username,
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

    fn run_claim(core: FakeCore) -> (Result<Claimed, ClaimError>, PathBuf) {
        let root = core.root.clone();
        let core = Arc::new(core);
        let client = CoreClient::new(core, Url::parse("http://127.0.0.1:16662").unwrap());
        let (sleep, _) = crate::game::download::recording_sleep();
        let elsewhere = root.parent().unwrap().join("elsewhere");
        let result = futures::executor::block_on(claim(
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
            made_owner: Default::default(),
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
        assert_eq!(claimed.key_id.as_deref(), Some("k1"));
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
