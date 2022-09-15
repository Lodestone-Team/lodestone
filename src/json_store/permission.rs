use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, TS)]
#[ts(export)]
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
