#![allow(clippy::enum_variant_names)]

use std::{collections::HashSet, path::PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    auth::{permission::UserPermission, user_id::UserId},
    macro_executor::MacroPID,
    output_types::ClientEvent,
    traits::{t_macro::ExitStatus, t_player::Player, t_server::State, InstanceInfo},
    types::{InstanceUuid, Snowflake},
};

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
#[serde(tag = "type")]
#[derive(enum_kinds::EnumKind)]
#[enum_kind(InstanceEventKind, derive(Serialize, Deserialize, TS))]
pub enum InstanceEventInner {
    StateTransition {
        to: State,
    },
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
        player_list: HashSet<Player>,
        players_joined: HashSet<Player>,
        players_left: HashSet<Player>,
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

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct InstanceEvent {
    pub instance_uuid: InstanceUuid,
    pub instance_name: String,
    pub instance_event_inner: InstanceEventInner,
}
#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
#[serde(tag = "type")]
#[derive(enum_kinds::EnumKind)]
#[enum_kind(UserEventKind, derive(Serialize, Deserialize, TS))]
pub enum UserEventInner {
    UserCreated,
    UserDeleted,
    UserLoggedIn,
    UserLoggedOut,
    UsernameChanged {
        new_username: String,
    },
    PermissionChanged {
        new_permissions: Box<UserPermission>,
    },
}

impl AsRef<UserEventInner> for UserEventInner {
    fn as_ref(&self) -> &UserEventInner {
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct UserEvent {
    pub user_id: UserId,
    pub user_event_inner: UserEventInner,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
#[serde(tag = "type")]
#[derive(enum_kinds::EnumKind)]
#[enum_kind(MacroEventKind, derive(Serialize, Deserialize, TS))]
pub enum MacroEventInner {
    Started,
    Stopped { exit_status: ExitStatus },
}
#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct MacroEvent {
    pub instance_uuid: Option<InstanceUuid>,
    pub macro_pid: MacroPID,
    pub macro_event_inner: MacroEventInner,
}

impl From<MacroEvent> for Event {
    fn from(val: MacroEvent) -> Self {
        Event {
            details: "".to_string(),
            snowflake: Snowflake::default(),
            event_inner: EventInner::MacroEvent(val.clone()),
            caused_by: CausedBy::Macro {
                macro_pid: val.macro_pid,
            },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
#[serde(tag = "type")]
pub enum ProgressionEndValue {
    InstanceCreation(InstanceInfo),
    InstanceDelete { instance_uuid: InstanceUuid },
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
#[serde(tag = "type")]
pub enum ProgressionStartValue {
    InstanceCreation {
        instance_uuid: InstanceUuid,
        instance_name: String,
        port: u32,
        flavour: String,
        game_type: String,
    },
    InstanceDelete {
        instance_uuid: InstanceUuid,
    },
}

// the backend will keep exactly 1 copy of ProgressionStart, and 1 copy of ProgressionUpdate OR ProgressionEnd
#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
#[serde(tag = "type")]
pub enum ProgressionEventInner {
    ProgressionStart {
        progression_name: String,
        producer_id: Option<InstanceUuid>,
        total: Option<f64>,
        inner: Option<ProgressionStartValue>,
    },
    ProgressionUpdate {
        progress_message: String,
        progress: f64,
    },
    ProgressionEnd {
        success: bool,
        message: Option<String>,
        inner: Option<ProgressionEndValue>,
    },
}
#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub enum FSOperation {
    Read,
    Write,
    Move { source: PathBuf },
    Create,
    Delete,
    Upload,
    Download,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[serde(tag = "type", content = "path")]
#[ts(export)]
pub enum FSTarget {
    File(PathBuf),
    Directory(PathBuf),
}
#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct FSEvent {
    pub operation: FSOperation,
    pub target: FSTarget,
}

pub fn new_fs_event(operation: FSOperation, target: FSTarget, caused_by: CausedBy) -> Event {
    Event {
        details: "".to_string(),
        snowflake: Snowflake::default(),
        event_inner: EventInner::FSEvent(FSEvent { operation, target }),
        caused_by,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct ProgressionEvent {
    pub event_id: Snowflake,
    pub progression_event_inner: ProgressionEventInner,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
#[serde(tag = "type")]
#[derive(enum_kinds::EnumKind)]
#[enum_kind(EventType, derive(Serialize, Deserialize, TS))]
pub enum EventInner {
    InstanceEvent(InstanceEvent),
    UserEvent(UserEvent),
    MacroEvent(MacroEvent),
    FSEvent(FSEvent),
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
#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
#[serde(tag = "type")]
pub enum CausedBy {
    User { user_id: UserId, user_name: String },
    Instance { instance_uuid: InstanceUuid },
    Macro { macro_pid: MacroPID },
    System,
    Unknown,
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[serde(into = "ClientEvent")]
pub struct Event {
    pub event_inner: EventInner,
    pub details: String,
    pub snowflake: Snowflake,
    pub caused_by: CausedBy,
}

pub trait IntoEvent {
    fn into_event(self, caused_by: CausedBy, details: String) -> Event;
}

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq, Eq)]
#[ts(export)]
#[derive(sqlx::Type)]
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
            snowflake: client_event.snowflake,
            caused_by: client_event.caused_by.clone(),
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
    pub fn get_instance_uuid(&self) -> Option<InstanceUuid> {
        match &self.event_inner {
            EventInner::InstanceEvent(instance_event) => Some(instance_event.instance_uuid.clone()),
            _ => None,
        }
    }

    pub fn try_macro_event(&self) -> Option<&MacroEvent> {
        match &self.event_inner {
            EventInner::MacroEvent(macro_event) => Some(macro_event),
            _ => None,
        }
    }

    pub fn new_instance_output(
        instance_uuid: InstanceUuid,
        instance_name: String,
        output: String,
    ) -> Event {
        Event {
            details: "".to_string(),
            snowflake: Snowflake::default(),
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_uuid,
                instance_name,
                instance_event_inner: InstanceEventInner::InstanceOutput { message: output },
            }),
            caused_by: CausedBy::System,
        }
    }

    pub fn new_player_message(
        instance_uuid: InstanceUuid,
        instance_name: String,
        player: String,
        player_message: String,
    ) -> Event {
        Event {
            details: "".to_string(),
            snowflake: Snowflake::default(),
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_uuid,
                instance_name,
                instance_event_inner: InstanceEventInner::PlayerMessage {
                    player,
                    player_message,
                },
            }),
            caused_by: CausedBy::System,
        }
    }

    pub fn new_system_message(
        instance_uuid: InstanceUuid,
        instance_name: String,
        system_message: String,
    ) -> Event {
        Event {
            details: "".to_string(),
            snowflake: Snowflake::default(),
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_uuid,
                instance_name,
                instance_event_inner: InstanceEventInner::SystemMessage {
                    message: system_message,
                },
            }),
            caused_by: CausedBy::System,
        }
    }

    pub fn new_instance_state_transition(
        instance_uuid: InstanceUuid,
        instance_name: String,
        state: State,
    ) -> Event {
        Event {
            details: "".to_string(),
            snowflake: Snowflake::default(),
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_uuid,
                instance_name,
                instance_event_inner: InstanceEventInner::StateTransition { to: state },
            }),
            caused_by: CausedBy::System,
        }
    }
}
