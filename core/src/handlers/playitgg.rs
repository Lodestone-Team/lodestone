use std::{
    sync::LazyLock,
    time::{Duration, Instant},
};

use crate::{
    error::Error,
    playitgg::{
        cli_is_running, generate_signup_link, get_tunnels, start_cli, stop_cli, verify_key,
        PlayitSignupData,
    },
};
use axum::{
    extract::{Request, State},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use tokio::sync::Mutex;

use crate::AppState;

use super::util::AuthedUser;

/// playit.gg can expose the host's ports to the internet, so every route is owner only.
async fn require_owner(
    requester: AuthedUser,
    request: Request,
    next: Next,
) -> Result<Response, Error> {
    requester.require_owner()?;
    Ok(next.run(request).await)
}

/// How long a signup link is reused before a new signup flow can start.
const SIGNUP_REUSE_WINDOW: Duration = Duration::from_secs(5 * 60);

/// The signup flow started most recently, with its start time.
static CURRENT_SIGNUP: LazyLock<Mutex<Option<(Instant, PlayitSignupData)>>> =
    LazyLock::new(|| Mutex::new(None));

/// Starts at most one signup flow at a time.
///
/// Each call of `generate_signup_link` starts a task that polls playit.gg for that claim code.
/// Within [`SIGNUP_REUSE_WINDOW`] of the last flow, return the same link (whose task is still
/// polling) instead of starting another one.
async fn signup_link_once(State(state): State<AppState>) -> Result<Json<PlayitSignupData>, Error> {
    let mut current = CURRENT_SIGNUP.lock().await;
    if let Some((started, data)) = current.as_ref() {
        if started.elapsed() < SIGNUP_REUSE_WINDOW {
            return Ok(Json(data.clone()));
        }
    }
    let Json(data) = generate_signup_link(State(state)).await?;
    *current = Some((Instant::now(), data.clone()));
    Ok(Json(data))
}

pub fn get_playitgg_routes(state: AppState) -> Router {
    Router::new()
        .route("/playitgg/generate_signup_link", get(signup_link_once))
        .route("/playitgg/start_cli", post(start_cli))
        .route("/playitgg/stop_cli", post(stop_cli))
        .route("/playitgg/verify_key", post(verify_key))
        .route("/playitgg/cli_is_running", get(cli_is_running))
        .route("/playitgg/get_tunnels", get(get_tunnels))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_owner))
        .with_state(state)
}
