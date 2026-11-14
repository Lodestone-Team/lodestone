use std::{collections::HashMap, path::PathBuf, sync::LazyLock};

use argon2::{Argon2, PasswordVerifier};
use color_eyre::eyre::{eyre, Context};
use jsonwebtoken::{Algorithm, Validation};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};
use ts_rs::TS;

use crate::{
    error::{Error, ErrorKind},
    event_broadcaster::EventBroadcaster,
    events::{
        CausedBy, Event, EventInner, InstanceEventInner, ProgressionEndValue, ProgressionEvent,
        ProgressionEventInner, ProgressionStartValue, UserEvent, UserEventInner,
    },
    types::{InstanceUuid, Snowflake},
};

use super::{
    api_key::{self, ApiKeyInfo, ApiKeyRecord, ApiKeyScope, ApiKeyStore, CreatedApiKey},
    hashed_password::{hash_password, HashedPassword},
    jwt_token::JwtToken,
    permission::UserPermission,
    user_id::UserId,
    user_secrets::UserSecret,
};

/// The most characters a username has.
pub const MAX_USERNAME_CHARS: usize = 64;

/// Checks a new username, for every user (the owner included) and every way one is made
/// or changed, and returns it trimmed: 1 to [`MAX_USERNAME_CHARS`] characters, no control
/// characters and no `:`. Signing in sends `username:password` (HTTP Basic), split at the
/// first `:`, so a username with one could never sign in. The error is for people.
pub fn valid_username(username: &str) -> Result<String, &'static str> {
    let username = username.trim();
    let chars = username.chars().count();
    if chars == 0 || chars > MAX_USERNAME_CHARS {
        return Err("A username must have 1 to 64 characters");
    }
    if username.chars().any(char::is_control) {
        return Err("A username can't have control characters");
    }
    if username.contains(':') {
        return Err("A username can't have a colon (:)");
    }
    Ok(username.to_string())
}

/// A 400 with `message`, which is for people.
fn bad_request(message: &str) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message.to_string()),
    }
}

#[derive(Deserialize, Serialize)]
pub struct Claim {
    pub uid: UserId,
    pub exp: usize,
}
/// How the request a [`User`] value was made for authenticated.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AuthSource {
    /// A session token from `POST /user/login`, or a user read from the store.
    #[default]
    Session,
    /// An API key. The key's permissions are already applied to the `User`.
    ApiKey {
        id: String,
        name: String,
        expires_at: Option<i64>,
        /// Whether the key is a full one ([`api_key::ApiKeyScope::Full`]): it may do
        /// everything its user can, including changing the account's own details.
        full: bool,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct User {
    pub uid: UserId,
    pub username: String,
    pub hashed_psw: HashedPassword,
    pub is_owner: bool,
    pub is_admin: bool,
    pub permissions: UserPermission,
    pub secret: UserSecret,
    /// Set per request by [`UsersManager::try_auth`]; never saved.
    #[serde(skip)]
    pub auth_source: AuthSource,
    /// Set per request for a launcher API key of the owner or an admin: it may view, start
    /// and stop every instance (and nothing else, since `is_owner` and `is_admin` are off).
    /// See [`api_key::launcher_user`]. Never saved.
    #[serde(skip)]
    pub key_all_instances: bool,
    /// Set per request for a launcher API key: which halves of console access the key
    /// opted in to. `None` (sessions and other keys) means the console permission applies
    /// in full, reading and writing. Never saved.
    #[serde(skip)]
    pub key_console: Option<KeyConsole>,
}

/// The console access a launcher API key opted in to, on top of (never beyond) its
/// user's `can_access_instance_console`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyConsole {
    /// Reading the console's output.
    pub read: bool,
    /// Sending commands and chat to the console.
    pub write: bool,
}

impl User {
    pub fn new(
        username: String,
        password: impl AsRef<str>,
        is_owner: bool,
        is_admin: bool,
        permissions: UserPermission,
    ) -> Self {
        User {
            uid: UserId::default(),
            username,
            hashed_psw: hash_password(password),
            is_owner,
            is_admin,
            permissions,
            secret: UserSecret::default(),
            auth_source: AuthSource::Session,
            key_all_instances: false,
            key_console: None,
        }
    }

    /// The owner Lodestone Host makes on its first run: no password, so nobody can sign
    /// in with one. The Host signs in through the local claim until the owner sets a
    /// password to share the core (`PUT /user/owner_account`).
    pub fn owner_without_password(username: String) -> Self {
        User {
            uid: UserId::default(),
            username,
            hashed_psw: HashedPassword::none(),
            is_owner: true,
            is_admin: false,
            permissions: UserPermission::default(),
            secret: UserSecret::default(),
            auth_source: AuthSource::Session,
            key_all_instances: false,
            key_console: None,
        }
    }

    /// Whether the user has a password to sign in with.
    pub fn has_password(&self) -> bool {
        self.hashed_psw.is_set()
    }

    /// Who to name in events and logs for what this request does: the username, plus the
    /// key's name for a request made with an API key.
    pub fn caused_by(&self) -> CausedBy {
        let user_name = match &self.auth_source {
            AuthSource::Session => self.username.clone(),
            AuthSource::ApiKey { name, .. } => format!("{} (API key: {name})", self.username),
        };
        CausedBy::User {
            user_id: self.uid.clone(),
            user_name,
        }
    }

    pub fn is_api_key(&self) -> bool {
        matches!(self.auth_source, AuthSource::ApiKey { .. })
    }

    /// Fails with 403 for a request made with an API key, whatever its scope.
    pub fn require_session(&self) -> Result<(), Error> {
        if self.is_api_key() {
            return Err(Error {
                kind: ErrorKind::PermissionDenied,
                source: eyre!(api_key::SESSION_REQUIRED_MESSAGE),
            });
        }
        Ok(())
    }

    /// Fails with 403 for a request made with a custom or launcher API key. Sessions and
    /// full keys pass: for account details a full key may change (the username) but a
    /// scoped key, which only has the permissions it lists, must not.
    pub fn require_session_or_full_key(&self) -> Result<(), Error> {
        if !self.acts_as_whole_account() {
            return Err(Error {
                kind: ErrorKind::PermissionDenied,
                source: eyre!(api_key::FULL_KEY_REQUIRED_MESSAGE),
            });
        }
        Ok(())
    }

    /// Whether the request may act as the whole account: a session or a full key. A
    /// custom or launcher key has only the permissions it lists, so what its user did
    /// elsewhere (their own events, say) is none of its business.
    fn acts_as_whole_account(&self) -> bool {
        !matches!(self.auth_source, AuthSource::ApiKey { full: false, .. })
    }

    /// Whether this request's API key has expired since it authenticated. Always `false`
    /// for sessions.
    pub fn api_key_expired(&self, now: i64) -> bool {
        match &self.auth_source {
            AuthSource::Session => false,
            AuthSource::ApiKey { expires_at, .. } => api_key::is_expired(*expires_at, now),
        }
    }

    fn get_permission_level(&self) -> u8 {
        if self.is_owner {
            u8::MAX
        } else if self.is_admin {
            2
        } else {
            1
        }
    }
    /// Fails with 403 unless this user is the owner. `action` completes the message
    /// "Only the owner can ...".
    pub fn owner_only(&self, action: &str) -> Result<(), Error> {
        if self.is_owner {
            Ok(())
        } else {
            Err(Error {
                kind: ErrorKind::PermissionDenied,
                source: eyre!("Only the owner can {action}"),
            })
        }
    }

    pub fn update_permission(
        &self,
        other: &mut User,
        mut permissions: UserPermission,
    ) -> Result<(), Error> {
        permissions.clear_owner_only_legacy_fields();
        if self.uid == other.uid {
            return Err(Error {
                kind: ErrorKind::PermissionDenied,
                source: eyre!("You cannot change your own permissions"),
            });
        }
        if self.get_permission_level() <= other.get_permission_level() {
            return Err(Error {
                kind: ErrorKind::PermissionDenied,
                source: eyre!("You don't have permission to manage other users' permission"),
            });
        }
        if self.is_owner {
            other.permissions = permissions;
            return Ok(());
        }
        if !(self.is_admin || self.permissions.can_manage_permission) {
            return Err(Error {
                kind: ErrorKind::PermissionDenied,
                source: eyre!("You don't have permission to manage other users' permission"),
            });
        }
        // Only the owner can grant unsafe and owner exclusive permissions.
        // Keeping or removing the ones the owner already granted is allowed.
        if grants_owner_exclusive_permission(&other.permissions, &permissions) {
            return Err(Error {
                kind: ErrorKind::PermissionDenied,
                source: eyre!(
                    "Unsafe and owner exclusive permissions can only be granted by the owner"
                ),
            });
        }
        other.permissions = permissions;
        Ok(())
    }

