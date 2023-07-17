use std::collections::HashSet;

use axum::{extract::Path, routing::get, Json, Router};
use color_eyre::eyre::eyre;

use crate::{
    error::{Error, ErrorKind},
    traits::t_player::{Player, TPlayerManagement},
    types::InstanceUuid,
    AppState,
};

pub async fn get_player_count(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
) -> Result<Json<u32>, Error> {
    state
        .instances
        .get(&uuid)
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })?
        .get_player_count()
        .await
        .map(Json)
}

pub async fn get_max_player_count(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
) -> Result<Json<u32>, Error> {
    state
        .instances
        .get(&uuid)
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })?
        .get_max_player_count()
        .await
        .map(Json)
}

pub async fn set_max_player_count(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    Json(count): Json<u32>,
) -> Result<Json<()>, Error> {
    state
        .instances
        .get_mut(&uuid)
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })?
        .set_max_player_count(count)
        .await
        .map(Json)
}

pub async fn get_player_list(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
) -> Result<Json<HashSet<Player>>, Error> {
    state
        .instances
        .get_mut(&uuid)
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })?
        .get_player_list()
        .await
        .map(Json)
}

pub fn get_instance_players_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/:uuid/players/count", get(get_player_count))
        .route(
            "/instance/:uuid/players/max",
            get(get_max_player_count).put(set_max_player_count),
        )
        .route("/instance/:uuid/players", get(get_player_list))
        .with_state(state)
}
