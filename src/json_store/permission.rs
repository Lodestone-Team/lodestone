use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum Permission {
    // Instance specific permissions:
    CanCreateInstance,
    CanDeleteInstance,
    CanStartInstance,
    CanStopInstance,
    CanAccessConsole,
    CanChangeSetting,
    CanManageResource,
    CanAccessMacro,

    // Global permissions:
    CanManageUser,
    CanManagePermission,
}
