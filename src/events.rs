use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::util::{DownloadProgress, SetupProgress};

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum InstanceEventInner {
    InstanceStarting,
    InstanceStarted,
    InstanceStopping,
    InstanceStopped,
    InstanceWarning,
    InstanceError,
    InstanceCreationFailed,
    InstanceInput {
        message: String,
    },
    InstanceOutput {
        message: String,
    },
    SystemMessage {
        message: String,
    },
    PlayerChange(HashSet<String>),
    PlayerJoined {
        player: String,
    },
    PlayerLeft {
        player: String,
    },
    PlayerMessage {
        player: String,
        player_message: String,
    },
    Downloading(DownloadProgress),
    Setup(SetupProgress),
}
#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub struct InstanceEvent {
    pub instance_uuid: String,
    pub instance_name: String,
    pub instance_event_inner: InstanceEventInner,
}
#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum UserEventInner {
    UserCreated,
    UserDeleted,
    UserLoggedIn,
    UserLoggedOut,
}
#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub struct UserEvent {
    pub user_id: String,
    pub user_event_inner: UserEventInner,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
#[serde(tag = "type")]
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