    pub fn can_perform_action(&self, action: &UserAction) -> bool {
        if self.is_owner {
            return true;
        }
        match action {
            UserAction::ViewInstance(instance_id) => {
                self.is_admin
                    || self.key_all_instances
                    || self.permissions.can_view_instance.contains(instance_id)
            }
            UserAction::StartInstance(instance_id) => {
                self.is_admin
                    || self.key_all_instances
                    || self.permissions.can_start_instance.contains(instance_id)
            }
            UserAction::StopInstance(instance_id) => {
                self.is_admin
                    || self.key_all_instances
                    || self.permissions.can_stop_instance.contains(instance_id)
            }
            UserAction::AccessConsole(instance_id) => {
                self.key_console.is_none_or(|console| console.write)
                    && self.has_console_permission(instance_id)
            }
            // Reading the console (its output, chat, and what was typed into it) needs the
            // console permission, as the dashboard describes it ("read and send commands");
            // viewing an instance is not enough. A launcher key also needs its read opt-in.
            UserAction::ReadConsole(instance_id) => {
                self.key_console.is_none_or(|console| console.read)
                    && self.has_console_permission(instance_id)
            }
            UserAction::AccessSetting(instance_id) => {
                self.is_admin
                    || self
                        .permissions
                        .can_access_instance_setting
                        .contains(instance_id)
            }
            UserAction::ReadResource(instance_id) => {
                self.is_admin
                    || self
                        .permissions
                        .can_read_instance_resource
                        .contains(instance_id)
            }
            UserAction::WriteResource(instance_id) => self
                .permissions
                .can_write_instance_resource
                .contains(instance_id),
            UserAction::ReadInstanceFile(instance_id) => {
                self.is_admin
                    || self.permissions.can_read_global_file
                    || self
                        .permissions
                        .can_read_instance_file
                        .contains(instance_id)
            }
            UserAction::WriteInstanceFile(instance_id) => {
                self.permissions.can_write_global_file
                    || self
                        .permissions
                        .can_write_instance_file
                        .contains(instance_id)
            }
            UserAction::CreateInstance => self.is_admin || self.permissions.can_create_instance,
            UserAction::DeleteInstance => self.is_admin || self.permissions.can_delete_instance,
            UserAction::ReadGlobalFile => self.permissions.can_read_global_file,
            UserAction::WriteGlobalFile => self.permissions.can_write_global_file,
            UserAction::ManageUser => self.is_owner,
            UserAction::ManagePermission => self.permissions.can_manage_permission,
        }
    }

    /// Whether the user's console permission covers `instance_id`, before a launcher
    /// key's opt-ins: admins have it everywhere, and a launcher key of the owner or an
    /// admin (`key_all_instances`) too.
    fn has_console_permission(&self, instance_id: &InstanceUuid) -> bool {
        self.is_admin
            || self.key_all_instances
            || self
                .permissions
                .can_access_instance_console
                .contains(instance_id)
    }

    /// Checks that the user may perform `action`.
    ///
    /// Safe mode only restricts users other than the owner: while it is on, nobody but the
    /// owner can use an unsafe action, even if the permission was granted earlier.
    pub fn try_action(&self, action: &UserAction, safe_mode: bool) -> Result<(), Error> {
        if self.is_owner {
            return Ok(());
        }
        if safe_mode && !action.is_safe() {
            return Err(Error {
                kind: ErrorKind::PermissionDenied,
                source: eyre!(
                    "This action is not allowed while safe mode is on. Only the owner can perform it"
                ),
            });
        }
        if self.can_perform_action(action) {
            Ok(())
        } else {
            Err(Error {
                kind: ErrorKind::PermissionDenied,
                source: match action {
                    UserAction::ViewInstance(_) => {
                        eyre!("You don't have permission to view this server")
                    }
                    UserAction::StartInstance(_) => {
                        eyre!("You don't have permission to start this server")
                    }
                    UserAction::StopInstance(_) => {
                        eyre!("You don't have permission to stop this server")
                    }
                    UserAction::AccessConsole(_) => {
                        eyre!("You don't have permission to access this server's console")
                    }
                    UserAction::ReadConsole(_) => {
                        eyre!("You don't have permission to read this server's console")
                    }
                    UserAction::AccessSetting(_) => {
                        eyre!("You don't have permission to access this server's setting")
                    }
                    UserAction::ReadResource(_) => {
                        eyre!("You don't have permission to read this server's resource")
                    }
                    UserAction::WriteResource(_) => {
                        eyre!("You don't have permission to write this server's resource")
                    }
                    UserAction::ReadInstanceFile(_) => {
                        eyre!("You don't have permission to read this server's file")
                    }
                    UserAction::WriteInstanceFile(_) => {
                        eyre!("You don't have permission to write this server's file")
                    }
                    UserAction::CreateInstance => {
                        eyre!("You don't have permission to create servers")
                    }
                    UserAction::DeleteInstance => {
                        eyre!("You don't have permission to delete servers")
                    }
                    UserAction::ReadGlobalFile => {
                        eyre!("You don't have permission to read global file")
                    }
                    UserAction::WriteGlobalFile => {
                        eyre!("You don't have permission to write global file")
                    }
                    UserAction::ManageUser => eyre!("You don't have permission to manage user"),
                    UserAction::ManagePermission => {
                        eyre!("You don't have permission to manage permission")
                    }
                },
            })
        }
    }

    pub fn can_view_event(&self, event: impl AsRef<Event>) -> bool {
        let event = event.as_ref();
        self.can_view_event_parts(&event.event_inner, &event.caused_by)
    }

    /// Same rules as [`User::can_view_event`], for events that are not an [`Event`], such as
    /// the `ClientEvent`s read back from the event database.
    pub fn can_view_event_parts(&self, event_inner: &EventInner, caused_by: &CausedBy) -> bool {
        match event_inner {
            // what the console shows, and what was typed into it, see ReadConsole
            EventInner::InstanceEvent(event) if is_console_content(&event.instance_event_inner) => {
                self.can_perform_action(&UserAction::ReadConsole(event.instance_uuid.clone()))
            }
            EventInner::InstanceEvent(event) => {
                self.can_perform_action(&UserAction::ViewInstance(event.instance_uuid.clone()))
            }
            // the user's own account changing (permissions, username, keys), so their
            // dashboard and Host can follow it; everything else about users is for managers
            EventInner::UserEvent(event) => {
                (event.user_id == self.uid && self.acts_as_whole_account())
                    || self.can_perform_action(&UserAction::ManageUser)
            }
            EventInner::FSEvent(_) => self.can_perform_action(&UserAction::ManageUser),
            EventInner::ProgressionEvent(progression_event) => {
                self.can_view_progression_event(progression_event, caused_by)
            }
            EventInner::GlobalSettingsChanged => true,
        }
    }

    /// Progression events can name files and carry full instance info, so only show them to
    /// the user who started the operation, to users who can view the instance it is about,
    /// and to owners and admins. A backup's or restore's progress is about the server's
    /// files, so it needs `ReadInstanceFile` rather than `ViewInstance`. A custom or
    /// launcher key isn't the user who started it: it sees only what its own permissions
    /// allow, whoever started the operation.
    fn can_view_progression_event(
        &self,
        progression_event: &ProgressionEvent,
        caused_by: &CausedBy,
    ) -> bool {
        if self.is_owner || self.is_admin {
            return true;
        }
        if let CausedBy::User { user_id, .. } = caused_by {
            if *user_id == self.uid && self.acts_as_whole_account() {
                return true;
            }
        }
        let view = |uuid: &InstanceUuid| UserAction::ViewInstance(uuid.clone());
        let read_files = |uuid: &InstanceUuid| UserAction::ReadInstanceFile(uuid.clone());
        let action = match progression_event.progression_event_inner() {
            // every start value with an instance must be listed here: nothing else checks it
            ProgressionEventInner::ProgressionStart {
                inner: Some(start_value),
                ..
            } => match start_value {
                ProgressionStartValue::InstanceCreation { instance_uuid }
                | ProgressionStartValue::InstanceDelete { instance_uuid } => {
                    Some(view(instance_uuid))
                }
                ProgressionStartValue::Backup { instance_uuid }
                | ProgressionStartValue::Restore { instance_uuid } => {
                    Some(read_files(instance_uuid))
                }
            },
            ProgressionEventInner::ProgressionEnd {
                inner: Some(end_value),
                ..
            } => match end_value {
                ProgressionEndValue::InstanceCreation(info) => Some(view(&info.uuid)),
                ProgressionEndValue::InstanceDelete { instance_uuid }
                | ProgressionEndValue::FSOperationCompleted { instance_uuid, .. } => {
                    Some(view(instance_uuid))
                }
                ProgressionEndValue::BackupCompleted { instance_uuid, .. }
                | ProgressionEndValue::RestoreCompleted { instance_uuid, .. } => {
                    Some(read_files(instance_uuid))
                }
            },
            _ => None,
        };
        action.is_some_and(|action| self.can_perform_action(&action))
    }

    pub fn create_jwt(&self) -> Result<JwtToken, Error> {
        let exp = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::days(60))
            .ok_or_else(|| eyre!("Failed to create JWT token"))?
            .timestamp();
        let claim = Claim {
            uid: self.uid.clone(),
            exp: exp as usize,
        };

        JwtToken::new(claim, self.secret.clone())
    }
}

/// Whether an instance event carries console content: output, chat, system messages and
/// the commands sent.
fn is_console_content(inner: &InstanceEventInner) -> bool {
    matches!(
        inner,
        InstanceEventInner::InstanceOutput { .. }
            | InstanceEventInner::PlayerMessage { .. }
            | InstanceEventInner::SystemMessage { .. }
            | InstanceEventInner::InstanceInput { .. }
    )
}

