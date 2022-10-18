use axum::{
    extract::Path,
    routing::{get, post, put},
    Extension, Router,
};

use axum::Json;
use axum_auth::AuthBearer;

use serde_json::{json, Value};

use crate::{
    auth::user::UserAction,
    traits::{Supported, Unsupported},
};

use super::util::try_auth;
use crate::{
    traits::{Error, ErrorInner},
    AppState,
};

pub async fn start_instance(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::StartInstance(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to start instance".to_string(),
        });
    }
    drop(users);
    let instance_list = state.instances.lock().await;
    let mut instance = instance_list
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await;
    if !port_scanner::local_port_available(instance.port().await as u16) {
        return Err(Error {
            inner: ErrorInner::PortInUse,
            detail: format!("Port {} is already in use", instance.port().await),
        });
    }
    instance.start().await?;
    Ok(Json(json!("ok")))
}

pub async fn stop_instance(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::StopInstance(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to stop instance".to_string(),
        });
    }
    drop(users);
    state
        .instances
        .lock()
        .await
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await
        .stop()
        .await?;
    Ok(Json(json!("ok")))
}

pub async fn kill_instance(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::StopInstance(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to kill instance".to_string(),
        });
    }
    drop(users);
    state
        .instances
        .lock()
        .await
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await
        .kill()
        .await?;
    state
        .instances
        .lock()
        .await
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await
        .kill()
        .await?;
    Ok(Json(json!("ok")))
}

pub async fn send_command(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
    Json(command): Json<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::AccessConsole(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to send command".to_string(),
        });
    }
    drop(users);
    match state
        .instances
        .lock()
        .await
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await
        .send_command(&command)
        .await
    {
        Supported(v) => v.map(|_| Json(())),
        Unsupported => Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "".to_string(),
        }),
    }
}

pub async fn get_instance_state(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::ViewInstance(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to get instance state".to_string(),
        });
    }
    drop(users);
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
            .lock()
            .await
            .state()
            .await
    )))
}

pub fn get_instance_server_routes() -> Router {
    Router::new()
        .route("/instance/:uuid/start", put(start_instance))
        .route("/instance/:uuid/stop", put(stop_instance))
        .route("/instance/:uuid/kill", put(kill_instance))
        .route("/instance/:uuid/console", post(send_command))
        .route("/instance/:uuid/state", get(get_instance_state))
}
