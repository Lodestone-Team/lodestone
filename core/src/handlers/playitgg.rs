use axum::{routing::{post, get}, Router};
use crate::playitgg::{stop_cli, generate_signup_link, start_cli, verify_key, cli_is_running, get_tunnels};

use crate::AppState;

pub fn get_playitgg_routes(state: AppState) -> Router {
    Router::new()
        .route("/playitgg/generate_signup_link", get(generate_signup_link))
        .route("/playitgg/start_cli", post(start_cli))
        .route("/playitgg/stop_cli", post(stop_cli))
        .route("/playitgg/verify_key", post(verify_key))
        .route("/playitgg/cli_is_running", get(cli_is_running))
        .route("/playitgg/get_tunnels", get(get_tunnels))
        .with_state(state)
}
