use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{output_types::ClientEvent, prelude::get_snowflake, traits::InstanceInfo};

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
#[serde(tag = "type")]
#[derive(enum_kinds::EnumKind)]
#[enum_kind(InstanceEventKind, derive(Serialize, Deserialize, TS))]
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

impl AsRef<InstanceEventInner> for InstanceEventInner {
    fn as_ref(&self) -> &InstanceEventInner {
        self
    }
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
#[derive(enum_kinds::EnumKind)]
#[enum_kind(UserEventKind, derive(Serialize, Deserialize, TS))]
pub enum UserEventInner {
    UserCreated,
    UserDeleted,
    UserLoggedIn,
    UserLoggedOut,
}

impl AsRef<UserEventInner> for UserEventInner {
    fn as_ref(&self) -> &UserEventInner {
        self
    }
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
#[derive(enum_kinds::EnumKind)]
#[enum_kind(MacroEventKind, derive(Serialize, Deserialize, TS))]
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

impl Into<Event> for MacroEvent {
    fn into(self) -> Event {
        Event {
            details: "".to_string(),
            snowflake: get_snowflake(),
            event_inner: EventInner::MacroEvent(self),
        }
    }
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
        progress_message: String,
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
    format!("PROG_{}", get_snowflake())
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
#[serde(tag = "type")]
#[derive(enum_kinds::EnumKind)]
#[enum_kind(EventType, derive(Serialize, Deserialize, TS))]
pub enum EventInner {
    InstanceEvent(InstanceEvent),
    UserEvent(UserEvent),
    MacroEvent(MacroEvent),
    ProgressionEvent(ProgressionEvent),
}

impl AsRef<EventInner> for EventInner {
    fn as_ref(&self) -> &EventInner {
        self
    }
}

#[test]
fn event_type_export() {
    let _ = EventType::export();
    let _ = MacroEventKind::export();
    let _ = UserEventKind::export();
    let _ = InstanceEventKind::export();
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[serde(into = "ClientEvent")]
pub struct Event {
    pub event_inner: EventInner,
    pub details: String,
    pub snowflake: i64,
}
#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub enum EventLevel {
    Info,
    Warning,
    Error,
}

// impl From<&EventInner> for EventType {
//     fn from(event_inner: &EventInner) -> Self {
//         match event_inner {
//             EventInner::InstanceEvent(_) => EventType::InstanceEvent,
//             EventInner::UserEvent(_) => EventType::UserEvent,
//             EventInner::MacroEvent(_) => EventType::MacroEvent,
//             EventInner::ProgressionEvent(_) => EventType::ProgressionEvent,
//         }
//     }
// }

impl From<&ClientEvent> for Event {
    fn from(client_event: &ClientEvent) -> Event {
        Event {
            event_inner: client_event.event_inner.clone(),
            details: client_event.details.clone(),
            snowflake: client_event.snowflake.clone(),
        }
    }
}

impl AsRef<Event> for Event {
    fn as_ref(&self) -> &Event {
        self
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