pub enum UserAction {
    // instance specific actions:
    ViewInstance(InstanceUuid),
    StartInstance(InstanceUuid),
    StopInstance(InstanceUuid),
    /// Sending commands to the console. A launcher key also needs its write opt-in.
    AccessConsole(InstanceUuid),
    /// Reading the console (output, chat and the commands sent). For sessions and full or
    /// custom keys this is [`UserAction::ViewInstance`]; a launcher key needs its read
    /// opt-in and `can_access_instance_console`.
    ReadConsole(InstanceUuid),
    AccessSetting(InstanceUuid),
    ReadResource(InstanceUuid),
    WriteResource(InstanceUuid),
    ReadInstanceFile(InstanceUuid),
    WriteInstanceFile(InstanceUuid),

    // global actions:
    CreateInstance,
    DeleteInstance,
    ReadGlobalFile,
    WriteGlobalFile,
    ManageUser,
    ManagePermission,
}

impl UserAction {
    /// Whether a non-owner may perform this action while safe mode is on.
    ///
    /// Must match the "unsafe permission" comments in `auth/permission.rs`.
    pub fn is_safe(&self) -> bool {
        match self {
            UserAction::ViewInstance(_) => true,
            UserAction::StartInstance(_) => true,
            UserAction::StopInstance(_) => true,
            UserAction::AccessConsole(_) => true,
            UserAction::ReadConsole(_) => true,
            UserAction::AccessSetting(_) => true,
            UserAction::ReadResource(_) => true,
            UserAction::WriteResource(_) => false,
            UserAction::ReadInstanceFile(_) => true,
            UserAction::WriteInstanceFile(_) => false,
            UserAction::CreateInstance => true,
            UserAction::DeleteInstance => true,
            UserAction::ReadGlobalFile => false,
            UserAction::WriteGlobalFile => false,
            UserAction::ManageUser => false,
            UserAction::ManagePermission => false,
        }
    }
}

/// Whether going from `old` to `new` adds any permission that only the owner may grant.
///
/// These are the unsafe permissions marked in `auth/permission.rs`, plus reading global
/// files (which can read every user's secret in `users.json`). The macro and extension
/// fields grant nothing and are cleared before this check.
fn grants_owner_exclusive_permission(old: &UserPermission, new: &UserPermission) -> bool {
    let adds_flag = |old: bool, new: bool| new && !old;
    !new.can_write_instance_resource
        .is_subset(&old.can_write_instance_resource)
        || !new
            .can_write_instance_file
            .is_subset(&old.can_write_instance_file)
        || adds_flag(old.can_write_global_file, new.can_write_global_file)
        || adds_flag(old.can_read_global_file, new.can_read_global_file)
        || adds_flag(old.can_manage_permission, new.can_manage_permission)
}

#[derive(Serialize, Deserialize, Clone, TS)]
#[ts(export)]
pub struct PublicUser {
    pub uid: UserId,
    pub username: String,
    pub is_owner: bool,
    pub is_admin: bool,
    pub permissions: UserPermission,
    /// Whether the user can sign in with a password. Only Lodestone Host's owner has
    /// none, until they set one to share the core. Older cores don't send it: all their
    /// users have one.
    #[serde(default = "crate::global_settings::default_true")]
    pub has_password: bool,
}

impl From<&User> for PublicUser {
    fn from(user: &User) -> Self {
        PublicUser {
            uid: user.uid.clone(),
            username: user.username.clone(),
            is_owner: user.is_owner,
            is_admin: user.is_admin,
            permissions: user.permissions.clone(),
            has_password: user.has_password(),
        }
    }
}

impl From<User> for PublicUser {
    fn from(user: User) -> Self {
        let has_password = user.has_password();
        PublicUser {
            uid: user.uid,
            username: user.username,
            is_owner: user.is_owner,
            is_admin: user.is_admin,
            permissions: user.permissions,
            has_password,
        }
    }
}

pub struct UsersManager {
    event_broadcaster: EventBroadcaster,
    users: HashMap<UserId, User>,
    path_to_users: PathBuf,
    /// Kept next to the users file, as `api_keys.json`.
    api_keys: ApiKeyStore,
}

impl UsersManager {
    pub fn new(
        event_broadcaster: EventBroadcaster,
        users: HashMap<UserId, User>,
        path_to_users: PathBuf,
    ) -> Self {
        let api_keys = ApiKeyStore::new(path_to_users.with_file_name("api_keys.json"));
        Self {
            event_broadcaster,
            users,
            path_to_users,
            api_keys,
        }
    }

    /// Loads the users, then their API keys.
    pub async fn load_users(&mut self) -> Result<(), Error> {
        self.load_users_file().await?;
        self.api_keys.load().await;
        let users = &self.users;
        self.api_keys.retain_users(|uid| users.contains_key(uid));
        Ok(())
    }

