use crate::{
    error::Error, output_types::ClientEvent,
    prelude::LODESTONE_EPOCH_MIL, events::EventQuery,
};

use color_eyre::eyre::Context;
use sqlx::sqlite::SqlitePool;
use tracing::error;

// TODO clean up all unwraps

pub async fn search_events(
    pool: &SqlitePool,
    event_query: EventQuery,
) -> Result<Vec<ClientEvent>, Error> {
    // TODO do not return sqlx::Error
    let mut connection = pool
        .acquire()
        .await
        .context("Failed to aquire connection to db")?;
    let parsed_client_events = if let Some(time_range) = &event_query.time_range {
        let start = (time_range.start - LODESTONE_EPOCH_MIL.with(|p| *p)) << 22;
        let end = (time_range.end + 1 - LODESTONE_EPOCH_MIL.with(|p| *p)) << 22;
        let rows = sqlx::query!(
            r#"
SELECT
event_value, details, snowflake, level, caused_by_user_id, instance_id
FROM ClientEvents
WHERE snowflake >= ($1) AND snowflake <= ($2)"#,
            start,
            end
        ) // TODO bit shift
        .fetch_all(&mut connection)
        .await
        .context("Failed to fetch events")?;
        let mut parsed_client_events: Vec<ClientEvent> = Vec::new();
        for row in rows {
            if let Ok(client_event) = serde_json::from_str(&row.event_value) {
                parsed_client_events.push(client_event);
            } else {
                error!("Failed to parse client event: {}", row.event_value);
            }
        }
        parsed_client_events
    } else {
        let rows = sqlx::query!(
            r#"
SELECT
*
FROM ClientEvents"#
        )
        .fetch_all(&mut connection)
        .await
        .context("Failed to fetch events")?;
        let mut parsed_client_events: Vec<ClientEvent> = Vec::new();
        for row in rows {
            if let Ok(client_event) = serde_json::from_str(&row.event_value) {
                parsed_client_events.push(client_event);
            } else {
                error!("Failed to parse client event: {}", row.event_value);
            }
        }
        parsed_client_events
    };
    let filtered = parsed_client_events
        .into_iter()
        .filter(|client_event| event_query.filter(client_event))
        .collect();
    Ok(filtered)
}
