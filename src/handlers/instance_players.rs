use std::collections::HashSet;

use axum::{extract::Path, routing::get, Extension, Json, Router};

use crate::{
    traits::{
        t_player::{Player, TPlayerManagement},
        Error, ErrorInner,
    },
    types::InstanceUuid,
    AppState,
};

pub async fn get_player_count(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<InstanceUuid>,
) -> Result<Json<u32>, Error> {
    state
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
        .map(Json)
}

pub async fn get_max_player_count(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<InstanceUuid>,
) -> Result<Json<u32>, Error> {
    state
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
        .map(Json)
}

pub async fn set_max_player_count(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<InstanceUuid>,
    Json(count): Json<u32>,
) -> Result<Json<()>, Error> {
    state
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
        .map(Json)
}

pub async fn get_player_list(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<InstanceUuid>,
) -> Result<Json<HashSet<Player>>, Error> {
    state
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
        .map(Json)
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