    async fn load_users_file(&mut self) -> Result<(), Error> {
        let mut options = tokio::fs::OpenOptions::new();
        options.read(true).create(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        if options
            .open(&self.path_to_users)
            .await
            .context(format!(
                "Failed to open user file : {}",
                &self.path_to_users.display()
            ))?
            .metadata()
            .await
            .context(format!(
                "Failed to access metadata : {}",
                &self.path_to_users.display()
            ))?
            .len()
            == 0
        {
            warn!("No user file found, creating a new one");
            self.users = HashMap::new();
        } else {
            let users: HashMap<UserId, User> = serde_json::from_reader(
                tokio::fs::File::open(&self.path_to_users)
                    .await
                    .context(format!(
                        "Failed to open user file : {}",
                        &self.path_to_users.display()
                    ))?
                    .into_std()
                    .await,
            )
            .context("Failed to deserialize user json")?;
            self.users = users;
        }
        Ok(())
    }

    /// Saves the users to disk.
    ///
    /// The file holds every user's secret, so it is written atomically, readable only by the
    /// current OS user. A crash while writing leaves the old file in place instead of a
    /// truncated one.
    async fn write_to_file(&self) -> Result<(), Error> {
        let data = serde_json::to_vec(&self.users).context("Failed to serialize user json")?;
        crate::persist::write_atomic(&self.path_to_users, data, Some(0o600))
            .await
            .context(format!(
                "Failed to write user file {}",
                self.path_to_users.display()
            ))?;
        Ok(())
    }
    pub fn get_user(&self, uid: impl AsRef<UserId>) -> Option<User> {
        self.users.get(uid.as_ref()).cloned()
    }
    /// Adds `user`, whose username must be valid ([`valid_username`]: callers trim it
    /// first) and not taken.
    pub async fn add_user(&mut self, user: User, caused_by: CausedBy) -> Result<(), Error> {
        if valid_username(&user.username).map_err(bad_request)? != user.username {
            return Err(bad_request("A username can't start or end with a space"));
        }
        if self.get_user_by_username(&user.username).is_some() {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Username already exist"),
            });
        }
        let uid = user.uid.clone();
        self.users.insert(uid.clone(), user);
        match self.write_to_file().await {
            Ok(()) => {
                self.event_broadcaster.send(Event {
                    event_inner: EventInner::UserEvent(UserEvent {
                        user_id: uid,
                        user_event_inner: UserEventInner::UserCreated,
                    }),
                    details: "".to_string(),
                    snowflake: Snowflake::default(),
                    caused_by,
                });
                Ok(())
            }
            Err(e) => {
                self.users.remove(&uid);
                Err(e)
            }
        }
    }
    pub async fn delete_user(
        &mut self,
        uid: impl AsRef<UserId>,
        caused_by: CausedBy,
    ) -> Result<Option<User>, Error> {
        let user = self.users.remove(uid.as_ref());
        match self.write_to_file().await {
            Ok(()) => {
                // the keys stop working at once, as they are removed from memory even if
                // saving fails; keys of a missing user never authenticate anyway
                if let Err(e) = self.api_keys.remove_user(uid.as_ref()).await {
                    error!("Failed to save the API keys after deleting a user: {e}");
                }
                if let Some(_user) = user.as_ref() {
                    self.event_broadcaster.send(Event {
                        event_inner: EventInner::UserEvent(UserEvent {
                            user_id: uid.as_ref().to_owned(),
                            user_event_inner: UserEventInner::UserDeleted,
                        }),
                        details: "".to_string(),
                        snowflake: Snowflake::default(),
                        caused_by,
                    });
                }
            }
            Err(e) => {
                if let Some(user) = user {
                    self.users.insert(uid.as_ref().to_owned(), user);
                }
                return Err(e);
            }
        }

        Ok(user)
    }

    pub async fn logout_user(
        &mut self,
        uid: impl AsRef<UserId>,
        caused_by: CausedBy,
    ) -> Result<(), Error> {
        let old_secret = self
            .users
            .get_mut(uid.as_ref())
            .ok_or_else(|| Error {
                kind: ErrorKind::NotFound,
                source: eyre!("User id not found"),
            })?
            .secret
            .clone();
        if let Some(user) = self.users.get_mut(uid.as_ref()) {
            user.secret = UserSecret::default();
        }

        match self.write_to_file().await {
            Ok(_) => {
                self.event_broadcaster.send(Event {
                    event_inner: EventInner::UserEvent(UserEvent {
                        user_id: uid.as_ref().to_owned(),
                        user_event_inner: UserEventInner::UserLoggedOut,
                    }),
                    details: "".to_string(),
                    snowflake: Snowflake::default(),
                    caused_by,
                });
                Ok(())
            }
            Err(e) => {
                if let Some(user) = self.users.get_mut(uid.as_ref()) {
                    user.secret = old_secret
                }
                Err(e)
            }
        }
    }

    pub async fn rename_user(
        &mut self,
        uid: impl AsRef<UserId>,
        new_username: String,
        caused_by: CausedBy,
    ) -> Result<(), Error> {
        let new_username = valid_username(&new_username).map_err(bad_request)?;
        let old_username = self
            .users
            .get_mut(uid.as_ref())
            .ok_or_else(|| Error {
                kind: ErrorKind::NotFound,
                source: eyre!("User id not found"),
            })?
            .username
            .clone();
        if self
            .users
            .values()
            .any(|user| user.username == new_username && user.uid != *uid.as_ref())
        {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Username already exist"),
            });
        }
        if let Some(user) = self.users.get_mut(uid.as_ref()) {
            user.username = new_username.clone();
            match self.write_to_file().await {
                Ok(_) => {
                    self.event_broadcaster.send(Event {
                        event_inner: EventInner::UserEvent(UserEvent {
                            user_id: uid.as_ref().to_owned(),
                            user_event_inner: UserEventInner::UsernameChanged {
                                new_username: new_username.clone(),
                            },
                        }),
                        details: "".to_string(),
                        snowflake: Snowflake::default(),
                        caused_by,
                    });
                    Ok(())
                }
                Err(e) => {
                    if let Some(user) = self.users.get_mut(uid.as_ref()) {
                        user.username = old_username
                    }
                    Err(e)
                }
            }
        } else {
            Err(Error {
                kind: ErrorKind::NotFound,
                source: eyre!("User id not found"),
            })
        }
    }

    pub async fn change_password(
        &mut self,
        uid: impl AsRef<UserId>,
        old_password: Option<impl AsRef<str>>,
        password: String,
        caused_by: CausedBy,
    ) -> Result<(), Error> {
        let user = self.users.get(uid.as_ref()).ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("User id not found"),
        })?;
        // the owner's password guards the whole core, from other devices too: the same
        // rule as when the owner account is set up
        if user.is_owner {
            crate::remote_access::owner_password(&password).map_err(bad_request)?;
        }
        let old_hash = user.hashed_psw.clone();
        let old_secret = user.secret.clone();
        if let Some(old_password) = old_password {
            // The requester is logged in, so a wrong old password is a refused action (403),
            // not a missing or expired login (401).
            verify_password(&old_hash, old_password.as_ref()).map_err(|e| match e.kind {
                ErrorKind::Unauthorized => Error {
                    kind: ErrorKind::PermissionDenied,
                    source: e.source,
                },
                _ => e,
            })?;
        }
        // changing the password also logs the user out everywhere
        if let Some(user) = self.users.get_mut(uid.as_ref()) {
            user.hashed_psw = hash_password(password);
            user.secret = UserSecret::default();
        }
        match self.write_to_file().await {
            Ok(_) => {
                self.event_broadcaster.send(Event {
                    event_inner: EventInner::UserEvent(UserEvent {
                        user_id: uid.as_ref().to_owned(),
                        user_event_inner: UserEventInner::UserLoggedOut,
                    }),
                    details: "".to_string(),
                    snowflake: Snowflake::default(),
                    caused_by,
                });
                Ok(())
            }
            Err(e) => {
                if let Some(user) = self.users.get_mut(uid.as_ref()) {
                    user.hashed_psw = old_hash;
                    user.secret = old_secret;
                }
                Err(e)
            }
        }
    }

    /// Gives the owner `uid`, who has no password yet (Lodestone Host's first run), a
    /// username and a password, `hashed` already. Fails with 409 when they have a
    /// password: that is changed with the old one. Their sessions stay signed in.
    pub async fn set_up_owner_account(
        &mut self,
        uid: &UserId,
        username: String,
        hashed: HashedPassword,
        caused_by: CausedBy,
    ) -> Result<User, Error> {
        let username = valid_username(&username).map_err(bad_request)?;
        let user = self.users.get(uid).ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("User id not found"),
        })?;
        if !user.is_owner {
            return Err(Error {
                kind: ErrorKind::PermissionDenied,
                source: eyre!("Only the owner sets up the owner account"),
            });
        }
        if user.has_password() {
            return Err(Error {
                kind: ErrorKind::Conflict,
                source: eyre!("The owner already has a password. Change it in your profile"),
            });
        }
        if self
            .users
            .values()
            .any(|other| other.username == username && other.uid != *uid)
        {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Username already exist"),
            });
        }
        let old = user.clone();
        let renamed = old.username != username;
        if let Some(user) = self.users.get_mut(uid) {
            user.username = username.clone();
            user.hashed_psw = hashed;
        }
        if let Err(e) = self.write_to_file().await {
            self.users.insert(uid.clone(), old);
            return Err(e);
        }
        if renamed {
            self.event_broadcaster.send(Event {
                event_inner: EventInner::UserEvent(UserEvent {
                    user_id: uid.clone(),
                    user_event_inner: UserEventInner::UsernameChanged {
                        new_username: username,
                    },
                }),
                details: "".to_string(),
                snowflake: Snowflake::default(),
                caused_by,
            });
        }
        self.get_user(uid).ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("User id not found"),
        })
    }

    pub fn get_user_by_username(&self, username: impl AsRef<str>) -> Option<User> {
        self.users
            .values()
            .find(|user| user.username == username.as_ref())
            .cloned()
    }

    /// Replaces a user's permissions and saves them.
    ///
    /// This does not check whether the change is allowed; callers acting for a user must
    /// check with [`User::update_permission`] first.
    pub async fn update_permissions(
        &mut self,
        uid: impl AsRef<UserId>,
        new_permissions: UserPermission,
        caused_by: CausedBy,
    ) -> Result<(), Error> {
        self.modify_permissions(uid, |permissions| *permissions = new_permissions, caused_by)
            .await
    }

    /// Changes a user's current permissions with `modify` and saves them.
    ///
    /// Unlike reading the permissions, changing them and calling
    /// [`UsersManager::update_permissions`], this cannot overwrite a change made in between,
    /// because it runs under the same `&mut self` borrow (the users write lock).
    pub async fn modify_permissions(
        &mut self,
        uid: impl AsRef<UserId>,
        modify: impl FnOnce(&mut UserPermission),
        caused_by: CausedBy,
    ) -> Result<(), Error> {
        let user = self.users.get_mut(uid.as_ref()).ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("User id not found"),
        })?;
        let old_permission = user.permissions.clone();
        modify(&mut user.permissions);
        let new_permissions = user.permissions.clone();
        match self.write_to_file().await {
            Ok(_) => {
                self.event_broadcaster.send(Event {
                    event_inner: EventInner::UserEvent(UserEvent {
                        user_id: uid.as_ref().to_owned(),
                        user_event_inner: UserEventInner::PermissionChanged {
                            new_permissions: Box::new(new_permissions),
                        },
                    }),
                    details: "".to_string(),
                    snowflake: Snowflake::default(),
                    caused_by,
                });
                Ok(())
            }
            Err(e) => {
                if let Some(user) = self.users.get_mut(uid.as_ref()) {
                    user.permissions = old_permission;
                }
                Err(e)
            }
        }
    }

    /// The user a request with `token` (a session JWT or an API key) acts as, or `None`.
    ///
    /// Every route authenticates through this, so it is the one place that turns an API key
    /// into its [`User`], see [`api_key::effective_user`].
    pub fn try_auth(&self, token: &str) -> Option<User> {
        if token.starts_with(api_key::API_KEY_PREFIX) {
            return self.try_auth_api_key(token);
        }
        let claimed_uid = decode_no_verify(token)?;
        let claimed_requester = self.users.get(&claimed_uid)?;
        let requester_uid = decode_token(token, &claimed_requester.secret)?;
        if claimed_uid != requester_uid {
            return None;
        }
        let mut user = claimed_requester.to_owned();
        user.auth_source = AuthSource::Session;
        Some(user)
    }

    fn try_auth_api_key(&self, token: &str) -> Option<User> {
        let Some(parsed) = api_key::parse_key(token) else {
            warn!("API key authentication failed: the key is malformed");
            return None;
        };
        let fail = |reason: &str| {
            warn!(
                "API key authentication failed for key {}: {reason}",
                parsed.id
            );
            None
        };
        let Some(record) = self.api_keys.get(parsed.id) else {
            return fail("no such key");
        };
        if !record.verify_secret(parsed.secret) {
            return fail("wrong secret");
        }
        let now = api_key::now();
        if record.is_expired(now) {
            return fail("the key has expired");
        }
        let Some(user) = self.users.get(&record.uid) else {
            return fail("its user no longer exists");
        };
        self.api_keys.record_use(&record.id, now);
        Some(api_key::effective_user(record, user))
    }

    /// `user` (from [`UsersManager::try_auth`]) as it is now: with its user's current
    /// permissions and, for an API key, the key's current scope. `None` if the user was
    /// deleted, or the key was revoked or has expired.
    ///
    /// Use this, not [`UsersManager::get_user`], to re-read the user a request or stream
    /// acts as: the stored user has all of the user's powers, which a custom key does not.
    pub fn refresh_user(&self, user: &User) -> Option<User> {
        let stored = self.users.get(&user.uid)?;
        match &user.auth_source {
            AuthSource::Session => {
                let mut refreshed = stored.clone();
                refreshed.auth_source = AuthSource::Session;
                Some(refreshed)
            }
            AuthSource::ApiKey { id, .. } => {
                let record = self.api_keys.get(id)?;
                if record.uid != user.uid || record.is_expired(api_key::now()) {
                    return None;
                }
                Some(api_key::effective_user(record, stored))
            }
        }
    }

    /// The API keys of `uid`, newest first.
    pub fn list_api_keys(&self, uid: &UserId) -> Vec<ApiKeyInfo> {
        self.api_keys.list_for_user(uid)
    }

    /// Creates an API key for `uid`. `name` and `expires_at` must already be validated.
    /// The reply is the only place the key is ever returned.
    pub async fn create_api_key(
        &mut self,
        uid: &UserId,
        name: String,
        expires_at: Option<i64>,
        scope: ApiKeyScope,
        caused_by: CausedBy,
    ) -> Result<CreatedApiKey, Error> {
        self.mint_api_key(uid, name, expires_at, scope, caused_by, true)
            .await
    }

    /// Creates a key; `limited` applies [`api_key::MAX_API_KEYS_PER_USER`].
    async fn mint_api_key(
        &mut self,
        uid: &UserId,
        name: String,
        expires_at: Option<i64>,
        scope: ApiKeyScope,
        caused_by: CausedBy,
        limited: bool,
    ) -> Result<CreatedApiKey, Error> {
        let username = self
            .users
            .get(uid)
            .ok_or_else(|| Error {
                kind: ErrorKind::NotFound,
                source: eyre!("User not found"),
            })?
            .username
            .clone();
        if limited && self.api_keys.count_for_user(uid) >= api_key::MAX_API_KEYS_PER_USER {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!(
                    "You already have {} API keys, the most allowed. Revoke one first",
                    api_key::MAX_API_KEYS_PER_USER
                ),
            });
        }
        let new = loop {
            let new = api_key::generate_key()?;
            if self.api_keys.get(&new.id).is_none() {
                break new;
            }
        };
        let record = ApiKeyRecord {
            id: new.id.clone(),
            name,
            uid: uid.clone(),
            created_at: api_key::now(),
            expires_at,
            last_used_at: None,
            scope,
            secret: Some(new.secret.clone()),
            secret_sha256: None,
        };
        let info = record.info();
        self.api_keys.insert(record).await?;
        self.send_api_key_event(
            uid,
            &username,
            UserEventInner::ApiKeyCreated {
                key_id: info.id.clone(),
                name: info.name.clone(),
            },
            caused_by,
        );
        Ok(CreatedApiKey { key: new.key, info })
    }

    /// Creates a full-scope key for `uid` named `name`, for a local claim
    /// ([`crate::local_claim`]): first revokes `uid`'s other keys of that name (the
    /// claiming program uses one name per computer, so each claim replaces the last), and
    /// isn't limited by [`api_key::MAX_API_KEYS_PER_USER`], so a user with many keys can
    /// still claim. Returns the key and how many were replaced.
    pub async fn replace_claim_key(
        &mut self,
        uid: &UserId,
        name: String,
    ) -> Result<(CreatedApiKey, usize), Error> {
        let same_name: Vec<String> = self
            .api_keys
            .list_for_user(uid)
            .into_iter()
            .filter(|key| key.name == name)
            .map(|key| key.id)
            .collect();
        for id in &same_name {
            self.revoke_api_key(uid, id, CausedBy::System).await?;
        }
        let created = self
            .mint_api_key(uid, name, None, ApiKeyScope::Full, CausedBy::System, false)
            .await?;
        Ok((created, same_name.len()))
    }

    /// Revokes the key `id` of `uid`. It stops working at once. 404 if `uid` has no such
    /// key.
    pub async fn revoke_api_key(
        &mut self,
        uid: &UserId,
        id: &str,
        caused_by: CausedBy,
    ) -> Result<(), Error> {
        let record = self.api_keys.remove(uid, id).await?.ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("API key not found"),
        })?;
        let username = self
            .users
            .get(uid)
            .map(|u| u.username.clone())
            .unwrap_or_else(|| uid.to_string());
        self.send_api_key_event(
            uid,
            &username,
            UserEventInner::ApiKeyRevoked {
                key_id: record.id,
                name: record.name,
            },
            caused_by,
        );
        Ok(())
    }

    fn send_api_key_event(
        &self,
        uid: &UserId,
        username: &str,
        inner: UserEventInner,
        caused_by: CausedBy,
    ) {
        let details = match &inner {
            UserEventInner::ApiKeyCreated { name, .. } => {
                format!("API key `{name}` created for {username}")
            }
            UserEventInner::ApiKeyRevoked { name, .. } => {
                format!("API key `{name}` revoked for {username}")
            }
            _ => String::new(),
        };
        self.event_broadcaster.send(Event {
            event_inner: EventInner::UserEvent(UserEvent {
                user_id: uid.clone(),
                user_event_inner: inner,
            }),
            details,
            snowflake: Snowflake::default(),
            caused_by,
        });
    }

    /// Saves the API keys' last-used times, if any changed since the last save. Called
    /// once a minute and at shutdown.
    pub async fn flush_api_keys(&self) -> Result<(), Error> {
        self.api_keys.flush().await
    }

    pub fn try_auth_or_err(&self, token: &str) -> Result<User, Error> {
        self.try_auth(token).ok_or_else(|| Error {
            kind: ErrorKind::Unauthorized,
            source: eyre!("Unauthorized"),
        })
    }

    pub fn login(
        &self,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<JwtToken, Error> {
        check_credentials(self.get_user_by_username(username), password.as_ref())?.create_jwt()
    }
}

