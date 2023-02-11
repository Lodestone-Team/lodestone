use axum::{extract::Path, routing::get, Json, Router};

use axum_auth::AuthBearer;
use color_eyre::eyre::eyre;

use crate::{
    auth::user::UserAction,
    error::{Error, ErrorKind},
    events::CausedBy,
    traits::t_macro::TMacro,
    types::InstanceUuid,
    AppState,
};

pub async fn run_macro(
    Path((uuid, macro_name)): Path<(InstanceUuid, String)>,
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
    Json(args): Json<Vec<String>>,
) -> Result<Json<()>, Error> {
    let requester = state.users_manager.read().await.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::AccessMacro(Some(uuid.clone())))?;
    let mut instances = state.instances.lock().await;
    let instance = instances.get_mut(&uuid).ok_or_else(|| Error {
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
            false,
        )
        .await?;
    Ok(Json(()))
}

pub fn get_instance_macro_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/:uuid/macro/run/:macro_name", get(run_macro))
        .with_state(state)
}
