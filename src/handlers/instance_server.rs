use axum::{
    extract::Path,
    routing::{get, post, put},
    Router,
};

use axum::Json;
use axum_auth::AuthBearer;

use serde_json::{json, Value};

use crate::{auth::user::UserAction, events::CausedBy, types::InstanceUuid};

use crate::{
    traits::{t_configurable::TConfigurable, t_server::TServer, Error, ErrorInner},
    AppState,
};

pub async fn start_instance(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::StartInstance(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to start instance".to_string(),
        });
    }
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    let mut instance_list = state.instances.lock().await;
    let instance = instance_list.get_mut(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    if !port_scanner::local_port_available(instance.port().await as u16) {
        return Err(Error {
            inner: ErrorInner::PortInUse,
            detail: format!("Port {} is already in use", instance.port().await),
        });
    }
    instance.start(caused_by).await?;
    Ok(Json(json!("ok")))
}

pub async fn stop_instance(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::StopInstance(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to stop instance".to_string(),
        });
    }
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    state
        .instances
        .lock()
        .await
        .get_mut(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .stop(caused_by)
        .await?;
    Ok(Json(json!("ok")))
}

pub async fn kill_instance(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::StopInstance(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to kill instance".to_string(),
        });
    }
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    state
        .instances
        .lock()
        .await
        .get_mut(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
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
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::AccessConsole(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to send command".to_string(),
        });
    }
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    state
        .instances
        .lock()
        .await
        .get_mut(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
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
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::ViewInstance(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to get instance state".to_string(),
        });
    }
    Ok(Json(json!(
        state
            .instances
            .lock()
            .await
            .get(&uuid)
            .ok_or(Error {
                inner: ErrorInner::InstanceNotFound,
                detail: "".to_string(),
            })?
            .state()
            .await
    )))
}

pub fn get_instance_server_routes(state : AppState) -> Router {
    Router::new()
        .route("/instance/:uuid/start", put(start_instance))
        .route("/instance/:uuid/stop", put(stop_instance))
        .route("/instance/:uuid/kill", put(kill_instance))
        .route("/instance/:uuid/console", post(send_command))
        .route("/instance/:uuid/state", get(get_instance_state))
        .with_state(state)
}