/// A hash to check passwords against when the username does not exist, so that a login for a
/// missing user takes as long as one with a wrong password.
static DUMMY_PASSWORD_HASH: LazyLock<HashedPassword> =
    LazyLock::new(|| hash_password("lodestone dummy password"));

fn credential_mismatch() -> Error {
    Error {
        kind: ErrorKind::Unauthorized,
        source: eyre!("Credential mismatch"),
    }
}

fn verify_password(hash: &HashedPassword, password: &str) -> Result<(), Error> {
    if !hash.is_set() {
        // no password matches: as slow as a wrong one, so it doesn't show
        let _ = verify_password(&DUMMY_PASSWORD_HASH, password);
        return Err(credential_mismatch());
    }
    let hash = argon2::PasswordHash::new(hash.as_ref()).map_err(|e| Error {
        kind: ErrorKind::Internal,
        source: eyre!("Stored password hash is invalid: {e}"),
    })?;
    Argon2::default()
        .verify_password(password.as_bytes(), &hash)
        .map_err(|_| credential_mismatch())
}

/// Checks `password` against `user` (as looked up by username) and returns the user.
///
/// This runs Argon2, which is slow on purpose: call it outside the users lock, on a blocking
/// thread. It does the same work whether or not the user exists.
pub fn check_credentials(user: Option<User>, password: &str) -> Result<User, Error> {
    match user {
        Some(user) => {
            verify_password(&user.hashed_psw, password)?;
            Ok(user)
        }
        None => {
            let _ = verify_password(&DUMMY_PASSWORD_HASH, password);
            Err(credential_mismatch())
        }
    }
}

fn decode_token(token: &str, jwt_secret: &UserSecret) -> Option<UserId> {
    match jsonwebtoken::decode::<Claim>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(jwt_secret.as_ref().as_bytes()),
        &Validation::new(Algorithm::HS512),
    ) {
        Ok(t) => Some(t.claims.uid),
        Err(_) => None,
    }
}

/// Reads the claimed uid without checking the signature or `exp`.
///
/// Only used by `try_auth` to look up the claimed user's secret; the token is then fully
/// validated by `decode_token` before it is trusted.
fn decode_no_verify(token: &str) -> Option<UserId> {
    jsonwebtoken::dangerous::insecure_decode_claims::<Claim>(token)
        .ok()
        .map(|claims| claims.uid)
}

impl AsRef<HashMap<UserId, User>> for UsersManager {
    fn as_ref(&self) -> &HashMap<UserId, User> {
        &self.users
    }
}

mod tests {

    #[test]
    fn test_jwt_round_trip() {
        use super::*;
        let user = User::new(
            "test_user1".to_string(),
            "12345",
            true,
            false,
            UserPermission::default(),
        );
        let (tx, _rx) = EventBroadcaster::new(10);
        let users_manager = UsersManager::new(
            tx,
            HashMap::from([(user.uid.clone(), user.clone())]),
            PathBuf::from("users.json"),
        );

        let token = user.create_jwt().unwrap();
        assert_eq!(decode_no_verify(token.as_ref()), Some(user.uid.clone()));
        assert_eq!(
            decode_token(token.as_ref(), &user.secret),
            Some(user.uid.clone())
        );
        assert_eq!(
            users_manager.try_auth(token.as_ref()).map(|u| u.uid),
            Some(user.uid.clone())
        );

        // signed with a different secret: the claimed uid is readable but auth must fail
        let forged = JwtToken::new(
            Claim {
                uid: user.uid.clone(),
                exp: (chrono::Utc::now().timestamp() + 3600) as usize,
            },
            UserSecret::default(),
        )
        .unwrap();
        assert_eq!(decode_no_verify(forged.as_ref()), Some(user.uid.clone()));
        assert!(users_manager.try_auth(forged.as_ref()).is_none());

        // expired (beyond the default 60s leeway)
        let expired = JwtToken::new(
            Claim {
                uid: user.uid.clone(),
                exp: (chrono::Utc::now().timestamp() - 3600) as usize,
            },
            user.secret.clone(),
        )
        .unwrap();
        assert!(users_manager.try_auth(expired.as_ref()).is_none());

        assert!(users_manager.try_auth("not.a.jwt").is_none());
        assert!(users_manager.try_auth("garbage").is_none());
    }

