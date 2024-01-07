use axum::{
    routing::{get, put},
    Json, Router,
};
use axum_auth::AuthBearer;
use color_eyre::eyre::eyre;

use crate::{error::ErrorKind, AppState, Error, GlobalSettingsData};

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
            kind: ErrorKind::Unauthorized,
            source: eyre!("Token error"),
        })?;

    Ok(Json(state.global_settings.lock().await.as_ref().clone()))
}

pub async fn change_core_name(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
    Json(new_name): Json<String>,
) -> Result<(), Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;

    if !requester.is_owner {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Not authorized to change core name"),
        });
    }
    if new_name.len() > 32 {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Name too long"),
        });
    }
    if new_name.is_empty() {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Name cannot be empty"),
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
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;

    if !requester.is_owner {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Not authorized to change core safe mode"),
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
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    if !requester.is_owner {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Not authorized to change core domain"),
        });
    }
    if new_domain.len() > 253 {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Domain too long"),
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

pub async fn change_core_playit_enabled(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
    Json(playit_enabled): Json<bool>,
) -> Result<(), Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;

    if !requester.is_owner {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Not authorized to change core safe mode"),
        });
    }

    state
        .global_settings
        .lock()
        .await
        .set_playit_enabled(playit_enabled)
        .await?;
    Ok(())
}

pub fn get_global_settings_routes(state: AppState) -> Router {
    Router::new()
        .route("/global_settings", get(get_core_settings))
        .route("/global_settings/name", put(change_core_name))
        .route("/global_settings/safe_mode", put(change_core_safe_mode))
        .route("/global_settings/domain", put(change_domain))
        .route("/global_settings/playit_enabled", put(change_core_playit_enabled))
        .with_state(state)
}
