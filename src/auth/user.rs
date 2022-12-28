use std::{collections::HashMap, path::PathBuf};

use argon2::{Argon2, PasswordVerifier};
use jsonwebtoken::{Algorithm, Validation};
use log::{warn, error};
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncWriteExt, sync::broadcast::Sender};
use ts_rs::TS;

use crate::{
    events::{CausedBy, Event, EventInner, UserEvent, UserEventInner},
    traits::{Error, ErrorInner},
    types::{InstanceUuid, Snowflake},
};

use super::{
    hashed_password::{hash_password, HashedPassword},
    jwt_token::JwtToken,
    permission::UserPermission,
    user_id::UserId,
    user_secrets::UserSecret,
};

#[derive(Deserialize, Serialize)]
pub struct Claim {
    pub uid: UserId,
    pub exp: usize,
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
    pub fn update_permission(
        &self,
        other: &mut User,
        permissions: UserPermission,
    ) -> Result<(), Error> {
        if self.get_permission_level() <= other.get_permission_level() {
            return Err(Error {
                inner: ErrorInner::PermissionDenied,
                detail: "You do not have permission to update this user's permissions.".to_string(),
            });
        }
        if self.is_owner {
            other.permissions = permissions;
            Ok(())
        } else {
            // reject granting any unsafe permission
            if !permissions.can_write_instance_resource.is_empty()
                || !permissions.can_access_instance_macro.is_empty()
                || permissions.can_write_global_file
                || permissions.can_manage_permission
                || !permissions.can_write_instance_file.is_empty()
            {
                Err(Error {
                    inner: ErrorInner::PermissionDenied,
                    detail:
                        "Unsafe and owner exclusive permissions can only be granted by the owner"
                            .to_string(),
                })
            } else if self.is_admin || self.permissions.can_manage_permission {
                other.permissions = permissions;
                Ok(())
            } else {
                Err(Error {
                    inner: ErrorInner::PermissionDenied,
                    detail: "You don't have permission to manage other users' permission"
                        .to_string(),
                })
            }
        }
    }

