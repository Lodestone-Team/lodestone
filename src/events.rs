use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::util::{rand_alphanumeric, DownloadProgress, SetupProgress};

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
    pub instance_uuid: String,
    pub instance_name: String,
    pub details: String,
    pub timestamp: i64,
    pub idempotency: String,
}

impl Event {
    pub fn new(
        event_inner: EventInner,
        instance_uuid: String,
        instance_name: String,
        details: String,
        idempotency: Option<String>,
    ) -> Self {
        Event {
            event_inner,
            instance_uuid,
            instance_name,
            details,
            timestamp: chrono::Utc::now().timestamp(),
            idempotency: idempotency.unwrap_or_else(|| rand_alphanumeric(10)),
        }
    }
}
