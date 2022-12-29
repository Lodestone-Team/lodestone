use axum::{
    routing::{get, put},
    Json, Router,
};
use axum_auth::AuthBearer;

use crate::{traits::ErrorInner, AppState, Error, GlobalSettingsData};

pub async fn get_core_settings(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<GlobalSettingsData>, Error> {
    state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;

    Ok(Json(state.global_settings.lock().await.as_ref().clone()))
}

pub async fn change_core_name(
    axum::extract::State(state): axum::extract::State<AppState>,
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
    axum::extract::State(state): axum::extract::State<AppState>,
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

pub async fn change_domain(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
    Json(new_domain): Json<String>,
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
            detail: "Not authorized to change core domain".to_string(),
        });
    }
    if new_domain.len() > 32 {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Domain too long".to_string(),
        });
    }
    state
        .global_settings
        .lock()
        .await
        .set_domain(if new_domain.is_empty() {
            None
        } else {
            Some(new_domain)
        })
        .await?;
    Ok(())
}

pub fn get_global_settings_routes(state: AppState) -> Router {
    Router::new()
        .route("/global_settings", get(get_core_settings))
        .route("/global_settings/name", put(change_core_name))
        .route("/global_settings/safe_mode", put(change_core_safe_mode))
        .route("/global_settings/domain", put(change_domain))
        .with_state(state)
}
