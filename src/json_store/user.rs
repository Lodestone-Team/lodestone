use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::permission::Permission;
#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub uid: String,
    pub username: String,
    pub hashed_psw: String,
    pub is_owner: bool,
    pub is_admin: bool,
    pub permissions: HashMap<Permission, HashSet<String>>,
    pub secret: String,
}

#[derive(Serialize, Deserialize, Clone, TS)]
#[ts(export)]
pub struct PublicUser {
    pub uid: String,
    pub username: String,
    pub is_owner: bool,
    pub is_admin: bool,
    pub permissions: HashMap<Permission, HashSet<String>>,
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

