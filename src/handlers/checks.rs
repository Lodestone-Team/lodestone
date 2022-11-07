use crate::{port_allocator::PortStatus, AppState};
use axum::{extract::Path, routing::get, Extension, Json, Router};

/// Check the status of a port
/// Note: this function is not cheap
pub async fn get_port_status(
    Extension(state): Extension<AppState>,
    Path(port): Path<u32>,
) -> Json<PortStatus> {
    Json(state.port_allocator.lock().await.port_status(port))
}

/// Check whether a name is in use
/// Note: this function is not cheap
pub async fn is_name_in_use(
    Extension(state): Extension<AppState>,
    Path(name): Path<String>,
) -> Json<bool> {
    for (_, instance) in state.instances.lock().await.iter() {
        if instance.lock().await.name().await == name {
            return Json(true);
        }
    }
    Json(false)
}

pub fn get_checks_routes() -> Router {
    Router::new()
        .route("/check/port/:port", get(get_port_status))
        .route("/check/name/:name", get(is_name_in_use))
}
