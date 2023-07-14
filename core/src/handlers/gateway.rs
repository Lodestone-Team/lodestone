use axum::{extract::Path, routing::put, Json, Router};
use axum_auth::AuthBearer;

use color_eyre::eyre::eyre;

use crate::{
    error::{Error, ErrorKind},
    AppState,
};

pub async fn open_port(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
    Path(port): Path<u16>,
) -> Result<Json<()>, Error> {
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or_else(|| Error {
            kind: ErrorKind::Unauthorized,
            source: eyre!("Token error"),
        })?;
    if !requester.is_owner {
        return Err(Error {
            kind: ErrorKind::Unauthorized,
            source: eyre!("Only owners can open ports"),
        });
    }

    Ok(Json(state.port_manager.lock().await.open_port(port).await?))
}

pub fn get_gateway_routes(state: AppState) -> Router {
    Router::new()
        .route("/gateway/open_port/:port", put(open_port))
        .with_state(state)
}
