//! Owner access for programs on this computer (Lodestone Launcher), proven through the
//! core's data folder instead of a password.
//!
//! Anything that can write to the data folder already controls the core: `stores/`
//! holds every user's token secret. So a program that proves it can write there may
//! have owner access, and none of the secrets cross the network:
//!
//! 1. `POST /local_claim/start` creates `local_claim/<id>/` (0700) with an empty `claim`
//!    file and returns the id, the folder (relative to the data folder, since a core in
//!    Docker sees another path than its host) and a value.
//! 2. The program writes the value into `claim`, a random value of its own into `reply`
//!    and, for a core without an owner yet, `owner.json` (`{"username", "password"}`).
//! 3. `POST /local_claim/finish` checks `claim`, creates the owner if asked, mints an
//!    owner API key and writes `result.json` (0600): the key and the program's `reply`.
//!    The program reads it there; finding its own `reply` in it proves that the server
//!    it talked to is the core behind that folder.
//!
//! One claim waits at a time and lasts [`CLAIM_TTL`]; a new start replaces it. Starts
//! are limited to [`MAX_STARTS_PER_MINUTE`]. Every failure uses the claim up. The
//! `allow_local_claim` setting turns it off. The folder is never reachable through the
//! file API.

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use axum::{extract::State, routing::post, Json, Router};
use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use tracing::{info, warn};

use crate::{
    auth::{api_key::ApiKeyScope, permission::UserPermission, user::User},
    error::{Error, ErrorKind},
    events::CausedBy,
    util::rand_alphanumeric,
    AppState,
};

/// The folder in the data folder that claims are made in.
pub const CLAIM_DIR: &str = "local_claim";
/// How long a claim waits for `finish`.
pub const CLAIM_TTL: Duration = Duration::from_secs(60);
/// How long a claim's folder (and an uncollected `result.json`) stays after `finish`.
pub const RESULT_TTL: Duration = Duration::from_secs(5 * 60);
/// How many claims may start in a minute.
pub const MAX_STARTS_PER_MINUTE: usize = 10;
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
}

/// The body of `POST /local_claim/finish`.
#[derive(Serialize, Deserialize, Debug)]
pub struct FinishRequest {
    pub id: String,
    /// The API key's name, such as "Lodestone Launcher on Peter's Mac".
    pub name: String,
}

/// `<dir>/result.json`, written by `finish`. Holds the key, so it is never sent over the
/// network and the file is readable only by the core's OS user.
#[derive(Serialize, Deserialize)]
pub struct ClaimResult {
    pub api_key: String,
    /// The program's `reply`, read from the folder.
    pub reply: String,
    pub owner_created: bool,
    pub owner_username: String,
}

/// `<dir>/owner.json`, written by the program for a core without an owner.
#[derive(Deserialize)]
struct OwnerFile {
    username: String,
    password: String,
}

struct Pending {
    id: String,
    value: String,
    started: Instant,
}

#[derive(Default)]
struct ClaimState {
    pending: Option<Pending>,
    /// When recent claims started, oldest first, for the rate limit.
    starts: VecDeque<Instant>,
}

