use axum::{
    extract::Path,
    routing::{get, post, put},
    Router,
};

use axum::Json;
use axum_auth::AuthBearer;

use color_eyre::eyre::eyre;
use serde_json::{json, Value};

use crate::{
    auth::user::UserAction,
    error::{Error, ErrorKind},
    events::CausedBy,
    types::InstanceUuid,
};

use crate::{
    traits::{t_configurable::TConfigurable, t_server::TServer},
    AppState,
};

pub async fn start_instance(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::StartInstance(uuid.clone()))?;
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    let mut instance = state
        .instances
        .get_mut(&uuid)
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })?
        .value()
        .clone();
    let port = instance.port().await;

    if state.port_manager.lock().await.port_status(port).is_in_use {
        return Err(Error {
            kind: ErrorKind::Internal,
            source: eyre!("Port {} is in use", port),
        });
    }

    instance.start(caused_by, false).await?;
    Ok(Json(()))
}

pub async fn stop_instance(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::StopInstance(uuid.clone()))?;
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    state
        .instances
        .get_mut(&uuid)
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })?
        .stop(caused_by, false)
        .await?;
    Ok(Json(()))
}

pub async fn restart_instance(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester
        .try_action(&UserAction::StopInstance(uuid.clone()))
        .and_then(|_x| requester.try_action(&UserAction::StartInstance(uuid.clone())))?;
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    let mut instance = state.instances.get_mut(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;

    instance.restart(caused_by, false).await?;
    Ok(Json(()))
}

pub async fn kill_instance(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::StopInstance(uuid.clone()))?;
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    state
        .instances
        .get_mut(&uuid)
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })?
        .kill(caused_by)
        .await?;
    Ok(Json(json!("ok")))
}

pub async fn send_command(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
    Json(command): Json<String>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessConsole(uuid.clone()))?;
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    state
        .instances
        .get_mut(&uuid)
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })?
        .send_command(&command, caused_by)
        .await
        .map(|_| Json(()))
}

pub async fn get_instance_state(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    if !requester.can_perform_action(&UserAction::ViewInstance(uuid.clone())) {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("You don't have permission to view this instance"),
        });
    }
    Ok(Json(json!(
        state
            .instances
            .get(&uuid)
            .ok_or_else(|| Error {
                kind: ErrorKind::NotFound,
                source: eyre!("Instance not found"),
            })?
            .state()
            .await
    )))
}

pub fn get_instance_server_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/:uuid/start", put(start_instance))
        .route("/instance/:uuid/stop", put(stop_instance))
        .route("/instance/:uuid/restart", put(restart_instance))
        .route("/instance/:uuid/kill", put(kill_instance))
        .route("/instance/:uuid/console", post(send_command))
        .route("/instance/:uuid/state", get(get_instance_state))
        .with_state(state)
}
