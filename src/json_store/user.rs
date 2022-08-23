use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::permission::Permission;
#[derive(Serialize, Deserialize, Clone)]
pub struct User {
    pub uid: String,
    pub username: String,
    pub hashed_psw: String,
    pub salt : String,
    pub is_owner : bool,
    pub is_admin : bool,
    pub permissions: HashMap<Permission, HashSet<String>>,
    pub secret: String
}
