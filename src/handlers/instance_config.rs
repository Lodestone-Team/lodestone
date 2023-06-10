use axum::{
    extract::Path,
    routing::{get, put},
    Json, Router,
};
use axum_auth::AuthBearer;
use color_eyre::eyre::eyre;

use crate::{
    auth::user::UserAction,
    error::{Error, ErrorKind},
    traits::t_configurable::{
        manifest::{ConfigurableManifest, ConfigurableValue},
        TConfigurable,
    },
    types::InstanceUuid,
    AppState,
};

pub async fn get_instance_configurable_manifest(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<ConfigurableManifest>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()))?;
    let mut instances = state.instances.lock().await;
    let instance = instances.get_mut(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;
    Ok(Json(instance.configurable_manifest().await))
}

pub async fn get_instance_settings(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<ConfigurableManifest>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()))?;
    let mut instances = state.instances.lock().await;
    let instance = instances.get_mut(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;
    Ok(Json(instance.configurable_manifest().await))
}

pub async fn set_instance_setting(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, section_id, setting_id)): Path<(InstanceUuid, String, String)>,
    AuthBearer(token): AuthBearer,
    Json(value): Json<ConfigurableValue>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()))?;
    let mut instances = state.instances.lock().await;
    let instance = instances.get_mut(&uuid).ok_or(Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;

    instance
        .update_configurable(&section_id, &setting_id, value)
        .await?;

    Ok(Json(()))
}

pub async fn set_instance_name(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
    Json(new_name): Json<String>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()))?;
    state
        .instances
        .lock()
        .await
        .get_mut(&uuid)
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })?
        .set_name(new_name)
        .await?;
    Ok(Json(()))
}

pub async fn set_instance_description(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
    Json(new_description): Json<String>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()))?;
    state
        .instances
        .lock()
        .await
        .get_mut(&uuid)
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })?
        .set_description(new_description)
        .await?;
    Ok(Json(()))
}

pub async fn change_version(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path((uuid, new_version)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessSetting(uuid.clone()))?;
    state
        .instances
        .lock()
        .await
        .get_mut(&uuid)
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })?
        .change_version(new_version)
        .await?;
    Ok(Json(()))
}

pub fn get_instance_config_routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/instance/:uuid/configurable_manifest",
            get(get_instance_configurable_manifest),
        )
        .route("/instance/:uuid/version/:new_version", put(change_version))
        .route("/instance/:uuid/settings", get(get_instance_settings))
        .route(
            "/instance/:uuid/settings/:section_id/:setting_id",
            put(set_instance_setting),
        )
        .route("/instance/:uuid/name", put(set_instance_name))
        .route("/instance/:uuid/description", put(set_instance_description))
        .with_state(state)
}
