use super::util::AuthedUser;
use axum::{
    routing::{get, put},
    Json, Router,
};
use color_eyre::eyre::eyre;
use tracing::info;

use crate::{
    error::ErrorKind,
    events::{Event, EventInner},
    types::Snowflake,
    AppState, Error, GlobalSettingsData,
};

pub async fn get_core_settings(
    axum::extract::State(state): axum::extract::State<AppState>,
    _requester: AuthedUser,
) -> Result<Json<GlobalSettingsData>, Error> {
    Ok(Json(state.global_settings.lock().await.client_view()))
}

pub async fn change_core_name(
    axum::extract::State(state): axum::extract::State<AppState>,
    requester: AuthedUser,
    Json(new_name): Json<String>,
) -> Result<(), Error> {
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
    requester: AuthedUser,
    Json(safe_mode): Json<bool>,
) -> Result<(), Error> {
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

/// Turns code execution on or off. Turning it off stops every running macro, including the
/// ones that run atom instances, and tells connected clients to fetch the settings again.
/// Turning it on starts nothing.
pub async fn change_code_execution(
    axum::extract::State(state): axum::extract::State<AppState>,
    requester: AuthedUser,
    Json(enabled): Json<bool>,
) -> Result<(), Error> {
    requester.owner_only("turn code execution on or off")?;
    // Held while the running macros are stopped, so turning it back on waits until they are.
    let mut global_settings = state.global_settings.lock().await;
    global_settings.set_code_execution_enabled(enabled).await?;
    if !enabled {
        info!("Code execution turned off, stopping all macros");
        // Every macro spawned before the switch went off is in the process table, see
        // `MacroExecutor::spawn`. Killing them sends their Stopped events.
        state.macro_executor.shutdown_all();
    } else {
        info!("Code execution turned on");
    }
    drop(global_settings);
    state.event_broadcaster.send(Event {
        event_inner: EventInner::GlobalSettingsChanged,
        details: format!(
            "Code execution turned {}",
            if enabled { "on" } else { "off" }
        ),
        snowflake: Snowflake::default(),
        caused_by: requester.caused_by(),
    });
    Ok(())
}

pub async fn change_domain(
    axum::extract::State(state): axum::extract::State<AppState>,
    requester: AuthedUser,
    Json(new_domain): Json<String>,
) -> Result<(), Error> {
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
    requester: AuthedUser,
    Json(playit_enabled): Json<bool>,
) -> Result<(), Error> {
    if !requester.is_owner {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Not authorized to change Playit Enabled status."),
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
        .route(
            "/global_settings/code_execution",
            put(change_code_execution),
        )
        .route("/global_settings/domain", put(change_domain))
        .route(
            "/global_settings/playit_enabled",
            put(change_core_playit_enabled),
        )
        .with_state(state)
}
