use axum::{
    extract::{Path, State},
    routing::{get, post, put},
    Router,
};

use axum::Json;

use color_eyre::eyre::eyre;
use serde_json::{json, Value};

use crate::{
    auth::user::UserAction,
    error::{Error, ErrorKind},
    types::InstanceUuid,
};

use crate::{
    traits::{t_configurable::TConfigurable, t_server::TServer},
    AppState,
};

use super::util::{
    get_instance, is_atom, require_code_execution, require_owner_for_atom, AuthedUser,
};

fn is_docker(uuid: &InstanceUuid) -> bool {
    uuid.to_string().starts_with("DOCKER-")
}

pub async fn start_instance(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<()>, Error> {
    requester
        .require(&state, UserAction::StartInstance(uuid.clone()))
        .await?;
    if is_docker(&uuid) {
        state.docker_bridge.start_container(&uuid).await?;
        return Ok(Json(()));
    }
    let instance = get_instance(&state, &uuid)?;
    require_owner_for_atom(&requester, &instance)?;
    if is_atom(&instance) {
        require_code_execution(&state)?;
    }
    let port = instance.port().await;

    // check if port is already in use
    if state.port_manager.lock().await.port_status(port).is_in_use {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Port {} is in use", port),
        });
    }

    instance.start(requester.caused_by(), false).await?;
    Ok(Json(()))
}

pub async fn stop_instance(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<()>, Error> {
    requester
        .require(&state, UserAction::StopInstance(uuid.clone()))
        .await?;
    if is_docker(&uuid) {
        state.docker_bridge.stop_container(&uuid).await?;
        return Ok(Json(()));
    }
    let instance = get_instance(&state, &uuid)?;
    require_owner_for_atom(&requester, &instance)?;
    instance.stop(requester.caused_by(), false).await?;
    Ok(Json(()))
}

pub async fn restart_instance(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<()>, Error> {
    requester
        .require(&state, UserAction::StopInstance(uuid.clone()))
        .await?;
    requester
        .require(&state, UserAction::StartInstance(uuid.clone()))
        .await?;
    if is_docker(&uuid) {
        state.docker_bridge.restart_container(&uuid).await?;
        return Ok(Json(()));
    }
    let instance = get_instance(&state, &uuid)?;
    require_owner_for_atom(&requester, &instance)?;
    if is_atom(&instance) {
        require_code_execution(&state)?;
    }
    instance.restart(requester.caused_by(), false).await?;
    Ok(Json(()))
}

pub async fn kill_instance(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<Value>, Error> {
    requester
        .require(&state, UserAction::StopInstance(uuid.clone()))
        .await?;
    if is_docker(&uuid) {
        state.docker_bridge.kill_container(&uuid).await?;
        return Ok(Json(json!("ok")));
    }
    let instance = get_instance(&state, &uuid)?;
    require_owner_for_atom(&requester, &instance)?;
    instance.kill(requester.caused_by()).await?;
    Ok(Json(json!("ok")))
}

pub async fn send_command(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
    Json(command): Json<String>,
) -> Result<Json<()>, Error> {
    requester
        .require(&state, UserAction::AccessConsole(uuid.clone()))
        .await?;
    let instance = get_instance(&state, &uuid)?;
    require_owner_for_atom(&requester, &instance)?;
    instance
        .send_command(&command, requester.caused_by())
        .await
        .map(|_| Json(()))
}

pub async fn get_instance_state(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<Value>, Error> {
    requester
        .require(&state, UserAction::ViewInstance(uuid.clone()))
        .await?;
    if is_docker(&uuid) {
        let state = state.docker_bridge.get_container_state(&uuid).await?;
        return Ok(Json(json!(state)));
    }
    Ok(Json(json!(get_instance(&state, &uuid)?.state().await)))
}

pub fn get_instance_server_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/{uuid}/start", put(start_instance))
        .route("/instance/{uuid}/stop", put(stop_instance))
        .route("/instance/{uuid}/restart", put(restart_instance))
        .route("/instance/{uuid}/kill", put(kill_instance))
        .route("/instance/{uuid}/console", post(send_command))
        .route("/instance/{uuid}/state", get(get_instance_state))
        .with_state(state)
}
