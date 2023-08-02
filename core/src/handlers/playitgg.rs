use axum::{routing::{post, get}, Router};
use crate::playitgg::{stop_cli, generate_signup_link, start_cli, verify_key};

use crate::AppState;

pub fn get_playitgg_routes(state: AppState) -> Router {
    Router::new()
        .route("/playitgg/generate_signup_link", get(generate_signup_link))
        .route("/playitgg/start_cli", post(start_cli))
        .route("/playitgg/stop_cli", post(stop_cli))
        .route("/playitgg/verify_key", post(verify_key))
        .with_state(state)
}