    #[tokio::test]
    async fn test_login() {
        use super::*;
        // create a temporary folder
        let temp_dir = tempfile::TempDir::with_prefix("test_login").unwrap().keep();
        let (tx, _rx) = EventBroadcaster::new(10);
        let mut users_manager =
            UsersManager::new(tx.clone(), HashMap::new(), temp_dir.join("users.json"));
        let test_user1 = User::new(
            "test_user1".to_string(),
            "12345",
            true,
            false,
            UserPermission::default(),
        );

        users_manager
            .add_user(test_user1.clone(), CausedBy::System)
            .await
            .unwrap();

        users_manager.login("test_user1", "12345").unwrap();
    }

    #[tokio::test]
    async fn test_change_password() {
        use super::*;
        // create a temporary folder
        let temp_dir = tempfile::TempDir::with_prefix("test_login").unwrap().keep();
        let (tx, _rx) = EventBroadcaster::new(10);
        let mut users_manager =
            UsersManager::new(tx.clone(), HashMap::new(), temp_dir.join("users.json"));
        let test_user1 = User::new(
            "test_user1".to_string(),
            "12345",
            true,
            false,
            UserPermission::default(),
        );

        users_manager
            .add_user(test_user1.clone(), CausedBy::System)
            .await
            .unwrap();

        users_manager.login("test_user1", "12345").unwrap();

        // the owner's new password has the owner's rule (12 to 256 characters)
        let err = users_manager
            .change_password(
                &test_user1.uid,
                Some("12345"),
                "54321".to_string(),
                CausedBy::System,
            )
            .await
            .unwrap_err();
        assert!(matches!(err.kind, ErrorKind::BadRequest));
        assert!(
            err.source.to_string().contains("12 to 256"),
            "{}",
            err.source
        );
        users_manager.login("test_user1", "12345").unwrap();

        users_manager
            .change_password(
                &test_user1.uid,
                Some("12345"),
                "a much longer password".to_string(),
                CausedBy::System,
            )
            .await
            .unwrap();

        users_manager
            .login("test_user1", "a much longer password")
            .unwrap();
    }

    #[tokio::test]
    async fn other_users_passwords_have_no_owner_minimum() {
        use super::*;
        let temp_dir = tempfile::TempDir::with_prefix("test_member_pw").unwrap();
        let (tx, _rx) = EventBroadcaster::new(10);
        let mut users_manager =
            UsersManager::new(tx, HashMap::new(), temp_dir.path().join("users.json"));
        let member = User::new(
            "member".to_string(),
            "old",
            false,
            false,
            UserPermission::default(),
        );
        users_manager
            .add_user(member.clone(), CausedBy::System)
            .await
            .unwrap();
        users_manager
            .change_password(
                &member.uid,
                Some("old"),
                "new".to_string(),
                CausedBy::System,
            )
            .await
            .unwrap();
        users_manager.login("member", "new").unwrap();
    }

    #[tokio::test]
    async fn test_persistent() {
        use super::*;
        // create a temporary folder
        let temp_dir = tempfile::TempDir::with_prefix("test_login").unwrap().keep();
        let (tx, _rx) = EventBroadcaster::new(10);
        let mut users_manager =
            UsersManager::new(tx.clone(), HashMap::new(), temp_dir.join("users.json"));
        let test_user1 = User::new(
            "test_user1".to_string(),
            "12345",
            true,
            false,
            UserPermission::default(),
        );

        users_manager
            .add_user(test_user1.clone(), CausedBy::System)
            .await
            .unwrap();

        let test_user2 = User::new(
            "test_user2".to_string(),
            "12345",
            true,
            false,
            UserPermission::default(),
        );

        users_manager
            .add_user(test_user2.clone(), CausedBy::System)
            .await
            .unwrap();

        users_manager.get_user_by_username("test_user2").unwrap();

        // delete user
        users_manager
            .delete_user(&test_user2.uid, CausedBy::System)
            .await
            .unwrap();

        assert!(users_manager.get_user_by_username("test_user2").is_none());

        drop(users_manager);

        let (tx, _rx) = EventBroadcaster::new(10);

        let mut users_manager = UsersManager::new(tx, HashMap::new(), temp_dir.join("users.json"));

        assert!(users_manager.get_user_by_username("test_user1").is_none());

        users_manager.load_users().await.unwrap();

        assert!(users_manager.get_user_by_username("test_user1").is_some());
    }
}

#[cfg(test)]
mod permission_tests {
    use super::*;
    use crate::events::Event;

    fn user(is_owner: bool, is_admin: bool, permissions: UserPermission) -> User {
        User::new("user".to_string(), "pw", is_owner, is_admin, permissions)
    }

    fn uuid(s: &str) -> InstanceUuid {
        InstanceUuid::from(format!("INSTANCE_{s}"))
    }

    #[test]
    fn unsafe_actions_match_permission_comments() {
        let i = uuid("a");
        // the "unsafe permission" fields in auth/permission.rs
        assert!(!UserAction::WriteResource(i.clone()).is_safe());
        assert!(!UserAction::WriteInstanceFile(i.clone()).is_safe());
        assert!(!UserAction::WriteGlobalFile.is_safe());
        assert!(!UserAction::ManagePermission.is_safe());
        assert!(!UserAction::ManageUser.is_safe());
        assert!(!UserAction::ReadGlobalFile.is_safe());

        assert!(UserAction::ViewInstance(i.clone()).is_safe());
        assert!(UserAction::StartInstance(i.clone()).is_safe());
        assert!(UserAction::StopInstance(i.clone()).is_safe());
        assert!(UserAction::AccessConsole(i.clone()).is_safe());
        assert!(UserAction::AccessSetting(i.clone()).is_safe());
        assert!(UserAction::ReadResource(i.clone()).is_safe());
        assert!(UserAction::ReadInstanceFile(i).is_safe());
        assert!(UserAction::CreateInstance.is_safe());
        assert!(UserAction::DeleteInstance.is_safe());
    }

    #[test]
    fn safe_mode_blocks_unsafe_actions_for_non_owners_only() {
        let i = uuid("a");
        let mut perms = UserPermission::default();
        perms.can_write_instance_file.insert(i.clone());
        perms.can_view_instance.insert(i.clone());
        let granted = user(false, false, perms);
        let owner = user(true, false, UserPermission::default());

        let action = UserAction::WriteInstanceFile(i.clone());
        assert!(granted.try_action(&action, true).is_err());
        assert!(granted.try_action(&action, false).is_ok());
        assert!(owner.try_action(&action, true).is_ok());
        assert!(granted
            .try_action(&UserAction::ViewInstance(i.clone()), true)
            .is_ok());
        // safe mode never restricts the owner
        assert!(owner.try_action(&UserAction::ManageUser, true).is_ok());
        assert!(owner.try_action(&UserAction::WriteGlobalFile, true).is_ok());
        // and permissions are still needed with safe mode off
        assert!(granted
            .try_action(&UserAction::WriteInstanceFile(uuid("b")), false)
            .is_err());
    }

    #[test]
    fn reading_the_console_needs_the_console_permission_not_just_view() {
        let (viewed, console) = (uuid("viewed"), uuid("console"));
        let mut perms = UserPermission::default();
        perms.can_view_instance.insert(viewed.clone());
        perms.can_view_instance.insert(console.clone());
        perms.can_access_instance_console.insert(console.clone());
        let member = user(false, false, perms);
        let admin = user(false, true, UserPermission::default());
        let owner = user(true, false, UserPermission::default());

        // viewing an instance no longer lets a user read its console
        assert!(member.can_perform_action(&UserAction::ViewInstance(viewed.clone())));
        assert!(!member.can_perform_action(&UserAction::ReadConsole(viewed.clone())));
        assert!(member
            .try_action(&UserAction::ReadConsole(viewed.clone()), false)
            .is_err());
        // the console permission does
        assert!(member.can_perform_action(&UserAction::ReadConsole(console.clone())));
        // admins and the owner read every console
        assert!(admin.can_perform_action(&UserAction::ReadConsole(viewed.clone())));
        assert!(owner
            .try_action(&UserAction::ReadConsole(viewed.clone()), false)
            .is_ok());

        // the same rule filters the event stream: console output is hidden without the
        // permission, while other events about the instance are still shown
        let event = |uuid: &InstanceUuid, inner: InstanceEventInner| {
            EventInner::InstanceEvent(crate::events::InstanceEvent {
                instance_uuid: uuid.clone(),
                instance_name: "server".into(),
                instance_event_inner: inner,
            })
        };
        let output = |uuid: &InstanceUuid| {
            event(
                uuid,
                InstanceEventInner::InstanceOutput {
                    message: "[Server] hi".into(),
                },
            )
        };
        let state = |uuid: &InstanceUuid| {
            event(
                uuid,
                InstanceEventInner::StateTransition {
                    to: crate::traits::t_server::State::Running,
                },
            )
        };
        assert!(!member.can_view_event_parts(&output(&viewed), &CausedBy::System));
        assert!(member.can_view_event_parts(&state(&viewed), &CausedBy::System));
        assert!(member.can_view_event_parts(&output(&console), &CausedBy::System));
    }

