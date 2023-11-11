use crate::error::Error;
use crate::{
    db::read::search_events_limited,
    events::{EventQuery, EventType, InstanceEventKind},
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
    let event_instance_ids = vec![InstanceUuid::from(uuid)];
    let time_range = TimeRange {
        start: query_params.start_snowflake_id,
        end: i64::MAX,
    };

    let event_query = EventQuery {
        event_levels: None,
        event_types: Some(vec![EventType::InstanceEvent]),
        instance_event_types: Some(vec![InstanceEventKind::InstanceOutput]),
        user_event_types: None,
        event_user_ids: None,
        event_instance_ids: Some(event_instance_ids),
        bearer_token: None,
        time_range: Some(time_range),
    };

    let client_events =
        dbg!(search_events_limited(&state.sqlite_pool, event_query, query_params.count).await)?;

    return Ok(Json(client_events));
}

pub fn get_console_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/:uuid/console", get(get_console_messages))
        .with_state(state)
}
