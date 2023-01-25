use axum::{extract::Path, Json, Router};

use crate::{
    auth::{permission::UserPermission, user::User},
    events::CausedBy,
    traits::{Error, ErrorInner},
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
    let mut setup_key_lock = state.first_time_setup_key.lock().await;
    match setup_key_lock.clone() {
        Some(k) if k == key => {
            setup_key_lock.take();
            let owner = User::new(
                owner_setup.username,
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
            Ok(Json(LoginReply {
                token: owner.create_jwt()?,
                user: owner.into(),
            }))
        }
        None => Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "This Lodestone client has been setup. Please login instead.".to_string(),
        }),
        Some(_) => Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Invalid setup key.".to_string(),
        }),
    }
}

pub fn get_setup_route(state: AppState) -> Router {
    Router::new()
        .route("/setup/:key", axum::routing::post(setup_owner))
        .with_state(state)
}
