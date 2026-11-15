use axum::{extract::Path, Json, Router};
use color_eyre::eyre::eyre;

use crate::{
    auth::{
        permission::UserPermission,
        user::{valid_username, User},
    },
    error::{Error, ErrorKind},
    events::CausedBy,
    AppState,
};

use super::users::LoginReply;

#[derive(serde::Deserialize)]
pub struct OwnerSetup {
    username: String,
    password: String,
}

pub async fn setup_owner(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(key): Path<String>,
    Json(owner_setup): Json<OwnerSetup>,
) -> Result<Json<LoginReply>, Error> {
    // Hold the setup key lock until the owner is saved: a second request waits here and then
    // sees the key gone. The key is only used up once the owner is saved, so a failed write
    // can be retried.
    let mut setup_key_lock = state.first_time_setup_key.lock().await;
    match setup_key_lock.as_deref() {
        Some(k) if k == key => {
            let username = valid_username(&owner_setup.username).map_err(|message| Error {
                kind: ErrorKind::BadRequest,
                source: eyre!(message.to_string()),
            })?;
            let owner = User::new(
                username,
                &owner_setup.password,
                true,
                false,
                UserPermission::default(),
            );
            state
                .users_manager
                .write()
                .await
                .add_user(owner.clone(), CausedBy::System)
                .await?;
            setup_key_lock.take();
            drop(setup_key_lock);
            // an owner with a password: other devices may use the core, as always
            crate::remote_access::owner_created(&state, true).await;
            Ok(Json(LoginReply {
                token: owner.create_jwt()?,
                user: owner.into(),
            }))
        }
        None => Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Setup key already used."),
        }),
        Some(_) => Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("Invalid setup key."),
        }),
    }
}

pub fn get_setup_route(state: AppState) -> Router {
    Router::new()
        .route("/setup/{key}", axum::routing::post(setup_owner))
        .with_state(state)
}
