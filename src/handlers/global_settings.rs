use axum::{
    extract::Path,
    routing::{get, post},
    Extension, Json, Router,
};
use axum_auth::AuthBearer;

use crate::{traits::ErrorInner, AppState, Error, GlobalSettings};

use super::util::try_auth;

pub async fn get_core_settings(
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<GlobalSettings>, Error> {
    let users = state.users.lock().await;
    try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;

    drop(users);

    Ok(Json(state.global_settings.lock().await.clone()))
}

pub async fn change_core_name(
    Path(new_name): Path<String>,
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<(), Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.is_owner {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to change core name".to_string(),
        });
    }
    state.global_settings.lock().await.set_core_name(new_name).await?;
    Ok(())
}

pub async fn change_core_safe_mode(
    Path(safe_mode): Path<bool>,
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<(), Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.is_owner {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to change core name".to_string(),
        });
    }
    state.global_settings.lock().await.set_safe_mode(safe_mode).await?;
    Ok(())
}

pub fn get_global_settings_routes() -> Router {
    Router::new()
        .route("/global_settings", get(get_core_settings))
        .route("/global_settings/name/:new_name", post(change_core_name))
        .route(
            "/global_settings/safe_mode/:safe_mode",
            post(change_core_safe_mode),
        )
}
