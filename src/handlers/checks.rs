use crate::traits::t_configurable::TConfigurable;
use crate::{port_manager::PortStatus, AppState};
use axum::{extract::Path, routing::get, Json, Router};
/// Check the status of a port
/// Note: this function is not cheap
pub async fn get_port_status(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(port): Path<u32>,
) -> Json<PortStatus> {
    Json(state.port_manager.lock().await.port_status(port))
}

/// Check whether a name is in use
/// Note: this function is not cheap
pub async fn is_name_in_use(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(name): Path<String>,
) -> Json<bool> {
    for entry in state.instances.iter() {
        if entry.value().name().await == name {
            return Json(true);
        }
    }
    Json(false)
}

pub fn get_checks_routes(state: AppState) -> Router {
    Router::new()
        .route("/check/port/:port", get(get_port_status))
        .route("/check/name/:name", get(is_name_in_use))
        .with_state(state)
}