    #[test]
    fn manager_cannot_escalate_or_edit_self() {
        let mut manager_perms = UserPermission::default();
        manager_perms.can_manage_permission = true;
        let admin = user(false, true, manager_perms.clone());
        let owner = user(true, false, UserPermission::default());
        let mut target = user(false, false, UserPermission::default());

        // nobody can change their own permissions, not even the owner
        let mut escalated = manager_perms.clone();
        escalated.can_write_global_file = true;
        let mut admin_copy = admin.clone();
        assert!(admin
            .update_permission(&mut admin_copy, escalated.clone())
            .is_err());
        assert_eq!(admin_copy.permissions, manager_perms);
        let mut owner_copy = owner.clone();
        assert!(owner
            .update_permission(&mut owner_copy, escalated.clone())
            .is_err());

        // a non-owner cannot grant any owner exclusive permission
        let i = uuid("a");
        let grants: Vec<fn(&mut UserPermission)> = vec![
            |p| p.can_write_global_file = true,
            |p| p.can_read_global_file = true,
            |p| p.can_manage_permission = true,
            |p| {
                p.can_write_instance_file.insert(uuid("a"));
            },
            |p| {
                p.can_write_instance_resource.insert(uuid("a"));
            },
        ];
        for grant in grants {
            let mut perms = UserPermission::default();
            grant(&mut perms);
            assert!(admin.update_permission(&mut target, perms).is_err());
            assert_eq!(target.permissions, UserPermission::default());
        }

        // safe permissions are fine
        let mut perms = UserPermission::default();
        perms.can_view_instance.insert(i.clone());
        perms.can_start_instance.insert(i.clone());
        admin.update_permission(&mut target, perms.clone()).unwrap();
        assert_eq!(target.permissions, perms);

        // unsafe permissions the owner granted can be kept or removed by the admin
        let mut owner_granted = perms.clone();
        owner_granted.can_write_instance_file.insert(i.clone());
        owner
            .update_permission(&mut target, owner_granted.clone())
            .unwrap();
        let mut kept = owner_granted.clone();
        kept.can_stop_instance.insert(i.clone());
        admin.update_permission(&mut target, kept.clone()).unwrap();
        assert_eq!(target.permissions, kept);
        admin.update_permission(&mut target, perms.clone()).unwrap();
        assert_eq!(target.permissions, perms);

        // the owner can grant anything to others
        owner
            .update_permission(&mut target, escalated.clone())
            .unwrap();
        assert_eq!(target.permissions, escalated);
    }

    #[test]
    fn cannot_manage_equal_or_higher_level() {
        let mut manager_perms = UserPermission::default();
        manager_perms.can_manage_permission = true;
        let manager = user(false, false, manager_perms.clone());
        let admin_manager = user(false, true, manager_perms);
        let mut peer = user(false, false, UserPermission::default());
        let mut admin = user(false, true, UserPermission::default());
        let mut owner = user(true, false, UserPermission::default());
        assert!(manager
            .update_permission(&mut peer, UserPermission::default())
            .is_err());
        assert!(manager
            .update_permission(&mut admin, UserPermission::default())
            .is_err());
        assert!(admin_manager
            .update_permission(&mut admin, UserPermission::default())
            .is_err());
        assert!(admin_manager
            .update_permission(&mut owner, UserPermission::default())
            .is_err());
        // a plain admin without the manage permission cannot either
        let plain_admin = user(false, true, UserPermission::default());
        assert!(plain_admin
            .try_action(&UserAction::ManagePermission, false)
            .is_err());
    }

    #[test]
    fn progression_events_are_filtered() {
        let i = uuid("a");
        let mut viewer_perms = UserPermission::default();
        viewer_perms.can_view_instance.insert(i.clone());
        let viewer = user(false, false, viewer_perms);
        let stranger = user(false, false, UserPermission::default());
        let admin = user(false, true, UserPermission::default());

        let (about_instance, _) = Event::new_progression_event_start(
            "x",
            None,
            Some(ProgressionStartValue::InstanceDelete {
                instance_uuid: i.clone(),
            }),
            CausedBy::System,
        );
        assert!(viewer.can_view_event(&about_instance));
        assert!(admin.can_view_event(&about_instance));
        assert!(!stranger.can_view_event(&about_instance));

        let (no_instance, _) = Event::new_progression_event_start(
            "x",
            None,
            None,
            CausedBy::User {
                user_id: stranger.uid.clone(),
                user_name: stranger.username.clone(),
            },
        );
        assert!(stranger.can_view_event(&no_instance));
        assert!(!viewer.can_view_event(&no_instance));
    }

    /// `user` acting through an API key, full or scoped (custom or launcher).
    fn through_key(mut user: User, full: bool) -> User {
        user.auth_source = AuthSource::ApiKey {
            id: "id".to_string(),
            name: "key".to_string(),
            expires_at: None,
            full,
        };
        user
    }

    #[test]
    fn a_scoped_key_sees_only_the_progress_its_permissions_allow() {
        let a = uuid("a");
        let b = uuid("b");
        let member = user(
            false,
            false,
            UserPermission {
                can_view_instance: [a.clone()].into(),
                can_read_instance_file: [a.clone()].into(),
                ..Default::default()
            },
        );
        let by_member = CausedBy::User {
            user_id: member.uid.clone(),
            user_name: member.username.clone(),
        };
        // the member backed up B, which their key scoped to A can't see
        let scoped = through_key(member.clone(), false);
        let full = through_key(member.clone(), true);
        let (backup_b, _) = Event::new_progression_event_start(
            "backup",
            None,
            Some(ProgressionStartValue::Backup {
                instance_uuid: b.clone(),
            }),
            by_member.clone(),
        );
        assert!(member.can_view_event(&backup_b));
        assert!(full.can_view_event(&backup_b));
        assert!(!scoped.can_view_event(&backup_b));
        // the key still sees A's progress through its permissions
        let (backup_a, _) = Event::new_progression_event_start(
            "backup",
            None,
            Some(ProgressionStartValue::Backup {
                instance_uuid: a.clone(),
            }),
            by_member.clone(),
        );
        assert!(scoped.can_view_event(&backup_a));
        // and nothing about other instances' creation, even with the full InstanceInfo
        let (no_instance, _) = Event::new_progression_event_start("x", None, None, by_member);
        assert!(member.can_view_event(&no_instance));
        assert!(!scoped.can_view_event(&no_instance));
    }

    #[test]
    fn users_see_their_own_account_change() {
        let member = user(false, false, UserPermission::default());
        let other = user(false, false, UserPermission::default());
        let owner = user(true, false, UserPermission::default());
        let changed = |user: &User| {
            EventInner::UserEvent(UserEvent {
                user_id: user.uid.clone(),
                user_event_inner: UserEventInner::PermissionChanged {
                    new_permissions: Box::default(),
                },
            })
        };
        let by_owner = CausedBy::User {
            user_id: owner.uid.clone(),
            user_name: owner.username.clone(),
        };
        assert!(member.can_view_event_parts(&changed(&member), &by_owner));
        assert!(
            through_key(member.clone(), true).can_view_event_parts(&changed(&member), &by_owner)
        );
        assert!(
            !through_key(member.clone(), false).can_view_event_parts(&changed(&member), &by_owner)
        );
        // other users' changes stay for those who manage users
        assert!(!member.can_view_event_parts(&changed(&other), &by_owner));
        assert!(owner.can_view_event_parts(&changed(&other), &by_owner));
    }

    #[test]
    fn usernames_are_checked() {
        assert_eq!(valid_username("  peter  ").unwrap(), "peter");
        assert_eq!(valid_username("Peter's Mac 2").unwrap(), "Peter's Mac 2");
        assert_eq!(valid_username(&"é".repeat(64)).unwrap(), "é".repeat(64));
        for bad in [
            "",
            "   ",
            "home:peter",
            ":",
            "a\nb",
            "tab\there",
            &"x".repeat(65),
        ] {
            assert!(valid_username(bad).is_err(), "{bad:?}");
        }
    }

    #[tokio::test]
    async fn users_with_unusable_names_are_never_made_or_renamed() {
        let temp_dir = tempfile::TempDir::with_prefix("test_usernames").unwrap();
        let (tx, _rx) = EventBroadcaster::new(10);
        let mut users_manager =
            UsersManager::new(tx, HashMap::new(), temp_dir.path().join("users.json"));
        for bad in ["home:peter", "", " padded ", "bell\u{7}"] {
            let user = User::new(bad.to_string(), "pw", false, false, Default::default());
            let err = users_manager
                .add_user(user, CausedBy::System)
                .await
                .unwrap_err();
            assert!(matches!(err.kind, ErrorKind::BadRequest), "{bad:?}");
        }
        assert!(users_manager.as_ref().is_empty());

        let owner = User::owner_without_password("owner".to_string());
        users_manager
            .add_user(owner.clone(), CausedBy::System)
            .await
            .unwrap();
        let err = users_manager
            .rename_user(&owner.uid, "home:peter".to_string(), CausedBy::System)
            .await
            .unwrap_err();
        assert!(matches!(err.kind, ErrorKind::BadRequest));
        let err = users_manager
            .set_up_owner_account(
                &owner.uid,
                "home:peter".to_string(),
                hash_password("a long enough password"),
                CausedBy::System,
            )
            .await
            .unwrap_err();
        assert!(matches!(err.kind, ErrorKind::BadRequest));
        let owner = users_manager.get_user(&owner.uid).unwrap();
        assert_eq!(owner.username, "owner");
        assert!(!owner.has_password());
        // a name is trimmed on the way in
        users_manager
            .rename_user(&owner.uid, "  peter ".to_string(), CausedBy::System)
            .await
            .unwrap();
        assert_eq!(
            users_manager.get_user(&owner.uid).unwrap().username,
            "peter"
        );
    }

