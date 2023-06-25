use axum::{routing::post, Router};
use crate::playitgg::{start_tunnel, kill_tunnel};

use crate::AppState;

pub fn get_playitgg_routes(state: AppState) -> Router {
    Router::new()
        .route("/playitgg/run_tunnel", post(start_tunnel))
        .route("/playitgg/kill_tunnel", post(kill_tunnel))
        .with_state(state)
}