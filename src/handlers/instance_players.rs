use axum::{extract::Path, routing::get, Extension, Json, Router};
use serde_json::Value;

use crate::traits::{Supported, Unsupported};
use crate::{
    traits::{t_player::TPlayerManagement, Error, ErrorInner},
    AppState,
};

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

pub async fn set_max_player_count(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
    Json(count): Json<u32>,
) -> Result<Json<()>, Error> {
    match state
        .instances
        .lock()
        .await
        .get_mut(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .set_max_player_count(count)
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
        .get_mut(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
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
        .route(
            "/instance/:uuid/players/max",
            get(get_max_player_count).put(set_max_player_count),
        )
        .route("/instance/:uuid/players", get(get_player_list))
}
