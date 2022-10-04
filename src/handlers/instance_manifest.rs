use axum::{extract::Path, Extension, Json, Router, routing::get};

use crate::{
    traits::{t_manifest::Manifest, Error, ErrorInner},
    AppState,
};

pub async fn get_instance_manifest(
    Path(uuid): Path<String>,
    Extension(state): Extension<AppState>,
) -> Result<Json<Manifest>, Error> {
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
            .get_manifest()
            .await,
    ))
}

pub fn get_instance_manifest_routes() -> Router {
    Router::new()
        .route("/instance/:uuid/manifest", get(get_instance_manifest))
}
