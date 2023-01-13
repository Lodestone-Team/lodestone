use axum::{extract::Path, routing::put, Json, Router};
use axum_auth::AuthBearer;

use crate::{traits::ErrorInner, AppState, Error};

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
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.is_owner {
        return Err(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Only owner is allowed to open ports".to_string(),
        });
    }

    Ok(Json(state.port_manager.lock().await.open_port(port).await?))
}

pub fn get_gateway_routes(state: AppState) -> Router {
    Router::new()
        .route("/gateway/open_port/:port", put(open_port))
        .with_state(state)
}
