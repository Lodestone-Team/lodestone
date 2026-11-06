use crate::traits::t_configurable::TConfigurable;
use crate::{port_manager::PortStatus, AppState};
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};

use super::util::{all_instances, AuthedUser};

/// Check the status of a port
/// Note: this function is not cheap
pub async fn get_port_status(
    State(state): State<AppState>,
    _requester: AuthedUser,
    Path(port): Path<u16>,
) -> Json<PortStatus> {
    Json(state.port_manager.lock().await.port_status(u32::from(port)))
}

/// Check whether a name is in use
/// Note: this function is not cheap
pub async fn is_name_in_use(
    State(state): State<AppState>,
    _requester: AuthedUser,
    Path(name): Path<String>,
) -> Json<bool> {
    for instance in all_instances(&state) {
        if instance.name().await == name {
            return Json(true);
        }
    }
    Json(false)
}

pub fn get_checks_routes(state: AppState) -> Router {
    Router::new()
        .route("/check/port/{port}", get(get_port_status))
        .route("/check/name/{name}", get(is_name_in_use))
        .with_state(state)
}
