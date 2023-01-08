use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    events::{
        CausedBy, EventInner, EventLevel, 
    },
    output_types::ClientEvent, auth::user_id::UserId, types::{InstanceUuid, Snowflake},
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
            event_value: serde_json::to_value(client_event).unwrap(),
            details: client_event.details.clone(),
            snowflake: client_event.snowflake,
            level: client_event.level.clone(),
            caused_by_user_id,
            instance_id,
        }
    }
}

impl From<&ClientEventRow> for ClientEvent {
    fn from(client_event_row: &ClientEventRow) -> Self {
        serde_json::from_value(client_event_row.event_value.to_owned()).unwrap()
    }
}
