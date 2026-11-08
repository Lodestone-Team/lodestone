//! What Lodestone Launcher may do with one server, so it shows only the controls that
//! work: `GET /launcher/instances/{uuid}/access`.
//!
//! The answer is for the request's own credentials: for a launcher key, its scope (with
//! the console opt-ins chosen at sign-in) intersected with its user's permissions now.
//! The live view itself uses the existing routes: the event stream for state and players,
//! the console buffer and stream, and `POST /instance/{uuid}/console`.

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use ts_rs::TS;

use super::util::AuthedUser;
use crate::{auth::user::UserAction, error::Error, types::InstanceUuid, AppState};

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct LauncherAccess {
    pub view: bool,
    pub start: bool,
    pub stop: bool,
    /// Reading the console (output and chat).
    pub console_read: bool,
    /// Sending commands and chat.
    pub console_write: bool,
}

async fn get_launcher_access(
    State(state): State<AppState>,
    requester: AuthedUser,
    Path(uuid): Path<InstanceUuid>,
) -> Result<Json<LauncherAccess>, Error> {
    requester
        .require(&state, UserAction::ViewInstance(uuid.clone()))
        .await?;
    let safe_mode = state.safe_mode();
    let can = |action: UserAction| requester.try_action(&action, safe_mode).is_ok();
    Ok(Json(LauncherAccess {
        view: true,
        start: can(UserAction::StartInstance(uuid.clone())),
        stop: can(UserAction::StopInstance(uuid.clone())),
        console_read: can(UserAction::ReadConsole(uuid.clone())),
        console_write: can(UserAction::AccessConsole(uuid.clone())),
    }))
}

pub fn get_launcher_live_routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/launcher/instances/{uuid}/access",
            get(get_launcher_access),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests;
