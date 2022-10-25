

use axum::{extract::Path, routing::get, Extension, Json, Router};

use axum_macros::debug_handler;

use crate::{
    traits::{Error, ErrorInner},
    AppState,
};
#[debug_handler]
async fn run_macro(
    Path((uuid, macro_name)): Path<(String, String)>,
    Json(args): Json<Vec<String>>,
    Extension(state): Extension<AppState>,
) -> Result<Json<()>, Error> {
    let instances = state.instances.lock().await;
    let instance = instances.get(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    let mut instance = instance.lock().await;
    instance
        .run_macro(&macro_name, args, None)
        .await
        .unwrap()
        .unwrap();
    Ok(Json(()))
}

pub fn get_instance_macro_routes() -> Router {
    Router::new().route("/instance/:uuid/macro/run/:macro_name", get(run_macro))
}
