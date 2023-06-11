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
    events::CausedBy,
    macro_executor::MacroPID,
    traits::t_macro::{HistoryEntry, MacroEntry, TMacro, TaskEntry},
    types::InstanceUuid,
    AppState,
};

pub async fn get_instance_task_list(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<TaskEntry>>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())))?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;
    let tasks = instance.get_task_list().await?;
    Ok(Json(tasks))
}

pub async fn get_instance_macro_list(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<MacroEntry>>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())))?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;
    let macros = instance.get_macro_list().await?;
    Ok(Json(macros))
}

pub async fn get_instance_history_list(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<HistoryEntry>>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())))?;
    let instance = state.instances.get(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;
    let history = instance.get_history_list().await?;
    Ok(Json(history))
}

pub async fn run_macro(
    Path((uuid, macro_name)): Path<(InstanceUuid, String)>,
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
    Json(args): Json<Vec<String>>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())))?;
    let mut instance = state.instances.get_mut(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;
    instance
        .run_macro(
            &macro_name,
            args,
            CausedBy::User {
                user_id: requester.uid,
                user_name: requester.username,
            },
        )
        .await?;
    Ok(Json(()))
}

pub async fn kill_macro(
    Path((uuid, pid)): Path<(InstanceUuid, MacroPID)>,
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())))?;
    let mut instance = state.instances.get_mut(&uuid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Instance not found"),
    })?;
    instance.kill_macro(pid).await?;
    Ok(Json(()))
}

pub fn get_instance_macro_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/:uuid/macro/run/:macro_name", put(run_macro))
        .route("/instance/:uuid/macro/kill/:pid", put(kill_macro))
        .route("/instance/:uuid/macro/list", get(get_instance_macro_list))
        .route("/instance/:uuid/task/list", get(get_instance_task_list))
        .route(
            "/instance/:uuid/history/list",
            get(get_instance_history_list),
        )
        .with_state(state)
}
