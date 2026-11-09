//! The core's events (`ClientEvent`), from `GET /events/all/buffer` and the event
//! stream (`StreamKind::All`), typed in full so every page can match on them.
//!
//! Variants the core may add later read as `Unknown`, so an older Host keeps working.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::models::{InstanceInfo, InstanceState, Player, UserPermission};

/// The level of an event (`EventLevel`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventLevel {
    #[default]
    Info,
    Warning,
    Error,
    #[serde(other)]
    Unknown,
}

/// Who caused an event (`CausedBy`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CausedBy {
    User {
        user_id: String,
        user_name: String,
    },
    Instance {
        instance_uuid: String,
    },
    Macro {
        macro_pid: Value,
    },
    System,
    #[default]
    #[serde(other)]
    Unknown,
}

/// One event (`ClientEvent`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClientEvent {
    pub event_inner: EventInner,
    #[serde(default)]
    pub details: String,
    /// The event's id; its time is `(id >> 22) + 1667530800000` ms ([`Self::millis`]).
    #[serde(deserialize_with = "snowflake")]
    pub snowflake: i64,
    #[serde(default)]
    pub level: EventLevel,
    #[serde(default)]
    pub caused_by: CausedBy,
}

/// The core's epoch for snowflakes, in Unix milliseconds.
const SNOWFLAKE_EPOCH_MS: i64 = 1_667_530_800_000;

impl ClientEvent {
    /// When the event happened, in Unix milliseconds.
    pub fn millis(&self) -> i64 {
        (self.snowflake >> 22) + SNOWFLAKE_EPOCH_MS
    }

    /// The instance the event is about, if it is about one.
    pub fn instance(&self) -> Option<&str> {
        match &self.event_inner {
            EventInner::InstanceEvent(event) => Some(&event.instance_uuid),
            EventInner::MacroEvent(event) => event.instance_uuid.as_deref(),
            _ => None,
        }
    }
}

/// A snowflake: a string of digits, or a number from an older core.
fn snowflake<'de, D: serde::Deserializer<'de>>(de: D) -> Result<i64, D::Error> {
    match Value::deserialize(de)? {
        Value::String(text) => text.parse().map_err(serde::de::Error::custom),
        Value::Number(number) => number
            .as_i64()
            .ok_or_else(|| serde::de::Error::custom("snowflake out of range")),
        _ => Err(serde::de::Error::custom(
            "a snowflake is a string or number",
        )),
    }
}

/// What an event is about (`EventInner`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventInner {
    InstanceEvent(InstanceEvent),
    UserEvent(UserEvent),
    MacroEvent(MacroEvent),
    #[serde(rename = "FSEvent")]
    FsEvent(Value),
    ProgressionEvent(ProgressionEvent),
    GlobalSettingsChanged,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstanceEvent {
    pub instance_uuid: String,
    #[serde(default)]
    pub instance_name: String,
    pub instance_event_inner: InstanceEventInner,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InstanceEventInner {
    StateTransition {
        to: InstanceState,
    },
    InstanceWarning {
        message: String,
    },
    InstanceError {
        message: String,
    },
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
        #[serde(default)]
        player_list: Vec<Player>,
        #[serde(default)]
        players_joined: Vec<Player>,
        #[serde(default)]
        players_left: Vec<Player>,
    },
    PlayerMessage {
        player: String,
        player_message: String,
    },
    #[serde(other)]
    Unknown,
}

