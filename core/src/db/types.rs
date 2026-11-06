use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    auth::user_id::UserId,
    events::{CausedBy, EventInner, EventLevel},
    output_types::ClientEvent,
    types::{InstanceUuid, Snowflake},
};

#[derive(Serialize, Deserialize)]
pub struct ClientEventRow {
    pub event_value: Value,
    pub details: String,
    pub snowflake: Snowflake,
    pub level: EventLevel,
    pub caused_by_user_id: Option<UserId>,
    pub instance_id: Option<InstanceUuid>,
}

impl From<&ClientEvent> for ClientEventRow {
    fn from(client_event: &ClientEvent) -> Self {
        let caused_by_user_id = if let CausedBy::User { user_id, .. } = &client_event.caused_by {
            Some(user_id.to_owned())
        } else {
            None
        };

        let instance_id = if let EventInner::InstanceEvent(i) = &client_event.event_inner {
            Some(i.instance_uuid.to_owned())
        } else {
            None
        };

        ClientEventRow {
            // ClientEvent is plain data (string map keys only), so this cannot fail
            event_value: serde_json::to_value(client_event)
                .expect("ClientEvent always serializes to JSON"),
            details: client_event.details.clone(),
            snowflake: client_event.snowflake,
            level: client_event.level.clone(),
            caused_by_user_id,
            instance_id,
        }
    }
}
