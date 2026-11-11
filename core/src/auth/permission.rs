use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use tracing::info;
use ts_rs::TS;

use crate::types::InstanceUuid;
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, TS, Debug)]
#[ts(export)]
pub struct UserPermission {
    pub can_view_instance: HashSet<InstanceUuid>,
    pub can_start_instance: HashSet<InstanceUuid>,
    pub can_stop_instance: HashSet<InstanceUuid>,
    pub can_access_instance_console: HashSet<InstanceUuid>,
    pub can_access_instance_setting: HashSet<InstanceUuid>,
    pub can_read_instance_resource: HashSet<InstanceUuid>,
    // unsafe permission, owner exclusive unless explicitly granted
    pub can_write_instance_resource: HashSet<InstanceUuid>,
    // No longer grants anything: macros were removed. Kept so that existing users.json
    // files and clients still load; updates store it empty.
    pub can_access_instance_macro: HashSet<InstanceUuid>,
    pub can_read_instance_file: HashSet<InstanceUuid>,
    // unsafe permission, owner exclusive unless explicitly granted
    pub can_write_instance_file: HashSet<InstanceUuid>,

    pub can_create_instance: bool,
    pub can_delete_instance: bool,
    pub can_read_global_file: bool,
    // unsafe permission, owner exclusive unless explicitly granted
    pub can_write_global_file: bool,
    // owner exclusive unless explicitly granted
    pub can_manage_permission: bool,
    // No longer grants anything: extensions were removed. Kept so that existing
    // users.json files and clients still load; updates store it as false.
    #[serde(default)]
    pub can_install_extension: bool,
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
            can_install_extension: false,
        }
    }
}

impl UserPermission {
    /// Clears `can_access_instance_macro` and `can_install_extension`, which grant nothing
    /// since macros and extensions were removed. Returns whether anything was set,
    /// and logs it, so older clients that still send them keep working.
    pub fn clear_owner_only_legacy_fields(&mut self) -> bool {
        let had_legacy = !self.can_access_instance_macro.is_empty() || self.can_install_extension;
        if had_legacy {
            info!(
                "Ignoring the macro and extension permissions in a permission update: macros \
                 and extensions were removed"
            );
            self.can_access_instance_macro.clear();
            self.can_install_extension = false;
        }
        had_legacy
    }
}

impl Default for UserPermission {
    fn default() -> Self {
        Self::new()
    }
}
