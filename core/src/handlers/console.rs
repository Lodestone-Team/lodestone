use crate::error::Error;
use crate::events::{EventQuery, EventType, InstanceEventKind};
use crate::{
    output_types::ClientEvent,
    types::{InstanceUuid, TimeRange},
    AppState,
};
use axum::{
    extract::{Path, Query},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use color_eyre::eyre::{eyre, Context};
use sqlx::sqlite::SqlitePool;
use tracing::error;

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConsoleQueryParams {
    start_snowflake_id: i64,
    count: u32,
}

async fn get_console_messages(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<String>,
    Query(query_params): Query<ConsoleQueryParams>,
) -> Result<Json<Vec<ClientEvent>>, Error> {
    let time_range = TimeRange {
        start: query_params.start_snowflake_id,
        end: i64::MAX,
    };

    let pool = &state.sqlite_pool;

    let mut connection = pool
        .acquire()
        .await
        .context("Failed to aquire connection to db")?;

    let rows = sqlx::query!(
        r#"
SELECT
event_value, details, snowflake, level, caused_by_user_id, instance_id
FROM ClientEvents
WHERE snowflake >= ($1)
LIMIT $2"#,
        query_params.start_snowflake_id,
        query_params.count,
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

    let event_query = EventQuery {
        event_levels: None,
        event_types: Some(vec![EventType::InstanceEvent]),
        instance_event_types: Some(vec![InstanceEventKind::InstanceOutput]),
        user_event_types: None,
        event_user_ids: None,
        event_instance_ids: Some(vec![InstanceUuid::from(uuid)]),
        bearer_token: None,
        time_range: None,
    };

    let filtered = parsed_client_events
        .into_iter()
        .filter(|client_event| event_query.filter(client_event))
        .collect();

    return Ok(Json(filtered));
}

pub fn get_console_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/:uuid/console", get(get_console_messages))
        .with_state(state)
}
