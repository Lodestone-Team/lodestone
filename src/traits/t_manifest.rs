use std::collections::HashSet;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Hash, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub enum Operation {
    GetMinRam,
    GetMaxRam,
    GetRestartOnCrash,
    GetTimeoutLastLeft,
    GetTimeoutNoActivity,
    GetStartOnConnection,
    GetBackupPeriod,

    SetMinRam,
    SetMaxRam,
    SetPort,
    SetRestartOnCrash,
    SetTimeoutLastLeft,
    SetTimeoutNoActivity,
    SetStartOnConnection,
    SetBackupPeriod,

    RunMacro,

    GetPlayerCount,
    GetMaxPlayerCount,
    GetPlayerList,

    LoadResource,
    UnloadResource,
    DeleteResource,
}

impl Operation {
    pub fn all() -> HashSet<Operation> {
        HashSet::from([
            Operation::GetMinRam,
            Operation::GetMaxRam,
            Operation::GetRestartOnCrash,
            Operation::GetTimeoutLastLeft,
            Operation::GetTimeoutNoActivity,
            Operation::GetStartOnConnection,
            Operation::GetBackupPeriod,
            Operation::SetMinRam,
            Operation::SetMaxRam,
            Operation::SetRestartOnCrash,
            Operation::SetTimeoutLastLeft,
            Operation::SetTimeoutNoActivity,
            Operation::SetStartOnConnection,
            Operation::SetBackupPeriod,
            Operation::RunMacro,
            Operation::GetPlayerCount,
            Operation::GetMaxPlayerCount,
            Operation::GetPlayerList,
            Operation::LoadResource,
            Operation::UnloadResource,
            Operation::DeleteResource,
        ])
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    pub supported_operations: HashSet<Operation>,
    pub settings: HashSet<String>,
}

#[async_trait]
pub trait TManifest {
    async fn get_manifest(&self) -> Manifest;
}