    #[tokio::test]
    async fn rename_rejects_taken_usernames() {
        let temp_dir = tempfile::TempDir::with_prefix("test_rename").unwrap();
        let (tx, _rx) = EventBroadcaster::new(10);
        let mut users_manager =
            UsersManager::new(tx, HashMap::new(), temp_dir.path().join("users.json"));
        let a = User::new(
            "a".to_string(),
            "pw",
            true,
            false,
            UserPermission::default(),
        );
        let b = User::new(
            "b".to_string(),
            "pw",
            false,
            false,
            UserPermission::default(),
        );
        users_manager
            .add_user(a.clone(), CausedBy::System)
            .await
            .unwrap();
        users_manager
            .add_user(b.clone(), CausedBy::System)
            .await
            .unwrap();
        assert!(users_manager
            .rename_user(&b.uid, "a".to_string(), CausedBy::System)
            .await
            .is_err());
        assert_eq!(users_manager.get_user(&b.uid).unwrap().username, "b");
        // keeping your own name is fine
        users_manager
            .rename_user(&b.uid, "b".to_string(), CausedBy::System)
            .await
            .unwrap();
        users_manager
            .rename_user(&b.uid, "c".to_string(), CausedBy::System)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn change_password_logs_out_once() {
        let temp_dir = tempfile::TempDir::with_prefix("test_change_pw").unwrap();
        let (tx, mut rx) = EventBroadcaster::new(10);
        let mut users_manager =
            UsersManager::new(tx, HashMap::new(), temp_dir.path().join("users.json"));
        let a = User::new(
            "a".to_string(),
            "old",
            false,
            false,
            UserPermission::default(),
        );
        users_manager
            .add_user(a.clone(), CausedBy::System)
            .await
            .unwrap();
        let _ = rx.recv().await; // UserCreated
        let token = a.create_jwt().unwrap();
        users_manager
            .change_password(&a.uid, Some("old"), "new".to_string(), CausedBy::System)
            .await
            .unwrap();
        // old tokens stop working
        assert!(users_manager.try_auth(token.as_ref()).is_none());
        let event = rx.recv().await.unwrap();
        assert!(matches!(
            event.event_inner,
            EventInner::UserEvent(UserEvent {
                user_event_inner: UserEventInner::UserLoggedOut,
                ..
            })
        ));
        assert!(rx.try_recv().is_err(), "only one logout event");
        // wrong old password: 403, because 401 means the login itself is not valid
        let err = users_manager
            .change_password(&a.uid, Some("old"), "x".to_string(), CausedBy::System)
            .await
            .unwrap_err();
        assert!(matches!(err.kind, ErrorKind::PermissionDenied));
        users_manager.login("a", "new").unwrap();
    }

    #[test]
    fn login_errors_are_the_same_for_missing_users() {
        let a = User::new(
            "a".to_string(),
            "pw",
            true,
            false,
            UserPermission::default(),
        );
        assert!(check_credentials(Some(a.clone()), "pw").is_ok());
        let wrong = check_credentials(Some(a), "nope").unwrap_err();
        let missing = check_credentials(None, "pw").unwrap_err();
        assert_eq!(wrong.source.to_string(), missing.source.to_string());
        assert!(matches!(wrong.kind, ErrorKind::Unauthorized));
        assert!(matches!(missing.kind, ErrorKind::Unauthorized));
    }

    #[tokio::test]
    async fn users_file_is_replaced_atomically_and_private() {
        let temp_dir = tempfile::TempDir::with_prefix("test_users_file").unwrap();
        let path = temp_dir.path().join("users.json");
        let (tx, _rx) = EventBroadcaster::new(10);
        let mut users_manager = UsersManager::new(tx, HashMap::new(), path.clone());
        let a = User::new(
            "a".to_string(),
            "pw",
            true,
            false,
            UserPermission::default(),
        );
        users_manager.add_user(a, CausedBy::System).await.unwrap();
        assert!(!temp_dir.path().join("users.json.tmp").exists());
        let saved: HashMap<UserId, User> =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.len(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    /// Every permission a non-owner can hold, for every instance in `instances`.
    fn all_permissions(instances: &[InstanceUuid]) -> UserPermission {
        let set: std::collections::HashSet<InstanceUuid> = instances.iter().cloned().collect();
        UserPermission {
            can_view_instance: set.clone(),
            can_start_instance: set.clone(),
            can_stop_instance: set.clone(),
            can_access_instance_console: set.clone(),
            can_access_instance_setting: set.clone(),
            can_read_instance_resource: set.clone(),
            can_write_instance_resource: set.clone(),
            can_access_instance_macro: set.clone(),
            can_read_instance_file: set.clone(),
            can_write_instance_file: set,
            can_create_instance: true,
            can_delete_instance: true,
            can_read_global_file: true,
            can_write_global_file: true,
            can_manage_permission: true,
            can_install_extension: true,
        }
    }

    #[test]
    fn owner_only_helper() {
        let owner = user(true, false, UserPermission::default());
        let admin = user(false, true, all_permissions(&[uuid("a")]));
        assert!(owner.owner_only("do it").is_ok());
        let err = admin.owner_only("manage users").unwrap_err();
        assert!(matches!(err.kind, ErrorKind::PermissionDenied));
        assert_eq!(err.source.to_string(), "Only the owner can manage users");
    }

    #[test]
    fn permission_updates_store_legacy_fields_as_false() {
        let i = uuid("a");
        let owner = user(true, false, UserPermission::default());
        let manager_perms = UserPermission {
            can_manage_permission: true,
            ..Default::default()
        };
        let admin = user(false, true, manager_perms);

        // the owner sending them does not fail, and they are not stored
        let mut target = user(false, false, UserPermission::default());
        let mut perms = UserPermission::default();
        perms.can_view_instance.insert(i.clone());
        perms.can_access_instance_macro.insert(i.clone());
        perms.can_install_extension = true;
        owner.update_permission(&mut target, perms).unwrap();
        assert!(target.permissions.can_access_instance_macro.is_empty());
        assert!(!target.permissions.can_install_extension);
        assert!(target.permissions.can_view_instance.contains(&i));

        // nor does an admin sending them
        let mut perms = target.permissions.clone();
        perms.can_access_instance_macro.insert(i.clone());
        perms.can_install_extension = true;
        perms.can_start_instance.insert(i.clone());
        admin.update_permission(&mut target, perms).unwrap();
        assert!(target.permissions.can_access_instance_macro.is_empty());
        assert!(!target.permissions.can_install_extension);
        assert!(target.permissions.can_start_instance.contains(&i));

        let mut perms = all_permissions(&[i]);
        assert!(perms.clear_owner_only_legacy_fields());
        assert!(!perms.clear_owner_only_legacy_fields());
    }

    #[test]
    fn legacy_users_file_still_loads_and_grants_nothing() {
        let i = uuid("a");
        let mut json = serde_json::to_value(user(
            false,
            false,
            all_permissions(std::slice::from_ref(&i)),
        ))
        .unwrap();
        // files written before `can_install_extension` existed
        json["permissions"]
            .as_object_mut()
            .unwrap()
            .remove("can_install_extension");
        let loaded: User = serde_json::from_value(json).unwrap();
        assert!(loaded.permissions.can_access_instance_macro.contains(&i));
    }

    #[test]
    fn progression_and_settings_events_visibility() {
        let i = uuid("a");
        let admin = user(false, true, all_permissions(std::slice::from_ref(&i)));
        let (progression, _) = Event::new_progression_event_start(
            "progress",
            None,
            Some(ProgressionStartValue::InstanceCreation {
                instance_uuid: i.clone(),
            }),
            CausedBy::System,
        );
        assert!(admin.can_view_event(&progression));

        // everyone is told to fetch the settings again
        let viewer = user(false, false, UserPermission::default());
        assert!(viewer.can_view_event_parts(&EventInner::GlobalSettingsChanged, &CausedBy::System));
    }

    #[test]
    fn backup_progress_is_shown_to_users_who_may_read_the_files() {
        let i = uuid("a");
        let view_only = user(
            false,
            false,
            UserPermission {
                can_view_instance: std::collections::HashSet::from([i.clone()]),
                ..Default::default()
            },
        );
        let reader = user(
            false,
            false,
            UserPermission {
                can_view_instance: std::collections::HashSet::from([i.clone()]),
                can_read_instance_file: std::collections::HashSet::from([i.clone()]),
                ..Default::default()
            },
        );
        let starts = [
            ProgressionStartValue::Backup {
                instance_uuid: i.clone(),
            },
            ProgressionStartValue::Restore {
                instance_uuid: i.clone(),
            },
        ];
        for start in starts {
            let (event, id) =
                Event::new_progression_event_start("p", None, Some(start), CausedBy::System);
            assert!(reader.can_view_event(&event));
            assert!(!view_only.can_view_event(&event));
            let end = Event::new_progression_event_end(
                id,
                true,
                Some("done"),
                Some(ProgressionEndValue::BackupCompleted {
                    instance_uuid: i.clone(),
                    backup_id: None,
                    success: true,
                    message: "done".to_string(),
                }),
            );
            assert!(reader.can_view_event(&end));
            assert!(!view_only.can_view_event(&end));
        }
    }
}
