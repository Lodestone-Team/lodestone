use axum::{Router, routing::{put, post, get}, Extension, extract::Path};


use axum::Json;
use axum_auth::AuthBearer;

use serde_json::{json, Value};


use crate::traits::{Supported, Unsupported};

use super::util::{is_authorized, try_auth};
use crate::json_store::permission::Permission::{self};
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
        inner: ErrorInner::PermissionDenied,
        detail: "".to_string(),
    })?;
    if !is_authorized(&requester, &uuid, Permission::CanStartInstance) {
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
) -> Result<Json<Value>, Error> {
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
) -> Result<Json<Value>, Error> {
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
) -> Result<Json<Value>, Error> {
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
        Supported(v) => v.map(|_| Json(json!("ok"))),
        Unsupported => Err(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "".to_string(),
        }),
    }
}

pub async fn get_instance_state(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<Value>, Error> {
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
