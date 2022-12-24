use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, TS, Debug)]
#[ts(export)]
pub struct UserPermission {
    pub can_view_instance: HashSet<String>,
    pub can_start_instance: HashSet<String>,
    pub can_stop_instance: HashSet<String>,
    pub can_access_instance_console: HashSet<String>,
    pub can_access_instance_setting: HashSet<String>,
    pub can_read_instance_resource: HashSet<String>,
    // unsafe permission, owner exclusive unless explicitly granted
    pub can_write_instance_resource: HashSet<String>,
    // unsafe permission, owner exclusive unless explicitly granted
    pub can_access_instance_macro: HashSet<String>,
    pub can_read_instance_file: HashSet<String>,
    // unsafe permission, owner exclusive unless explicitly granted
    pub can_write_instance_file: HashSet<String>,

    pub can_create_instance: bool,
    pub can_delete_instance: bool,
    pub can_read_global_file: bool,
    // unsafe permission, owner exclusive unless explicitly granted
    pub can_write_global_file: bool,
    // owner exclusive unless explicitly granted
    pub can_manage_permission: bool,
}

impl UserPermission {
    pub fn new() -> Self {
        UserPermission {
            can_view_instance: HashSet::new(),
            can_start_instance: HashSet::new(),
            can_stop_instance: HashSet::new(),
            can_access_instance_console: HashSet::new(),
            can_access_instance_setting: HashSet::new(),
            can_read_instance_resource: HashSet::new(),
            can_write_instance_resource: HashSet::new(),
            can_access_instance_macro: HashSet::new(),
            can_read_instance_file: HashSet::new(),
            can_write_instance_file: HashSet::new(),
            can_create_instance: false,
            can_delete_instance: false,
            can_read_global_file: false,
            can_write_global_file: false,
            can_manage_permission: false,
        }
    }
}

impl Default for UserPermission {
    fn default() -> Self {
        Self::new()
    }
}
