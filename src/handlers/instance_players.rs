use axum::{extract::Path, Extension, Json, Router, routing::get};
use serde_json::Value;

use crate::{
    traits::{Error, ErrorInner},
    AppState,
};
use crate::traits::{Supported, Unsupported};

pub async fn get_player_count(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<u32>, Error> {
    match state
        .instances
        .lock()
        .await
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await
        .get_player_count()
        .await
    {
        Supported(v) => Ok(Json(v)),
        Unsupported => Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "".to_string(),
        }),
    }
}

pub async fn get_max_player_count(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<u32>, Error> {
    match state
        .instances
        .lock()
        .await
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await
        .get_max_player_count()
        .await
    {
        Supported(v) => Ok(Json(v)),
        Unsupported => Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "".to_string(),
        }),
    }
}

pub async fn get_player_list(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<Vec<Value>>, Error> {
    match state
        .instances
        .lock()
        .await
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await
        .get_player_list()
        .await
    {
        Supported(v) => Ok(Json(v)),
        Unsupported => Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "".to_string(),
        }),
    }
}

pub fn get_instance_players_routes() -> Router {
    Router::new()
        .route("/instance/:uuid/players/count", get(get_player_count))
        .route("/instance/:uuid/players/max", get(get_max_player_count))
        .route("/instance/:uuid/players", get(get_player_list))
}
