use axum::{extract::{Path, Query}, routing::{put, get}, Json, Router};
use axum_macros::debug_handler;
use serde::{Deserialize, Serialize};
use crate::AppState;
use crate::error::Error;
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConsoleQuery {
    instance_uuid : String,
    start_snowflake_id : i64,
    count : i64,
}

#[derive(Debug, Serialize, Deserialize, TS)]
pub struct ConsoleQueryParams {
    start_snowflake_id : i64,
    count : i64,
}

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ConsoleEvent {
  timestamp: i64,
  snowflake: i64,
  detail: String,
  uuid: String,
  name: String,
  message: String,
}

async fn get_console_messages(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<String>,
    Query(query_params): Query<ConsoleQueryParams>,
) -> Result<Json<Vec<ConsoleEvent>>, Error> {
    let console_query = ConsoleQuery {
        instance_uuid: uuid,
        start_snowflake_id: query_params.start_snowflake_id,
        count: query_params.count,
    };


    return Ok(Json(vec![]))
}


pub fn get_console_routes(state: AppState) -> Router {
    Router::new()
        .route("/console/:uuid", get(get_console_messages))
        .with_state(state)
}
