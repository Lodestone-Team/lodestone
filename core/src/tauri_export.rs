use color_eyre::eyre::eyre;

use crate::{
    auth::{jwt_token::JwtToken, permission::UserPermission, user::User},
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
    if is_owner_account_present(app_state).await {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Owner account already present"),
        });
    }
    let user = User::new(username, password, true, false, UserPermission::default());
    app_state
        .users_manager
        .write()
        .await
        .add_user(user, CausedBy::System)
        .await?;
    app_state.first_time_setup_key.lock().await.take();
    Ok(())
}

pub async fn get_first_time_setup_key(app_state: &AppState) -> Option<String> {
    app_state.first_time_setup_key.lock().await.clone()
}
