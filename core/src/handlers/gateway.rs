use axum::{
    extract::{Path, State},
    routing::put,
    Json, Router,
};

use crate::{error::Error, AppState};

use super::util::AuthedUser;

pub async fn open_port(
    State(state): State<AppState>,
    requester: AuthedUser,
    Path(port): Path<u16>,
) -> Result<Json<()>, Error> {
    requester.require_owner()?;
    Ok(Json(state.port_manager.lock().await.open_port(port).await?))
}

pub fn get_gateway_routes(state: AppState) -> Router {
    Router::new()
        .route("/gateway/open_port/{port}", put(open_port))
        .with_state(state)
}
