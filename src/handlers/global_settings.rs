use axum::{
    routing::{get, put},
    Extension, Json, Router,
};
use axum_auth::AuthBearer;

use crate::{traits::ErrorInner, AppState, Error, GlobalSettings};

pub async fn get_core_settings(
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<GlobalSettings>, Error> {
    state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;

    Ok(Json(state.global_settings.lock().await.clone()))
}

pub async fn change_core_name(
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
    Json(new_name): Json<String>,
) -> Result<(), Error> {
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
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to change core name".to_string(),
        });
    }
    if new_name.len() > 32 {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Name too long".to_string(),
        });
    }
    if new_name.is_empty() {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Name too short".to_string(),
        });
    }
    state
        .global_settings
        .lock()
        .await
        .set_core_name(new_name)
        .await?;
    Ok(())
}

pub async fn change_core_safe_mode(
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
    Json(safe_mode): Json<bool>,
) -> Result<(), Error> {
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
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to change core name".to_string(),
        });
    }
    state
        .global_settings
        .lock()
        .await
        .set_safe_mode(safe_mode)
        .await?;
    Ok(())
}

pub fn get_global_settings_routes() -> Router {
    Router::new()
        .route("/global_settings", get(get_core_settings))
        .route("/global_settings/name", put(change_core_name))
        .route("/global_settings/safe_mode", put(change_core_safe_mode))
}
