use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use axum::{extract::Path, Extension, Json, Router};
use log::info;
use rand_core::OsRng;

use crate::{
    auth::{permission::UserPermission, user::User},
    events::CausedBy,
    traits::{Error, ErrorInner},
    types::UserId,
    util::rand_alphanumeric,
    AppState,
};

#[derive(serde::Deserialize)]
pub struct OwnerSetup {
    username: String,
    password: String,
}

pub async fn setup_owner(
    Extension(state): Extension<AppState>,
    Path(key): Path<String>,
    Json(owner_setup): Json<OwnerSetup>,
) -> Result<Json<()>, Error> {
    let mut setup_key_lock = state.first_time_setup_key.lock().await;
    match setup_key_lock.clone() {
        Some(k) if k == key => {
            setup_key_lock.take();
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = Argon2::default();
            let hashed_psw = argon2
                .hash_password(owner_setup.password.as_bytes(), &salt)
                .unwrap()
                .to_string();
            let uid = UserId::default();
            let owner = User {
                username: owner_setup.username,
                is_owner: true,
                permissions: UserPermission::new(),
                uid: uid.clone(),
                hashed_psw,
                is_admin: false,
                secret: rand_alphanumeric(32),
            };
            state
                .users_manager
                .write()
                .await
                .add_user(owner, CausedBy::System)
                .await?;
            info!("Owner password: {}", owner_setup.password);
            Ok(Json(()))
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

pub fn get_setup_route() -> Router {
    Router::new().route("/setup/:key", axum::routing::post(setup_owner))
}
