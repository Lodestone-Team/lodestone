use uuid::Uuid;

use crate::{
    auth::{permission::UserPermission, user::User},
    events::CausedBy,
    traits::{Error, ErrorInner},
    util::{hash_password, rand_alphanumeric},
    AppState,
};

pub async fn get_owner_jwt(app_state: &AppState) -> Option<String> {
    app_state
        .users_manager
        .read()
        .await
        .as_ref()
        .iter()
        .find(|(_, user)| user.is_owner)
        .and_then(|(_, user)| user.create_jwt().ok())
}

pub async fn is_owner_account_present(app_state: &AppState) -> bool {
    app_state
        .users_manager
        .read()
        .await
        .as_ref()
        .iter()
        .any(|(_, user)| user.is_owner)
}

pub async fn setup_owner_account(
    app_state: &AppState,
    username: String,
    password: String,
) -> Result<(), Error> {
    if is_owner_account_present(app_state).await {
        return Err(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Owner account already exists.".to_string(),
        });
    }
    let hashed_psw = hash_password(&password);
    let user = User::new(
        Uuid::new_v4().to_string(),
        username,
        hashed_psw,
        true,
        false,
        UserPermission::new(),
        rand_alphanumeric(32),
    );
    app_state
        .users_manager
        .write()
        .await
        .add_user(user, CausedBy::System)
        .await?;
    app_state.first_time_setup_key.lock().await.take();
    Ok(())
}
