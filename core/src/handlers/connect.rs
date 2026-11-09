//! Sharing an instance with friends through Lodestone Connect (see [`crate::connect`]):
//!
//! - `GET /instance/{uuid}/share`: whether it is shared, the link, who is connected;
//! - `POST /instance/{uuid}/share`: starts sharing; answers once the link is known;
//! - `DELETE /instance/{uuid}/share`: stops sharing; the link stops working for good.
//!
//! All three need the instance's settings permission (the owner has it). The link lets
//! anyone who has it join the server, so only those who may manage the server see it.

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};

use super::util::{get_instance, AuthedUser};
use crate::{
    auth::user::UserAction, connect::InstanceShare, error::Error, types::InstanceUuid, AppState,
};

async fn allowed(
    state: &AppState,
    requester: &AuthedUser,
    uuid: &InstanceUuid,
) -> Result<(), Error> {
    requester
        .require(state, UserAction::AccessSetting(uuid.clone()))
        .await?;
    get_instance(state, uuid)?;
    Ok(())
}

async fn get_share(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<InstanceShare>, Error> {
    allowed(&state, &requester, &uuid).await?;
    Ok(Json(state.connect.status(&uuid).await))
}

async fn start_share(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<InstanceShare>, Error> {
    allowed(&state, &requester, &uuid).await?;
    Ok(Json(state.connect.start(&state, &uuid).await?))
}

async fn stop_share(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<InstanceShare>, Error> {
    allowed(&state, &requester, &uuid).await?;
    state.connect.stop(&uuid, true).await;
    Ok(Json(state.connect.status(&uuid).await))
}

pub fn get_connect_routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/instance/{uuid}/share",
            get(get_share).post(start_share).delete(stop_share),
        )
        .with_state(state)
}
