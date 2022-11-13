use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{prelude::get_snowflake, traits::InstanceInfo};

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
        players_joined: HashSet<String>,
        players_left: HashSet<String>,
    },

    PlayerMessage {
        player: String,
        player_message: String,
    },
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
pub enum ProgressionEndValue {
    InstanceInfo(InstanceInfo),
}

// the backend will keep exactly 1 copy of ProgressionStart, and 1 copy of ProgressionUpdate OR ProgressionEnd
#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum ProgressionEventInner {
    ProgressionStart {
        progression_name: String,
        producer_id: String,
        total: Option<f64>,
    },
    ProgressionUpdate {
        progress_message: Option<String>,
        progress: f64,
    },
    ProgressionEnd {
        success: bool,
        message: Option<String>,
        value: Option<ProgressionEndValue>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub struct ProgressionEvent {
    pub event_id: String,
    pub progression_event_inner: ProgressionEventInner,
}

pub fn new_progression_event_id() -> String {
    format!("PROGRESSION_{}", get_snowflake())
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum EventInner {
    InstanceEvent(InstanceEvent),
    UserEvent(UserEvent),
    MacroEvent(MacroEvent),
    ProgressionEvent(ProgressionEvent),
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[serde(into = "ClientEvent")]
pub struct Event {
    pub event_inner: EventInner,
    pub details: String,
    pub snowflake: i64,
}

// a type that Event will be serialized to
// used to serialize snowflake as string
#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
struct ClientEvent {
    pub event_inner: EventInner,
    pub details: String,
    pub snowflake: i64,
    pub snowflake_str: String,
}

impl Into<ClientEvent> for Event {
    fn into(self) -> ClientEvent {
        ClientEvent {
            event_inner: self.event_inner,
            details: self.details,
            snowflake: self.snowflake,
            snowflake_str: self.snowflake.to_string(),
        }
    }
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