    pub fn can_perform_action(&self, action: &UserAction) -> bool {
        if self.is_owner {
            return true;
        }
        match action {
            UserAction::ViewInstance(instance_id) => {
                self.is_admin || self.permissions.can_view_instance.contains(instance_id)
            }
            UserAction::StartInstance(instance_id) => {
                self.is_admin || self.permissions.can_start_instance.contains(instance_id)
            }
            UserAction::StopInstance(instance_id) => {
                self.is_admin || self.permissions.can_stop_instance.contains(instance_id)
            }
            UserAction::AccessConsole(instance_id) => {
                self.is_admin
                    || self
                        .permissions
                        .can_access_instance_console
                        .contains(instance_id)
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
            UserAction::AccessMacro(instance_id) => self
                .permissions
                .can_access_instance_macro
                .contains(instance_id),
            UserAction::CreateInstance => self.is_admin || self.permissions.can_create_instance,
            UserAction::DeleteInstance => self.is_admin || self.permissions.can_delete_instance,
            UserAction::ReadGlobalFile => self.permissions.can_read_global_file,
            UserAction::WriteGlobalFile => self.permissions.can_write_global_file,
            UserAction::ManageUser => self.is_owner,
            UserAction::ManagePermission => self.permissions.can_manage_permission,
        }
    }

    pub fn can_view_event(&self, event: impl AsRef<Event>) -> bool {
        match &event.as_ref().event_inner {
            EventInner::InstanceEvent(event) => {
                self.can_perform_action(&UserAction::ViewInstance(event.instance_uuid.clone()))
            }
            EventInner::UserEvent(_event) => self.can_perform_action(&UserAction::ManageUser),
            EventInner::FSEvent(_) => self.can_perform_action(&UserAction::ManageUser),
            EventInner::MacroEvent(macro_event) => {
                self.can_perform_action(&UserAction::AccessMacro(macro_event.instance_uuid.clone()))
            }
            // TODO!,
            EventInner::ProgressionEvent(_progression_event) => true,
        }
    }

    pub fn create_jwt(&self) -> Result<JwtToken, Error> {
        let exp = chrono::Utc::now()
            .checked_add_signed(chrono::Duration::days(60))
            .ok_or(Error {
                inner: ErrorInner::InternalError,
                detail: "Failed to generate JWT".to_string(),
            })?
            .timestamp();
        let claim = Claim {
            uid: self.uid.clone(),
            exp: exp as usize,
        };

        JwtToken::new(claim, self.secret.clone())
    }
}

pub enum UserAction {
    // instance specific actions:
    ViewInstance(InstanceUuid),
    StartInstance(InstanceUuid),
    StopInstance(InstanceUuid),
    AccessConsole(InstanceUuid),
    AccessSetting(InstanceUuid),
    ReadResource(InstanceUuid),
    WriteResource(InstanceUuid),
    AccessMacro(InstanceUuid),
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

#[derive(Serialize, Deserialize, Clone, TS)]
#[ts(export)]
pub struct PublicUser {
    pub uid: UserId,
    pub username: String,
    pub is_owner: bool,
    pub is_admin: bool,
    pub permissions: UserPermission,
}

impl From<&User> for PublicUser {
    fn from(user: &User) -> Self {
        PublicUser {
            uid: user.uid.clone(),
            username: user.username.clone(),
            is_owner: user.is_owner,
            is_admin: user.is_admin,
            permissions: user.permissions.clone(),
        }
    }
}

impl From<User> for PublicUser {
    fn from(user: User) -> Self {
        PublicUser {
            uid: user.uid,
            username: user.username,
            is_owner: user.is_owner,
            is_admin: user.is_admin,
            permissions: user.permissions,
        }
    }
}

#[derive(Clone)]
pub struct UsersManager {
    event_broadcaster: Sender<Event>,
    users: HashMap<UserId, User>,
    path_to_users: PathBuf,
}

impl UsersManager {
    pub fn new(
        event_broadcaster: Sender<Event>,
        users: HashMap<UserId, User>,
        path_to_users: PathBuf,
    ) -> Self {
        Self {
            event_broadcaster,
            users,
            path_to_users,
        }
    }
    pub async fn load_users(&mut self) -> Result<(), Error> {
        if tokio::fs::OpenOptions::new()
            .read(true)
            .create(true)
            .write(true)
            .open(&self.path_to_users)
            .await
            .map_err(|e| Error {
                inner: ErrorInner::FailedToReadFileOrDir,
                detail: format!("Failed to open user file: {}", e),
            })?
            .metadata()
            .await
            .map_err(|e| Error {
                inner: ErrorInner::MalformedFile,
                detail: format!("Failed to read metadata : {}", e),
            })?
            .len()
            == 0
        {
            warn!("No user file found, creating a new one");
            self.users = HashMap::new();
        } else {
            let users: HashMap<UserId, User> = serde_json::from_reader(
                tokio::fs::File::open(&self.path_to_users)
                    .await
                    .map_err(|e| Error {
                        inner: ErrorInner::FailedToReadFileOrDir,
                        detail: format!("Failed to open user file: {}", e),
                    })?
                    .into_std()
                    .await,
            )
            .map_err(|e| Error {
                inner: ErrorInner::MalformedFile,
                detail: format!("Failed to deserialize users: {}", e),
            })?;
            self.users = users;
        }
        Ok(())
    }