impl InstanceEventInner {
    /// Console content: output, chat, system messages and the commands sent. The
    /// console stream carries these; the notifications leave them out.
    pub fn is_console(&self) -> bool {
        matches!(
            self,
            InstanceEventInner::InstanceOutput { .. }
                | InstanceEventInner::InstanceInput { .. }
                | InstanceEventInner::SystemMessage { .. }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserEvent {
    pub user_id: String,
    pub user_event_inner: UserEventInner,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UserEventInner {
    UserCreated,
    UserDeleted,
    UserLoggedIn,
    UserLoggedOut,
    UsernameChanged {
        new_username: String,
    },
    PermissionChanged {
        new_permissions: UserPermission,
    },
    ApiKeyCreated {
        key_id: String,
        name: String,
    },
    ApiKeyRevoked {
        key_id: String,
        name: String,
    },
    #[serde(other)]
    Unknown,
}

/// Macros run on Deno, which Lodestone Host's own core doesn't have; another core's
/// macro events still arrive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MacroEvent {
    #[serde(default)]
    pub instance_uuid: Option<String>,
    pub macro_pid: Value,
    pub macro_event_inner: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgressionEvent {
    #[serde(deserialize_with = "snowflake")]
    pub event_id: i64,
    pub progression_event_inner: ProgressionEventInner,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProgressionEventInner {
    ProgressionStart {
        progression_name: String,
        #[serde(default)]
        total: Option<f64>,
        #[serde(default)]
        inner: Option<ProgressionStartValue>,
    },
    ProgressionUpdate {
        progress_message: String,
        progress: f64,
    },
    ProgressionEnd {
        success: bool,
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        inner: Option<ProgressionEndValue>,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProgressionStartValue {
    InstanceCreation {
        instance_uuid: String,
    },
    InstanceDelete {
        instance_uuid: String,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProgressionEndValue {
    InstanceCreation(Box<InstanceInfo>),
    InstanceDelete {
        instance_uuid: String,
    },
    #[serde(rename = "FSOperationCompleted")]
    FsOperationCompleted {
        instance_uuid: String,
        success: bool,
        message: String,
    },
    #[serde(other)]
    Unknown,
}

/// Reads a frame of the event stream. `None` for a frame that isn't an event.
pub fn parse(text: &str) -> Option<ClientEvent> {
    serde_json::from_str(text).ok()
}

impl super::AdminClient {
    /// `GET /events/all/buffer`: the events the core kept, oldest first.
    pub async fn event_history(&self) -> Result<Vec<ClientEvent>, super::CoreError> {
        let filter = serde_json::json!({
            "event_levels": null,
            "event_types": null,
            "instance_event_types": null,
            "user_event_types": null,
            "event_user_ids": null,
            "event_instance_ids": null,
            "bearer_token": null,
            "time_range": null,
        });
        let encoded: String =
            url::form_urlencoded::byte_serialize(filter.to_string().as_bytes()).collect();
        let raw: Vec<Value> = self
            .get(&format!("events/all/buffer?filter={encoded}"))
            .await?;
        // an event this build can't read is skipped, not the whole history
        Ok(raw
            .into_iter()
            .filter_map(|event| serde_json::from_value(event).ok())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_state_change() {
        let event = parse(
            r#"{"event_inner":{"type":"InstanceEvent","instance_uuid":"i1","instance_name":"SMP",
                "instance_event_inner":{"type":"StateTransition","to":"Running"}},
                "details":"","snowflake":"7380000000000000000","level":"Info",
                "caused_by":{"type":"System"}}"#,
        )
        .unwrap();
        assert_eq!(event.instance(), Some("i1"));
        let EventInner::InstanceEvent(inner) = &event.event_inner else {
            panic!("an instance event");
        };
        assert_eq!(
            inner.instance_event_inner,
            InstanceEventInner::StateTransition {
                to: InstanceState::Running
            }
        );
        assert!(event.millis() > SNOWFLAKE_EPOCH_MS);
    }

    #[test]
    fn reads_progressions_and_new_kinds() {
        let event = parse(
            r#"{"event_inner":{"type":"ProgressionEvent","event_id":12,
                "progression_event_inner":{"type":"ProgressionStart","progression_name":"Setting up",
                "total":100,"inner":{"type":"InstanceCreation","instance_uuid":"n1"}}},
                "details":"","snowflake":13,"level":"Info","caused_by":{"type":"Unknown"}}"#,
        )
        .unwrap();
        assert!(matches!(event.event_inner, EventInner::ProgressionEvent(_)));
        let event = parse(
            r#"{"event_inner":{"type":"SomethingNew","x":1},"details":"",
                "snowflake":"1","level":"Info","caused_by":{"type":"System"}}"#,
        )
        .unwrap();
        assert_eq!(event.event_inner, EventInner::Unknown);
        let event = parse(
            r#"{"event_inner":{"type":"GlobalSettingsChanged"},"details":"",
                "snowflake":"1","level":"Info","caused_by":{"type":"System"}}"#,
        )
        .unwrap();
        assert_eq!(event.event_inner, EventInner::GlobalSettingsChanged);
        // a playit.gg runner event from a core before Lodestone Connect
        let event = parse(
            r#"{"event_inner":{"type":"PlayitggRunnerEvent",
                "playitgg_runner_event_inner":{"type":"RunnerStarted"}},"details":"",
                "snowflake":"1","level":"Info","caused_by":{"type":"System"}}"#,
        )
        .unwrap();
        assert_eq!(event.event_inner, EventInner::Unknown);
    }
}
