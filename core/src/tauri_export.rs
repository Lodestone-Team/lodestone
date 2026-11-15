use color_eyre::eyre::eyre;

use crate::{
    auth::{
        jwt_token::JwtToken,
        permission::UserPermission,
        user::{valid_username, User},
    },
    error::{Error, ErrorKind},
    events::CausedBy,
    AppState,
};

pub async fn get_owner_jwt(app_state: &AppState) -> Option<JwtToken> {
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
    let username = valid_username(&username).map_err(|message| Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message.to_string()),
    })?;
    let user = User::new(username, password, true, false, UserPermission::default());
    // Same lock order as the `/setup/{key}` handler (setup key, then users), and the owner
    // check and the insert happen under one users write lock, so two owners cannot be
    // created at once.
    let mut setup_key_lock = app_state.first_time_setup_key.lock().await;
    let mut users_manager = app_state.users_manager.write().await;
    if users_manager.as_ref().values().any(|user| user.is_owner) {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Owner account already present"),
        });
    }
    users_manager.add_user(user, CausedBy::System).await?;
    setup_key_lock.take();
    drop(users_manager);
    drop(setup_key_lock);
    // an owner with a password: other devices may use the core, as always
    crate::remote_access::owner_created(app_state, true).await;
    Ok(())
}

pub async fn get_first_time_setup_key(app_state: &AppState) -> Option<String> {
    app_state.first_time_setup_key.lock().await.clone()
}
