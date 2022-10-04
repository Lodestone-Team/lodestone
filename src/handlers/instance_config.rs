use axum::{extract::Path, Extension, Json, Router, routing::get};

use crate::{
    traits::{Error, ErrorInner},
    AppState,
};

pub async fn get_instance_port(
    Path(uuid): Path<String>,
    Extension(state): Extension<AppState>,
) -> Result<Json<u32>, Error> {
    Ok(Json(
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
            .port()
            .await,
    ))
}

pub async fn set_instance_port(
    Path(uuid): Path<String>,
    Extension(state): Extension<AppState>,
    Json(port): Json<u32>,
) -> Result<Json<String>, Error> {
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
        .set_port(port)
        .await
        .ok_or(Error {
            inner: ErrorInner::UnsupportedOperation,
            detail: "".to_string(),
        })??;
    Ok(Json("ok".to_string()))
}

pub async fn get_instance_name(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<String>, Error> {
    Ok(Json(
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
            .name()
            .await
            ,
    ))
}

pub async fn set_instance_name(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
    Json(name): Json<String>,
) -> Result<Json<String>, Error> {
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
        .set_name(name)
        .await?;
    Ok(Json("ok".to_string()))
}

pub async fn get_instance_description(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<String>, Error> {
    Ok(Json(
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
            .description()
            .await
            ,
    ))
}

pub async fn set_instance_description(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
    Json(description): Json<String>,
) -> Result<Json<String>, Error> {
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
        .set_description(description)
        .await?;
    Ok(Json("ok".to_string()))
}

pub fn get_instance_config_routes () -> Router {
    Router::new()
        .route("/instance/:uuid/port", get(get_instance_port).put(set_instance_port))
        .route("/instance/:uuid/name", get(get_instance_name).put(set_instance_name))
        .route("/instance/:uuid/description", get(get_instance_description).put(set_instance_description))
}
