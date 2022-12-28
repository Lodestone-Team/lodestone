use axum::{extract::Path, routing::get, Extension, Router, Json};

use axum_auth::AuthBearer;


use crate::{
    auth::user::UserAction,
    traits::{t_macro::TMacro, Error, ErrorInner},
    types::InstanceUuid,
    AppState,
};

pub async fn run_macro(
    Path((uuid, macro_name)): Path<(InstanceUuid, String)>,
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
    Json(args): Json<Vec<String>>,
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
    if !requester.can_perform_action(&UserAction::AccessMacro(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access macro for this instance".to_string(),
        });
    }
    let mut instances = state.instances.lock().await;
    let instance = instances.get_mut(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    instance.run_macro(&macro_name, args, None, false).await?;
    Ok(Json(()))
}

pub fn get_instance_macro_routes() -> Router {
    Router::new().route("/instance/:uuid/macro/run/:macro_name", get(run_macro))
}