    async fn write_to_file(&self) -> Result<(), Error> {
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(&self.path_to_users)
            .await
            .map_err(|e| Error {
                inner: ErrorInner::InternalError,
                detail: format!("Failed to open user file: {}", e),
            })?;

        file.write_all(serde_json::to_string(&self.users).unwrap().as_bytes())
            .await
            .map_err(|e| Error {
                inner: ErrorInner::InternalError,
                detail: format!("Failed to serialize users: {}", e),
            })?;
        Ok(())
    }
    pub fn get_user(&self, uid: impl AsRef<UserId>) -> Option<User> {
        self.users.get(uid.as_ref()).cloned()
    }
    pub async fn add_user(&mut self, user: User, caused_by: CausedBy) -> Result<(), Error> {
        if self.get_user_by_username(&user.username).is_some() {
            return Err(Error {
                inner: ErrorInner::UsernameAlreadyExists,
                detail: "User already exists".to_string(),
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
                self.users
                    .insert(uid.as_ref().to_owned(), user.clone().unwrap());
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
                inner: ErrorInner::UserNotFound,
                detail: "User not found".to_string(),
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

    pub async fn change_password(
        &mut self,
        uid: impl AsRef<UserId>,
        password: String,
        caused_by: CausedBy,
    ) -> Result<(), Error> {
        let old_psw = self
            .users
            .get_mut(uid.as_ref())
            .ok_or_else(|| Error {
                inner: ErrorInner::UserNotFound,
                detail: "User not found".to_string(),
            })?
            .hashed_psw
            .clone();
        if let Some(user) = self.users.get_mut(uid.as_ref()) {
            user.hashed_psw = hash_password(password);
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
                    caused_by: caused_by.clone(),
                });
                self.logout_user(uid, caused_by).await
            }
            Err(_) => {
                if let Some(user) = self.users.get_mut(uid.as_ref()) {
                    user.hashed_psw = old_psw;
                }
                Err(Error {
                    inner: ErrorInner::InternalError,
                    detail: "Failed to write to file".to_string(),
                })
            }
        }
    }

    pub fn get_user_by_username(&self, username: impl AsRef<str>) -> Option<User> {
        self.users
            .values()
            .find(|user| user.username == username.as_ref())
            .cloned()
    }

    pub async fn update_permissions(
        &mut self,
        uid: impl AsRef<UserId>,
        new_permissions: UserPermission,
        caused_by: CausedBy,
    ) -> Result<(), Error> {
        let old_permission = self
            .users
            .get_mut(uid.as_ref())
            .ok_or_else(|| Error {
                inner: ErrorInner::UserNotFound,
                detail: "User not found".to_string(),
            })?
            .permissions
            .clone();
        if let Some(user) = self.users.get_mut(uid.as_ref()) {
            user.permissions = new_permissions.clone();
        }
        match self.write_to_file().await {
            Ok(_) => {
                self.event_broadcaster.send(Event {
                    event_inner: EventInner::UserEvent(UserEvent {
                        user_id: uid.as_ref().to_owned(),
                        user_event_inner: UserEventInner::PermissionChanged(Box::new(
                            new_permissions,
                        )),
                    }),
                    details: "".to_string(),
                    snowflake: Snowflake::default(),
                    caused_by,
                });
                Ok(())
            }
            Err(_) => {
                if let Some(user) = self.users.get_mut(uid.as_ref()) {
                    user.permissions = old_permission;
                }
                Err(Error {
                    inner: ErrorInner::InternalError,
                    detail: "Failed to write to file".to_string(),
                })
            }
        }
    }

    pub fn try_auth(&self, token: &str) -> Option<User> {
        let claimed_uid = decode_no_verify(token)?;
        let claimed_requester = self.users.get(&claimed_uid)?;
        let requester_uid = decode_token(token, &claimed_requester.secret)?;
        if claimed_uid != requester_uid {
            return None;
        }
        Some(claimed_requester.to_owned())
    }

    pub fn login(
        &self,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<JwtToken, Error> {
        let user = self.get_user_by_username(username).ok_or_else(|| Error {
            inner: ErrorInner::Unauthorized,
            detail: "Username not found".to_string(),
        })?;
        Argon2::default()
            .verify_password(
                password.as_ref().as_bytes(),
                &argon2::PasswordHash::new(user.hashed_psw.as_ref()).unwrap(),
            )
            .map_err(|_| Error {
                inner: ErrorInner::Unauthorized,
                detail: "Wrong username or password".to_string(),
            })?;
        user.create_jwt()
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

fn decode_no_verify(token: &str) -> Option<UserId> {
    let mut no_verify = Validation::new(Algorithm::HS512);
    no_verify.insecure_disable_signature_validation();
    match jsonwebtoken::decode::<Claim>(
        token,
        &jsonwebtoken::DecodingKey::from_secret("noverify".as_bytes()),
        &no_verify,
    ) {
        Ok(t) => Some(t.claims.uid),
        Err(_) => None,
    }
}

impl AsRef<HashMap<UserId, User>> for UsersManager {
    fn as_ref(&self) -> &HashMap<UserId, User> {
        &self.users
    }
}

mod tests {

    #[tokio::test]
    async fn test_login() {
        use super::*;
        // create a temporary folder
        let temp_dir = tempdir::TempDir::new("test_login").unwrap().into_path();
        let (tx, _rx) = tokio::sync::broadcast::channel(10);
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
    async fn test_persistent() {
        use super::*;
        // create a temporary folder
        let temp_dir = tempdir::TempDir::new("test_login").unwrap().into_path();
        let (tx, _rx) = tokio::sync::broadcast::channel(10);
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

        users_manager.get_user_by_username("test_user1").unwrap();

        drop(users_manager);

        let (tx, _rx) = tokio::sync::broadcast::channel(10);

        let mut users_manager = UsersManager::new(tx, HashMap::new(), temp_dir.join("users.json"));

        assert!(users_manager.get_user_by_username("test_user1").is_none());

        users_manager.load_users().await.unwrap();

        assert!(users_manager.get_user_by_username("test_user1").is_some());
    }
}
