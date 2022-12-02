use axum::{extract::Path, routing::get, Extension, Json, Router};

use axum_auth::AuthBearer;
use axum_macros::debug_handler;

use crate::{
    auth::user::UserAction,
    handlers::util::try_auth,
    traits::{t_macro::TMacro, Error, ErrorInner},
    AppState,
};
#[debug_handler]
async fn run_macro(
    Path((uuid, macro_name)): Path<(String, String)>,
    Json(args): Json<Vec<String>>,
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::AccessMacro(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access macro for this instance".to_string(),
        });
    }
    drop(users);
    let mut instances = state.instances.lock().await;
    let instance = instances.get_mut(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    instance
        .run_macro(&macro_name, args, None)
        .await?;
    Ok(Json(()))
}

pub fn get_instance_macro_routes() -> Router {
    Router::new().route("/instance/:uuid/macro/run/:macro_name", get(run_macro))
}
