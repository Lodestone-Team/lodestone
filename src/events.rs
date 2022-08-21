use std::collections::HashSet;

use serde::{Serialize, Deserialize};
use ts_rs::TS;

use crate::util::{DownloadProgress, SetupProgress};


#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub enum EventInner {
    InstanceStarting,
    InstanceStarted,
    InstanceStopping,
    InstanceStopped,
    InstanceWarning,
    InstanceError,
    InstanceInput(String),
    InstanceOutput(String),
    SystemMessage(String),
    PlayerChange(HashSet<String>),
    PlayerJoined(String),
    PlayerLeft(String),
    PlayerMessage(String, String),
    Downloading(DownloadProgress),
    Setup(SetupProgress),
}
#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub struct Event {
    pub event_inner: EventInner,
    pub instance_uuid : String,
    pub instance_name : String,
    pub details: String,
}