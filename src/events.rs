use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::util::{DownloadProgress, SetupProgress};

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub enum InstanceEventInner {
    InstanceStarting,
    InstanceStarted,
    InstanceStopping,
    InstanceStopped,
    InstanceWarning,
    InstanceError,
    InstanceCreationFailed,
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
pub struct InstanceEvent {
    pub instance_uuid: String,
    pub instance_name: String,
    pub instance_event_inner: InstanceEventInner,
}
#[derive(Serialize, Deserialize, Clone, Debug, TS)]

pub enum UserEventInner {
    UserCreated,
    UserDeleted,
    UserLoggedIn,
    UserLoggedOut,
}
#[derive(Serialize, Deserialize, Clone, Debug, TS)]
pub struct UserEvent {
    pub user_id: String,
    pub user_event_inner: UserEventInner,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
pub enum EventInner {
    InstanceEvent(InstanceEvent),
    UserEvent(UserEvent),
}
#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub struct Event {
    pub event_inner: EventInner,
    pub details: String,
    pub timestamp: i64,
    pub idempotency: String,
}
