use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub enum Permission {
    // Instance specific permissions:
    CanViewInstance,
    CanStartInstance,
    CanStopInstance,
    CanAccessConsole,
    CanChangeSetting,
    CanManageResource,
    CanAccessMacro,

    // Global permissions:
    CanCreateInstance,
    CanDeleteInstance,
    CanManageUser,
    CanManagePermission,
}