/// The claim in progress, and the folder claims are made in.
pub struct LocalClaims {
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
            dir: data_dir.join(CLAIM_DIR),
            state: Default::default(),
        }
    }

    /// Removes whatever claims a previous run left. Call it at startup, before serving.
    pub async fn clean_up(&self) {
        match tokio::fs::remove_dir_all(&self.dir).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => warn!("Failed to remove {}: {e}", self.dir.display()),
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

    /// Removes claim folders older than [`RESULT_TTL`], apart from `keep`.
    async fn sweep(&self, keep: Option<&str>) {
        let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await else {
            return;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().into_owned();
            if Some(name.as_str()) == keep {
                continue;
            }
            let old = entry
                .metadata()
                .await
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_none_or(|age| age >= RESULT_TTL);
            if old {
                let _ = tokio::fs::remove_dir_all(entry.path()).await;
            }
        }
    }

    /// Starts a claim: replaces any waiting one, creates its folder and empty `claim`.
    pub async fn start(
        &self,
        core_uuid: &str,
        product: crate::product::CoreProduct,
        launched_by: crate::product::LaunchedBy,
    ) -> Result<StartReply, Error> {
        let mut state = self.state.lock().await;
        let now = Instant::now();
        while state
            .starts
            .front()
            .is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(60))
        {
            state.starts.pop_front();
        }
        if state.starts.len() >= MAX_STARTS_PER_MINUTE {
            return Err(Error {
                kind: ErrorKind::TooManyRequests,
                source: eyre!("Too many claims started. Try again in a minute"),
            });
        }
        state.starts.push_back(now);
        if let Some(old) = state.pending.take() {
            self.remove_claim(&old.id).await;
        }
        self.sweep(None).await;

        let id = rand_alphanumeric(ID_LEN);
        let value = rand_alphanumeric(VALUE_LEN);
        let dir = self.claim_dir(&id);
        create_private_dir(&self.dir).await?;
        create_private_dir(&dir).await?;
        create_private_file(&dir.join("claim"), b"").await?;
        state.pending = Some(Pending {
            id: id.clone(),
            value: value.clone(),
            started: now,
        });
        info!("A program on this computer started claiming owner access");
        Ok(StartReply {
            dir: format!("{CLAIM_DIR}/{id}"),
            id,
            value,
            core_uuid: core_uuid.to_string(),
            product,
            launched_by,
            expires_in: CLAIM_TTL.as_secs(),
        })
    }

    /// Checks the claim `id` and, when it holds, writes the owner key into its folder.
    pub async fn finish(&self, app: &AppState, request: FinishRequest) -> Result<(), Error> {
        let mut state = self.state.lock().await;
        // An id that isn't the waiting claim's leaves the claim alone: ids can't be
        // guessed, so otherwise anyone on the network could cancel a claim in progress.
        let Some(pending) = state
            .pending
            .take_if(|pending| pending.id.as_bytes().ct_eq(request.id.as_bytes()).into())
        else {
            return Err(denied("No such claim. Start a new one"));
        };
        drop(state);
        let dir = self.claim_dir(&pending.id);
        let result = self
            .check_and_grant(app, &pending, &dir, &request.name)
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
    ) -> Result<(), Error> {
        if pending.started.elapsed() >= CLAIM_TTL {
            return Err(denied("The claim has expired. Start a new one"));
        }
        let claim = read_small(&dir.join("claim"), MAX_SMALL_FILE)
            .await?
            .ok_or_else(|| denied("The claim file is missing"))?;
        if !bool::from(claim.trim().as_bytes().ct_eq(pending.value.as_bytes())) {
            return Err(denied("The claim file doesn't hold the claim's value"));
        }
        let reply = read_small(&dir.join("reply"), MAX_SMALL_FILE)
            .await?
            .ok_or_else(|| denied("The reply file is missing"))?;
        let reply = reply.trim().to_string();
        if !(16..=256).contains(&reply.len()) || !reply.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(denied("The reply must be 16 to 256 letters and digits"));
        }
        let owner_file = read_small(&dir.join("owner.json"), MAX_OWNER_FILE).await?;

        let (owner, owner_created) = match current_owner(app).await {
            Some(owner) => (owner, false),
            None => {
                let Some(text) = owner_file else {
                    return Err(denied(
                        "This core has no owner yet: write owner.json with the owner's \
                         username and password",
                    ));
                };
                (create_owner(app, &text).await?, true)
            }
        };
        let name = key_name(name);
        let created = app
            .users_manager
            .write()
            .await
            .create_api_key(
                &owner.uid,
                name.clone(),
                None,
                ApiKeyScope::Full,
                CausedBy::System,
            )
            .await?;
        let result = ClaimResult {
            api_key: created.key,
            reply,
            owner_created,
            owner_username: owner.username.clone(),
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
        info!("{name} was given owner access through the data folder (API key `{name}`)");
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
/// first-time setup key.
async fn create_owner(app: &AppState, text: &str) -> Result<User, Error> {
    let owner: OwnerFile =
        serde_json::from_str(text).map_err(|e| denied(format!("owner.json isn't valid: {e}")))?;
    let username = owner.username.trim().to_string();
    let chars = username.chars().count();
    if chars == 0 || chars > 64 || username.chars().any(char::is_control) {
        return Err(denied("The owner's username must have 1 to 64 characters"));
    }
    if owner.password.chars().count() < 12 || owner.password.len() > 256 {
        return Err(denied(
            "The owner's password must have 12 to 256 characters",
        ));
    }
    // held until the owner is saved, as in setup_owner
    let mut setup_key = app.first_time_setup_key.lock().await;
    if current_owner(app).await.is_some() {
        return Err(denied("This core already has an owner"));
    }
    let password = owner.password;
    let user = tokio::task::spawn_blocking(move || {
        User::new(username, &password, true, false, UserPermission::default())
    })
    .await
    .context("Password hashing task failed")?;
    app.users_manager
        .write()
        .await
        .add_user(user.clone(), CausedBy::System)
        .await?;
    setup_key.take();
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

/// The text of `path`, `None` if it doesn't exist. Refuses a symbolic link, anything but
/// a regular file, a file larger than `max` and text that isn't UTF-8.
async fn read_small(path: &Path, max: u64) -> Result<Option<String>, Error> {
    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(denied(format!("Failed to read {}: {e}", file_name(path)))),
    };
    if !metadata.file_type().is_file() {
        return Err(denied(format!("{} isn't a plain file", file_name(path))));
    }
    if metadata.len() > max {
        return Err(denied(format!("{} is too large", file_name(path))));
    }
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| denied(format!("Failed to read {}: {e}", file_name(path))))?;
    if bytes.len() as u64 > max {
        return Err(denied(format!("{} is too large", file_name(path))));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| denied(format!("{} isn't text", file_name(path))))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Creates `dir` if needed, readable only by the core's OS user.
async fn create_private_dir(dir: &Path) -> Result<(), Error> {
    match tokio::fs::create_dir(dir).await {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let metadata = tokio::fs::symlink_metadata(dir)
                .await
                .context(format!("Failed to check {}", dir.display()))?;
            if !metadata.is_dir() {
                return Err(Error {
                    kind: ErrorKind::Internal,
                    source: eyre!("{} isn't a folder", dir.display()),
                });
            }
        }
        Err(e) => {
            return Err(Error {
                kind: ErrorKind::Internal,
                source: eyre!("Failed to create {}: {e}", dir.display()),
            })
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
            .await
            .context(format!("Failed to restrict {}", dir.display()))?;
    }
    Ok(())
}

/// Creates the new file `path` with `contents`, readable only by the core's OS user.
async fn create_private_file(path: &Path, contents: &[u8]) -> Result<(), Error> {
    use tokio::io::AsyncWriteExt;
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(path)
        .await
        .context(format!("Failed to create {}", path.display()))?;
    file.write_all(contents)
        .await
        .context(format!("Failed to write {}", path.display()))?;
    Ok(())
}

async fn start_claim(State(state): State<AppState>) -> Result<Json<StartReply>, Error> {
    if !state.global_settings.lock().await.allow_local_claim() {
        return Err(denied(
            "Claiming owner access from this computer is turned off in Core Settings",
        ));
    }
    Ok(Json(
        state
            .local_claims
            .start(&state.uuid, state.product, state.launched_by)
            .await?,
    ))
}

async fn finish_claim(
    State(state): State<AppState>,
    Json(request): Json<FinishRequest>,
) -> Result<Json<serde_json::Value>, Error> {
    if !state.global_settings.lock().await.allow_local_claim() {
        return Err(denied(
            "Claiming owner access from this computer is turned off in Core Settings",
        ));
    }
    state.local_claims.finish(&state, request).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// `POST /local_claim/start` and `POST /local_claim/finish`. No sign-in: the proof is the
/// data folder.
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
        test_util::{serve_api, test_app_state},
    };
    use reqwest::StatusCode;
    use serde_json::{json, Value};

    struct Core {
        dir: tempfile::TempDir,
        state: AppState,
        base: String,
        client: reqwest::Client,
    }

    async fn core_with(settings: GlobalSettingsData, owner: bool) -> Core {
        crate::test_util::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        let state = test_app_state(dir.path(), settings).await;
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

        async fn finish(&self, id: &str) -> StatusCode {
            self.post(
                "/local_claim/finish",
                json!({ "id": id, "name": "Lodestone Launcher on Test\u{7}Mac" }),
            )
            .await
            .0
        }

        fn result(&self, start: &StartReply) -> ClaimResult {
            let text = std::fs::read_to_string(self.folder(start).join("result.json")).unwrap();
            serde_json::from_str(&text).unwrap()
        }
    }

    #[tokio::test]
    async fn an_existing_core_gives_an_owner_key_through_the_folder() {
        let core = core(true).await;
        let start = core.start().await;
        assert_eq!(start.id.len(), ID_LEN);
        assert_eq!(start.value.len(), VALUE_LEN);
        assert_eq!(start.dir, format!("local_claim/{}", start.id));
        assert_eq!(start.core_uuid, core.state.uuid);
        assert_eq!(start.product, crate::product::CoreProduct::Core);
        assert_eq!(start.launched_by, crate::product::LaunchedBy::Direct);
        assert_eq!(start.expires_in, 60);
        let folder = core.folder(&start);
        assert_eq!(std::fs::read(folder.join("claim")).unwrap(), b"");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
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
        let (status, body) = core
            .post(
                "/local_claim/finish",
                json!({ "id": start.id, "name": "Lodestone Launcher on Test\u{7}Mac" }),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        // nothing secret over the network
        assert_eq!(body, json!({ "ok": true }));

        let result = core.result(&start);
        assert_eq!(result.reply, "ReplyFromTheLauncher123");
        assert!(!result.owner_created);
        assert_eq!(result.owner_username, "boss");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(folder.join("result.json"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        for gone in ["claim", "reply", "owner.json"] {
            assert!(!folder.join(gone).exists(), "{gone}");
        }
        let users = core.state.users_manager.read().await;
        let user = users.try_auth(&result.api_key).unwrap();
        assert!(user.is_owner);
        assert_eq!(user.username, "boss");
        let keys = users.list_api_keys(&user.uid);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "Lodestone Launcher on TestMac");
        assert_eq!(keys[0].expires_at, None);
        assert_eq!(keys[0].scope, ApiKeyScope::Full);
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
        assert_eq!(core.finish(&start.id).await, StatusCode::OK);
        let result = core.result(&start);
        assert!(result.owner_created);
        assert_eq!(result.owner_username, "owner_x7k2q");
        assert!(core.state.first_time_setup_key.lock().await.is_none());
        let users = core.state.users_manager.read().await;
        let owner = users.get_user_by_username("owner_x7k2q").unwrap();
        assert!(owner.is_owner);
        assert!(users.try_auth(&result.api_key).unwrap().is_owner);
    }

    #[tokio::test]
    async fn a_new_core_without_owner_json_refuses() {
        let core = core(false).await;
        let start = core.start().await;
        core.answer(&start, &start.value);
        assert_eq!(core.finish(&start.id).await, StatusCode::FORBIDDEN);
        assert!(!core.folder(&start).exists());
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
        assert_eq!(core.finish(&start.id).await, StatusCode::FORBIDDEN);
        assert!(core.state.first_time_setup_key.lock().await.is_some());
    }

    #[tokio::test]
    async fn a_wrong_value_is_refused_and_uses_the_claim_up() {
        let core = core(true).await;
        let start = core.start().await;
        core.answer(&start, "not the value");
        assert_eq!(core.finish(&start.id).await, StatusCode::FORBIDDEN);
        assert!(!core.folder(&start).exists());
        // used up: the right value doesn't help now
        assert_eq!(core.finish(&start.id).await, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn an_untouched_claim_file_is_refused() {
        let core = core(true).await;
        let start = core.start().await;
        std::fs::write(core.folder(&start).join("reply"), "ReplyFromTheLauncher123").unwrap();
        assert_eq!(core.finish(&start.id).await, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn a_bad_reply_is_refused() {
        let core = core(true).await;
        let start = core.start().await;
        std::fs::write(core.folder(&start).join("claim"), &start.value).unwrap();
        std::fs::write(core.folder(&start).join("reply"), "short").unwrap();
        assert_eq!(core.finish(&start.id).await, StatusCode::FORBIDDEN);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_symlinked_claim_file_is_refused() {
        let core = core(true).await;
        let start = core.start().await;
        let elsewhere = core.dir.path().join("elsewhere");
        std::fs::write(&elsewhere, &start.value).unwrap();
        let folder = core.folder(&start);
        std::fs::remove_file(folder.join("claim")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, folder.join("claim")).unwrap();
        std::fs::write(folder.join("reply"), "ReplyFromTheLauncher123").unwrap();
        assert_eq!(core.finish(&start.id).await, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn an_expired_claim_is_refused() {
        let core = core(true).await;
        let start = core.start().await;
        core.answer(&start, &start.value);
        core.state
            .local_claims
            .state
            .lock()
            .await
            .pending
            .as_mut()
            .unwrap()
            .started -= CLAIM_TTL;
        assert_eq!(core.finish(&start.id).await, StatusCode::FORBIDDEN);
        assert!(!core.folder(&start).exists());
    }

    #[tokio::test]
    async fn a_claim_is_used_once() {
        let core = core(true).await;
        let start = core.start().await;
        core.answer(&start, &start.value);
        assert_eq!(core.finish(&start.id).await, StatusCode::OK);
        core.answer(&start, &start.value);
        assert_eq!(core.finish(&start.id).await, StatusCode::FORBIDDEN);
        let users = core.state.users_manager.read().await;
        let owner = users.get_user_by_username("boss").unwrap();
        assert_eq!(users.list_api_keys(&owner.uid).len(), 1);
    }

    #[tokio::test]
    async fn a_new_start_replaces_the_waiting_claim() {
        let core = core(true).await;
        let first = core.start().await;
        let second = core.start().await;
        assert_ne!(first.id, second.id);
        assert!(!core.folder(&first).exists());
        // a wrong id leaves the waiting claim alone
        assert_eq!(core.finish(&first.id).await, StatusCode::FORBIDDEN);
        core.answer(&second, &second.value);
        assert_eq!(core.finish(&second.id).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn starts_are_limited() {
        let core = core(true).await;
        for _ in 0..MAX_STARTS_PER_MINUTE {
            core.start().await;
        }
        let (status, _) = core.post("/local_claim/start", json!({})).await;
        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
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
