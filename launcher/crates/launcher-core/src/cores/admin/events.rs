//! The core's events (`ClientEvent`), from `GET /events/all/buffer` and the event
//! stream (`StreamKind::All`), typed in full so every page can match on them.
//!
//! Variants the core may add later read as `Unknown`, so an older Host keeps working.
//! So do the ones it dropped that an older core still sends, such as its macro events
//! (`MacroEvent`, `caused_by: {"type": "Macro"}`).

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::game::download::{CancelToken, Sleep};

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
            _ => None,
        }
    }
}

/// A snowflake: a string of digits, or a number from an older core.
pub(super) fn snowflake<'de, D: serde::Deserializer<'de>>(de: D) -> Result<i64, D::Error> {
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
    /// A newer version of the modpack the server was made from is on Modrinth.
    ModpackUpdateAvailable {
        modpack: String,
        version_id: String,
        version_number: String,
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
    /// A backup of the server started (`super::backups`).
    Backup {
        instance_uuid: String,
    },
    /// Restoring a backup over the server started.
    Restore {
        instance_uuid: String,
    },
    /// A world backup of the server started (`super::world_backups`).
    WorldBackup {
        instance_uuid: String,
    },
    /// Restoring a world backup into the server started.
    WorldRestore {
        instance_uuid: String,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProgressionEndValue {
    InstanceCreation(Box<CreatedInstance>),
    InstanceDelete {
        instance_uuid: String,
    },
    #[serde(rename = "FSOperationCompleted")]
    FsOperationCompleted {
        instance_uuid: String,
        success: bool,
        message: String,
    },
    /// A backup ended: the backup it made (or, when nothing changed, the newest one),
    /// and the core's message for people.
    BackupCompleted {
        instance_uuid: String,
        #[serde(default)]
        backup_id: Option<String>,
        success: bool,
        #[serde(default)]
        message: String,
    },
    /// A restore ended: the backup restored.
    RestoreCompleted {
        instance_uuid: String,
        #[serde(default)]
        backup_id: Option<String>,
        success: bool,
        #[serde(default)]
        message: String,
    },
    /// A world backup ended: the world backup it made (none when it failed or was
    /// cancelled).
    WorldBackupCompleted {
        instance_uuid: String,
        #[serde(default)]
        world_backup_id: Option<String>,
        success: bool,
        #[serde(default)]
        message: String,
    },
    /// A world backup's restore ended: the one restored, and the safety world backup
    /// taken first (none when the server had no world).
    WorldRestoreCompleted {
        instance_uuid: String,
        #[serde(default)]
        world_backup_id: Option<String>,
        #[serde(default)]
        safety_id: Option<String>,
        success: bool,
        #[serde(default)]
        message: String,
    },
    #[serde(other)]
    Unknown,
}

/// A new server at the end of its creation (`CreatedInstance`): its info, and for one
/// made from a `.lodeimg` what it didn't get. On the wire the info's fields and `outcome`
/// sit side by side, which an older core leaves out.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CreatedInstance {
    #[serde(flatten)]
    pub info: InstanceInfo,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<super::images::CreationOutcome>,
}

/// Reads a frame of the event stream. `None` for a frame that isn't an event.
pub fn parse(text: &str) -> Option<ClientEvent> {
    read(text).ok()
}

/// [`parse`], saying why a frame isn't an event this client can read. The serde error
/// is logged (a few a minute), never the frame: an event can carry a server's console
/// line or a player's name.
pub fn read(text: &str) -> Result<ClientEvent, serde_json::Error> {
    serde_json::from_str(text).inspect_err(|err| {
        UNREADABLE.log(
            "frame",
            log::Level::Warn,
            module_path!(),
            format_args!("couldn't read an event from the core ({} bytes): {err}", text.len()),
        );
    })
}

/// Unreadable events, a few lines a minute.
static UNREADABLE: crate::logging::Throttle = crate::logging::Throttle::new();

impl super::AdminClient {
    /// `GET /events/all/buffer`: the events the core kept, oldest first.
    pub async fn event_history(&self) -> Result<Vec<ClientEvent>, super::CoreError> {
        self.buffered_events(None).await
    }

    /// `GET /events/all/buffer` with only the event types `types` (`"ProgressionEvent"`),
    /// or all when `None`.
    async fn buffered_events(
        &self,
        types: Option<&[&str]>,
    ) -> Result<Vec<ClientEvent>, super::CoreError> {
        let filter = serde_json::json!({
            "event_levels": null,
            "event_types": types,
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
        let total = raw.len();
        let mut first_error = None;
        let events: Vec<ClientEvent> = raw
            .into_iter()
            .filter_map(|event| {
                serde_json::from_value(event)
                    .inspect_err(|err| {
                        first_error.get_or_insert_with(|| err.to_string());
                    })
                    .ok()
            })
            .collect();
        if let Some(err) = first_error {
            UNREADABLE.log(
                "history",
                log::Level::Warn,
                module_path!(),
                format_args!(
                    "skipped {} of the core's {total} past events this build can't read; the first: {err}",
                    total - events.len()
                ),
            );
        }
        Ok(events)
    }
}

/// A progression [`AdminClient::wait_progression`](super::AdminClient::wait_progression)
/// waits for.
pub struct Awaited<'a> {
    /// Its event id, when the request that started it gave one (a backup's `task_id`).
    pub id: Option<i64>,
    /// Recognises its start, for a progression whose id isn't known (a creation, by the
    /// new server's uuid).
    pub start: &'a (dyn Fn(&ProgressionStartValue) -> bool + Sync),
    /// Recognises its end by its value, for when its start fell out of the core's buffer
    /// before it was seen.
    pub end: &'a (dyn Fn(&ProgressionEndValue) -> bool + Sync),
}

/// How a progression ended.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgressionEnded {
    pub success: bool,
    /// The core's message, in English.
    pub message: Option<String>,
    pub value: Option<ProgressionEndValue>,
}

/// What waiting for a progression came to.
#[derive(Clone, Debug, PartialEq)]
pub enum Waited {
    Ended(ProgressionEnded),
    /// Neither its start nor its end showed in [`UNSEEN_POLLS`] looks: the core's buffer
    /// (its last 512 events) may have lost them on a busy core. The caller checks another
    /// way (the new server listed) and may wait again.
    Unseen,
}

/// Why waiting for a progression stopped before it ended.
#[derive(Debug, thiserror::Error)]
pub enum WaitError {
    #[error("cancelled")]
    Cancelled,
    #[error("still running after {0:?}")]
    TimedOut(Duration),
    #[error(transparent)]
    Core(#[from] super::CoreError),
}

/// How often the core's buffer is read while waiting.
pub const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// How many looks without a sign of the progression make [`Waited::Unseen`].
pub const UNSEEN_POLLS: u32 = 5;
/// Failed reads of the buffer in a row that stop the wait (a core restarting, say).
const FAILED_POLLS: u32 = 3;

/// Where waiting for one progression has got to.
#[derive(Default)]
struct Following {
    id: Option<i64>,
    total: Option<f64>,
    done: f64,
    /// The updates already counted, by snowflake: the buffer gives them again each look.
    counted: std::collections::HashSet<i64>,
}

impl Following {
    /// Reads one look at the buffer; the end when it is there.
    fn read(&mut self, events: &[ClientEvent], awaited: &Awaited<'_>) -> Option<ProgressionEnded> {
        for event in events {
            let EventInner::ProgressionEvent(progression) = &event.event_inner else {
                continue;
            };
            let ours = self.id == Some(progression.event_id);
            match &progression.progression_event_inner {
                ProgressionEventInner::ProgressionStart { total, inner, .. } => {
                    if self.id.is_none() && inner.as_ref().is_some_and(|v| (awaited.start)(v)) {
                        self.id = Some(progression.event_id);
                        self.total = *total;
                    } else if ours {
                        self.total = *total;
                    }
                }
                ProgressionEventInner::ProgressionUpdate { progress, .. } if ours => {
                    if self.counted.insert(event.snowflake) {
                        self.done += progress;
                    }
                }
                ProgressionEventInner::ProgressionEnd {
                    success,
                    message,
                    inner,
                } => {
                    let by_value = inner.as_ref().is_some_and(|v| (awaited.end)(v));
                    if ours || by_value {
                        return Some(ProgressionEnded {
                            success: *success,
                            message: message.clone(),
                            value: inner.clone(),
                        });
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// How far along it is, from 0 to 1, when its total is known.
    fn fraction(&self) -> Option<f64> {
        self.total
            .filter(|total| *total > 0.0)
            .map(|total| (self.done / total).clamp(0.0, 1.0))
    }
}

impl super::AdminClient {
    /// Waits for a progression to end by reading the core's event buffer about once a
    /// [`POLL_INTERVAL`] (with `sleep`, which tests replace), calling `progress` with how
    /// far along it is. Gives up after `limit` ([`WaitError::TimedOut`]) or on `cancel`,
    /// which only stops the waiting: the job goes on on the core.
    ///
    /// The buffer holds the core's last 512 events, so a busy core can lose a
    /// progression's start before it is seen: the end is then recognised by its value,
    /// and when neither shows for [`UNSEEN_POLLS`] looks the answer is [`Waited::Unseen`].
    pub async fn wait_progression(
        &self,
        awaited: &Awaited<'_>,
        progress: &(dyn Fn(f64) + Sync),
        cancel: &CancelToken,
        sleep: &Sleep,
        limit: Duration,
    ) -> Result<Waited, WaitError> {
        let mut following = Following {
            id: awaited.id,
            ..Following::default()
        };
        let mut waited = Duration::ZERO;
        let mut failures = 0;
        let mut looks = 0;
        loop {
            if cancel.is_cancelled() {
                return Err(WaitError::Cancelled);
            }
            match self.buffered_events(Some(&["ProgressionEvent"])).await {
                Ok(events) => {
                    failures = 0;
                    looks += 1;
                    if let Some(end) = following.read(&events, awaited) {
                        return Ok(Waited::Ended(end));
                    }
                    if let Some(fraction) = following.fraction() {
                        progress(fraction);
                    }
                    if following.id.is_none() && looks >= UNSEEN_POLLS {
                        return Ok(Waited::Unseen);
                    }
                }
                Err(err @ (super::CoreError::SignedOut | super::CoreError::Forbidden(_))) => {
                    return Err(err.into());
                }
                Err(err) => {
                    failures += 1;
                    if failures >= FAILED_POLLS {
                        return Err(err.into());
                    }
                }
            }
            if waited >= limit {
                return Err(WaitError::TimedOut(limit));
            }
            let pause = std::pin::pin!(sleep(POLL_INTERVAL));
            let cancelled = std::pin::pin!(cancel.cancelled());
            if let futures::future::Either::Right(_) =
                futures::future::select(pause, cancelled).await
            {
                return Err(WaitError::Cancelled);
            }
            waited += POLL_INTERVAL;
        }
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

    #[test]
    fn reads_backup_progressions() {
        let event = parse(
            r#"{"event_inner":{"type":"ProgressionEvent","event_id":"7380000000000000001",
                "progression_event_inner":{"type":"ProgressionStart","progression_name":"Backing up SMP",
                "total":100,"inner":{"type":"Backup","instance_uuid":"i1"}}},
                "details":"","snowflake":"7380000000000000002","level":"Info","caused_by":{"type":"System"}}"#,
        )
        .unwrap();
        let EventInner::ProgressionEvent(progression) = &event.event_inner else {
            panic!("a progression");
        };
        assert_eq!(progression.event_id, 7_380_000_000_000_000_001);
        assert!(matches!(
            &progression.progression_event_inner,
            ProgressionEventInner::ProgressionStart {
                inner: Some(ProgressionStartValue::Backup { instance_uuid }),
                ..
            } if instance_uuid == "i1"
        ));
        let event = parse(
            r#"{"event_inner":{"type":"ProgressionEvent","event_id":"5",
                "progression_event_inner":{"type":"ProgressionEnd","success":true,"message":"Backup done",
                "inner":{"type":"RestoreCompleted","instance_uuid":"i1","backup_id":"20261112-101500-ab12",
                "success":true,"message":"Restored"}}},
                "details":"","snowflake":"6","level":"Info","caused_by":{"type":"System"}}"#,
        )
        .unwrap();
        let EventInner::ProgressionEvent(progression) = &event.event_inner else {
            panic!("a progression");
        };
        assert_eq!(
            progression.progression_event_inner,
            ProgressionEventInner::ProgressionEnd {
                success: true,
                message: Some("Backup done".into()),
                inner: Some(ProgressionEndValue::RestoreCompleted {
                    instance_uuid: "i1".into(),
                    backup_id: Some("20261112-101500-ab12".into()),
                    success: true,
                    message: "Restored".into(),
                }),
            }
        );
        // a backup that failed before it had an id
        let end: ProgressionEndValue = serde_json::from_str(
            r#"{"type":"BackupCompleted","instance_uuid":"i1","backup_id":null,"success":false,"message":"Not enough free disk space"}"#,
        )
        .unwrap();
        assert!(matches!(
            end,
            ProgressionEndValue::BackupCompleted {
                backup_id: None,
                success: false,
                ..
            }
        ));
    }

    /// An older core still sends macro events, and events a macro caused: they read as
    /// `Unknown`, without failing the stream or the history.
    #[test]
    fn reads_an_older_cores_macro_events() {
        let event = parse(
            r#"{"event_inner":{"type":"MacroEvent","instance_uuid":"i1","macro_pid":3,
                "macro_event_inner":{"type":"Stopped","exit_status":{"type":"Success"}}},
                "details":"","snowflake":"1","level":"Info","caused_by":{"type":"System"}}"#,
        )
        .unwrap();
        assert_eq!(event.event_inner, EventInner::Unknown);
        assert_eq!(event.instance(), None);
        let event = parse(
            r#"{"event_inner":{"type":"InstanceEvent","instance_uuid":"i1","instance_name":"SMP",
                "instance_event_inner":{"type":"StateTransition","to":"Starting"}},
                "details":"","snowflake":"2","level":"Info",
                "caused_by":{"type":"Macro","macro_pid":3}}"#,
        )
        .unwrap();
        assert_eq!(event.caused_by, CausedBy::Unknown);
        assert_eq!(event.instance(), Some("i1"));
    }
    /// The end of a real creation from a file, recorded from a core
    /// (`core/src/images/share/launcher_fixtures.rs`).
    #[test]
    fn reads_a_creation_end_with_its_outcome() {
        let event = parse(include_str!(
            "../../../tests/fixtures/core_images/creation-end.json"
        ))
        .unwrap();
        let EventInner::ProgressionEvent(progression) = &event.event_inner else {
            panic!("a progression");
        };
        let ProgressionEventInner::ProgressionEnd {
            success: true,
            inner: Some(ProgressionEndValue::InstanceCreation(created)),
            message,
        } = &progression.progression_event_inner
        else {
            panic!("a creation's end: {progression:?}");
        };
        assert_eq!(created.info.name, "From a file");
        let outcome = created.outcome.as_ref().expect("an outcome");
        assert_eq!(outcome.no_source.names, ["mods/cf.jar"]);
        assert_eq!(outcome.client_jars.names, ["zoom.jar"]);
        assert_eq!(outcome.scripts, 1);
        assert!(message.as_deref().unwrap().contains("mods/cf.jar"));
    }

    #[test]
    fn reads_a_creation_end_without_an_outcome() {
        let end: ProgressionEndValue = serde_json::from_value(serde_json::json!({
            "type": "InstanceCreation", "uuid": "n1", "name": "SMP",
            "game_type": {"type": "MinecraftJava", "variant": {"type": "Fabric"}},
            "version": "1.21.1", "port": 25565, "creation_time": 1, "path": "/s",
            "auto_start": false, "restart_on_crash": false, "state": "Stopped"
        }))
        .unwrap();
        let ProgressionEndValue::InstanceCreation(created) = end else {
            panic!("a creation's end");
        };
        assert_eq!(created.info.uuid, "n1");
        assert_eq!(created.outcome, None);
    }

    /// An HTTP client whose answers to `GET /events/all/buffer` are scripted, one per
    /// request (the last again once they run out).
    struct Buffers {
        answers: std::sync::Mutex<std::collections::VecDeque<String>>,
        last: std::sync::Mutex<String>,
        asked: std::sync::atomic::AtomicUsize,
    }

    impl Buffers {
        fn new(answers: Vec<Value>) -> std::sync::Arc<Self> {
            std::sync::Arc::new(Buffers {
                answers: std::sync::Mutex::new(answers.iter().map(Value::to_string).collect()),
                last: std::sync::Mutex::new("[]".into()),
                asked: Default::default(),
            })
        }
    }

    impl http_client::HttpClient for Buffers {
        fn user_agent(&self) -> Option<&http_client::http::HeaderValue> {
            None
        }

        fn proxy(&self) -> Option<&url::Url> {
            None
        }

        fn send(
            &self,
            req: http_client::Request<http_client::AsyncBody>,
        ) -> futures::future::BoxFuture<
            'static,
            anyhow::Result<http_client::Response<http_client::AsyncBody>>,
        > {
            assert!(req.uri().path().ends_with("/events/all/buffer"));
            let query = req.uri().query().unwrap_or_default().to_string();
            assert!(query.contains("ProgressionEvent"), "{query}");
            self.asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let body = match self.answers.lock().unwrap().pop_front() {
                Some(body) => {
                    *self.last.lock().unwrap() = body.clone();
                    body
                }
                None => self.last.lock().unwrap().clone(),
            };
            Box::pin(async move {
                Ok(http_client::Response::builder()
                    .status(200)
                    .header("Content-Type", "application/json")
                    .body(http_client::AsyncBody::from(body))?)
            })
        }
    }

    fn client(http: std::sync::Arc<Buffers>) -> super::super::AdminClient {
        super::super::AdminClient::new(
            crate::cores::CoreClient::new(http, url::Url::parse("http://127.0.0.1:1").unwrap()),
            crate::Secret::new("token"),
        )
    }

    fn progression(id: i64, snowflake: i64, inner: Value) -> Value {
        serde_json::json!({
            "event_inner": {"type": "ProgressionEvent", "event_id": id.to_string(),
                            "progression_event_inner": inner},
            "details": "", "snowflake": snowflake.to_string(), "level": "Info",
            "caused_by": {"type": "System"}
        })
    }

    fn start(id: i64, server: &str) -> Value {
        progression(
            id,
            id,
            serde_json::json!({
                "type": "ProgressionStart", "progression_name": "Setting up", "total": 10.0,
                "inner": {"type": "InstanceCreation", "instance_uuid": server}
            }),
        )
    }

    fn update(id: i64, snowflake: i64, progress: f64) -> Value {
        progression(
            id,
            snowflake,
            serde_json::json!({
                "type": "ProgressionUpdate", "progress_message": "Placing", "progress": progress
            }),
        )
    }

    fn end(id: i64, server: &str) -> Value {
        progression(
            id,
            id + 1000,
            serde_json::json!({
                "type": "ProgressionEnd", "success": true, "message": "Server created successfully",
                "inner": {"type": "InstanceCreation", "uuid": server, "name": "SMP",
                          "game_type": {"type": "MinecraftJava", "variant": {"type": "Fabric"}},
                          "version": "1.21.1", "port": 25565, "creation_time": 1, "path": "/s",
                          "auto_start": false, "restart_on_crash": false, "state": "Stopped",
                          "outcome": {"scripts": 2}}
            }),
        )
    }

    /// A sleep that returns at once and counts the waits.
    fn counted() -> (Sleep, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        let waits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = waits.clone();
        let sleep: Sleep = std::sync::Arc::new(move |_| {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Box::pin(async {})
        });
        (sleep, waits)
    }

    fn creation_of(server: &'static str) -> Awaited<'static> {
        Awaited {
            id: None,
            start: Box::leak(Box::new(
                move |value: &ProgressionStartValue| matches!(value, ProgressionStartValue::InstanceCreation { instance_uuid } if instance_uuid == server),
            )),
            end: Box::leak(Box::new(
                move |value: &ProgressionEndValue| matches!(value, ProgressionEndValue::InstanceCreation(created) if created.info.uuid == server),
            )),
        }
    }

    #[test]
    fn waits_for_a_progression_and_follows_it() {
        let other = start(7, "someone-else");
        let http = Buffers::new(vec![
            serde_json::json!([]),
            serde_json::json!([other, start(9, "n1")]),
            serde_json::json!([start(9, "n1"), update(9, 20, 2.5), update(7, 21, 9.0)]),
            serde_json::json!([start(9, "n1"), update(9, 20, 2.5), update(9, 22, 2.5)]),
            serde_json::json!([update(9, 22, 2.5), end(7, "someone-else"), end(9, "n1")]),
        ]);
        let (sleep, waits) = counted();
        let seen = std::sync::Mutex::new(Vec::new());
        let waited = futures::executor::block_on(client(http.clone()).wait_progression(
            &creation_of("n1"),
            &|fraction| seen.lock().unwrap().push(fraction),
            &CancelToken::new(),
            &sleep,
            Duration::from_secs(60),
        ))
        .unwrap();
        let Waited::Ended(ended) = waited else {
            panic!("it ended");
        };
        assert!(ended.success);
        let Some(ProgressionEndValue::InstanceCreation(created)) = ended.value else {
            panic!("the creation's end");
        };
        assert_eq!(created.info.uuid, "n1");
        assert_eq!(created.outcome.unwrap().scripts, 2);
        // each update counted once, another progression's never
        assert_eq!(*seen.lock().unwrap(), [0.0, 0.25, 0.5]);
        assert_eq!(waits.load(std::sync::atomic::Ordering::SeqCst), 4);
    }

    #[test]
    fn an_end_whose_start_was_lost_is_known_by_its_value() {
        let http = Buffers::new(vec![serde_json::json!([end(9, "n1")])]);
        let (sleep, _) = counted();
        let waited = futures::executor::block_on(client(http).wait_progression(
            &creation_of("n1"),
            &|_| {},
            &CancelToken::new(),
            &sleep,
            Duration::from_secs(60),
        ))
        .unwrap();
        assert!(matches!(
            waited,
            Waited::Ended(ProgressionEnded { success: true, .. })
        ));
    }

    #[test]
    fn a_progression_that_never_shows_is_unseen_and_cancel_stops_the_wait() {
        let http = Buffers::new(vec![serde_json::json!([start(7, "someone-else")])]);
        let (sleep, waits) = counted();
        let waited = futures::executor::block_on(client(http.clone()).wait_progression(
            &creation_of("n1"),
            &|_| {},
            &CancelToken::new(),
            &sleep,
            Duration::from_secs(60),
        ))
        .unwrap();
        assert_eq!(waited, Waited::Unseen);
        assert_eq!(
            http.asked.load(std::sync::atomic::Ordering::SeqCst),
            UNSEEN_POLLS as usize
        );
        assert_eq!(
            waits.load(std::sync::atomic::Ordering::SeqCst),
            UNSEEN_POLLS as usize - 1
        );

        // a known id is waited for until the limit, unless cancelled
        let backup = Awaited {
            id: Some(9),
            start: &|_| false,
            end: &|_| false,
        };
        let timed_out = futures::executor::block_on(client(http.clone()).wait_progression(
            &backup,
            &|_| {},
            &CancelToken::new(),
            &sleep,
            Duration::from_secs(3),
        ));
        assert!(
            matches!(timed_out, Err(WaitError::TimedOut(_))),
            "{timed_out:?}"
        );
        let cancel = CancelToken::new();
        cancel.cancel();
        let cancelled = futures::executor::block_on(client(http).wait_progression(
            &backup,
            &|_| {},
            &cancel,
            &sleep,
            Duration::from_secs(60),
        ));
        assert!(matches!(cancelled, Err(WaitError::Cancelled)));
    }

    /// The world jobs' progressions, as the core sends them
    /// (`core/src/backups/world/service.rs`).
    #[test]
    fn reads_world_backup_progressions() {
        let start: ProgressionStartValue =
            serde_json::from_str(r#"{"type":"WorldRestore","instance_uuid":"i1"}"#).unwrap();
        assert_eq!(
            start,
            ProgressionStartValue::WorldRestore {
                instance_uuid: "i1".into()
            }
        );
        let end: ProgressionEndValue = serde_json::from_str(
            r#"{"type":"WorldRestoreCompleted","instance_uuid":"i1","world_backup_id":"20261112-101500-ab12",
                "safety_id":null,"success":true,"message":"Restored the world of SMP"}"#,
        )
        .unwrap();
        assert_eq!(
            end,
            ProgressionEndValue::WorldRestoreCompleted {
                instance_uuid: "i1".into(),
                world_backup_id: Some("20261112-101500-ab12".into()),
                safety_id: None,
                success: true,
                message: "Restored the world of SMP".into(),
            }
        );
        let end: ProgressionEndValue = serde_json::from_str(
            r#"{"type":"WorldBackupCompleted","instance_uuid":"i1","success":false}"#,
        )
        .unwrap();
        assert!(matches!(
            end,
            ProgressionEndValue::WorldBackupCompleted {
                world_backup_id: None,
                success: false,
                ..
            }
        ));
    }
}
