use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::traits::{Error, ErrorInner};

use super::permission::UserPermission;
#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub uid: String,
    pub username: String,
    pub hashed_psw: String,
    pub is_owner: bool,
    pub is_admin: bool,
    pub permissions: UserPermission,
    pub secret: String,
}

impl User {
    pub fn new(
        uid: String,
        username: String,
        hashed_psw: String,
        is_owner: bool,
        is_admin: bool,
        permissions: UserPermission,
        secret: String,
    ) -> Self {
        User {
            uid,
            username,
            hashed_psw,
            is_owner,
            is_admin,
            permissions,
            secret,
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
}

pub enum UserAction {
    // instance specific actions:
    ViewInstance(String),
    StartInstance(String),
    StopInstance(String),
    AccessConsole(String),
    AccessSetting(String),
    ReadResource(String),
    WriteResource(String),
    AccessMacro(String),
    ReadInstanceFile(String),
    WriteInstanceFile(String),

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
    pub uid: String,
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
