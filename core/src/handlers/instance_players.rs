use std::collections::HashSet;

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};

use crate::{
    auth::user::UserAction,
    error::Error,
    traits::t_player::{Player, TPlayerManagement},
    types::InstanceUuid,
    AppState,
};

use super::util::{get_instance, require_owner_for_atom, AuthedUser};

pub async fn get_player_count(
    State(state): State<AppState>,
    requester: AuthedUser,
    Path(uuid): Path<InstanceUuid>,
) -> Result<Json<u32>, Error> {
    requester
        .require(&state, UserAction::ViewInstance(uuid.clone()))
        .await?;
    get_instance(&state, &uuid)?
        .get_player_count()
        .await
        .map(Json)
}

pub async fn get_max_player_count(
    State(state): State<AppState>,
    requester: AuthedUser,
    Path(uuid): Path<InstanceUuid>,
) -> Result<Json<u32>, Error> {
    requester
        .require(&state, UserAction::ViewInstance(uuid.clone()))
        .await?;
    get_instance(&state, &uuid)?
        .get_max_player_count()
        .await
        .map(Json)
}

pub async fn set_max_player_count(
    State(state): State<AppState>,
    requester: AuthedUser,
    Path(uuid): Path<InstanceUuid>,
    Json(count): Json<u32>,
) -> Result<Json<()>, Error> {
    requester
        .require(&state, UserAction::AccessSetting(uuid.clone()))
        .await?;
    let instance = get_instance(&state, &uuid)?;
    require_owner_for_atom(&requester, &instance)?;
    instance.set_max_player_count(count).await.map(Json)
}

pub async fn get_player_list(
    State(state): State<AppState>,
    requester: AuthedUser,
    Path(uuid): Path<InstanceUuid>,
) -> Result<Json<HashSet<Player>>, Error> {
    requester
        .require(&state, UserAction::ViewInstance(uuid.clone()))
        .await?;
    get_instance(&state, &uuid)?
        .get_player_list()
        .await
        .map(Json)
}

pub fn get_instance_players_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/{uuid}/players/count", get(get_player_count))
        .route(
            "/instance/{uuid}/players/max",
            get(get_max_player_count).put(set_max_player_count),
        )
        .route("/instance/{uuid}/players", get(get_player_list))
        .with_state(state)
}
