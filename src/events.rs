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
    PlayerChange {
        player_list: HashSet<String>,
        players_joined : HashSet<String>,
        players_left : HashSet<String>,
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
pub enum MacroEventInner {
    MacroStarted,
    MacroStopped,
    MacroErrored { error_msg: String },
}
#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub struct MacroEvent {
    pub instance_uuid: String,
    pub macro_uuid: String,
    pub macro_event_inner: MacroEventInner,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum EventInner {
    InstanceEvent(InstanceEvent),
    UserEvent(UserEvent),
    MacroEvent(MacroEvent),
}
#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[serde(rename = "ClientEvent")]
#[ts(export)]
pub struct Event {
    pub event_inner: EventInner,
    pub details: String,
    pub timestamp: i64,
    pub idempotency: String,
}

impl Event {
    pub fn is_event_console_message(&self) -> bool {
        match &self.event_inner {
            EventInner::InstanceEvent(instance_event) => matches!(
                &instance_event.instance_event_inner,
                InstanceEventInner::InstanceOutput { .. }
                    | InstanceEventInner::PlayerMessage { .. }
                    | InstanceEventInner::SystemMessage { .. }
            ),
            _ => false,
        }
    }
    pub fn try_player_message(&self) -> Option<(String, String)> {
        match &self.event_inner {
            EventInner::InstanceEvent(instance_event) => match &instance_event.instance_event_inner
            {
                InstanceEventInner::PlayerMessage {
                    player,
                    player_message,
                } => Some((player.clone(), player_message.clone())),
                _ => None,
            },
            _ => None,
        }
    }
    pub fn get_instance_uuid(&self) -> Option<String> {
        match &self.event_inner {
            EventInner::InstanceEvent(instance_event) => Some(instance_event.instance_uuid.clone()),
            _ => None,
        }
    }
}
