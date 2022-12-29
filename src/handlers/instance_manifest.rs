use axum::{extract::Path, routing::get, Json, Router};

use crate::{
    traits::{t_manifest::Manifest, t_manifest::TManifest, Error, ErrorInner},
    types::InstanceUuid,
    AppState,
};

pub async fn get_instance_manifest(
    Path(uuid): Path<InstanceUuid>,
    axum::extract::State(state): axum::extract::State<AppState>,
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
            .get_manifest()
            .await,
    ))
}

pub fn get_instance_manifest_routes(state: AppState) -> Router {
    Router::new()
        .route("/instance/:uuid/manifest", get(get_instance_manifest))
        .with_state(state)
}
