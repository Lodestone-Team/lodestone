use crate::{
    auth::{
        jwt_token::JwtToken,
        permission::UserPermission,
        user::{check_credentials, PublicUser, User, UserAction},
        user_id::UserId,
    },
    error::{Error, ErrorKind},
    AppState,
};

use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use axum_auth::AuthBasic;

use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use super::util::AuthedUser;

#[derive(Deserialize, Serialize)]
pub struct NewUser {
    pub username: String,
    pub password: String,
}

/// Runs Argon2 (hashing or checking a password) on a blocking thread, so it does not hold up
/// the async runtime.
async fn run_argon2<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, Error> {
    Ok(tokio::task::spawn_blocking(f)
        .await
        .context("Password hashing task failed")?)
}

fn require_self_or_manage_user(requester: &User, uid: &UserId, what: &str) -> Result<(), Error> {
    if requester.uid != *uid && !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("You are not authorized to {what}"),
        });
    }
    Ok(())
}

/// Creates a user. Returns only the new user's public info: the requester has no use for the
/// new user's token.
pub async fn new_user(
    State(state): State<AppState>,
    requester: AuthedUser,
    Json(config): Json<NewUser>,
) -> Result<Json<PublicUser>, Error> {
    requester.require(&state, UserAction::ManageUser).await?;
    let user = run_argon2(move || {
        User::new(
            config.username,
            config.password,
            false,
            false,
            UserPermission::default(),
        )
    })
    .await?;
    state
        .users_manager
        .write()
        .await
        .add_user(user.clone(), requester.caused_by())
        .await?;
    Ok(Json(user.into()))
}

pub async fn delete_user(
    State(state): State<AppState>,
    Path(uid): Path<UserId>,
    requester: AuthedUser,
) -> Result<Json<Value>, Error> {
    requester.require(&state, UserAction::ManageUser).await?;

    if uid == requester.uid {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("You cannot delete yourself"),
        });
    }

    state
        .users_manager
        .write()
        .await
        .delete_user(uid, requester.caused_by())
        .await?;
    Ok(Json(json!("ok")))
}

pub async fn logout(
    State(state): State<AppState>,
    Path(uid): Path<UserId>,
    requester: AuthedUser,
) -> Result<Json<()>, Error> {
    requester.require_session()?;
    require_self_or_manage_user(&requester, &uid, "logout other users")?;
    state
        .users_manager
        .write()
        .await
        .logout_user(uid, requester.caused_by())
        .await?;
    Ok(Json(()))
}

pub async fn update_permissions(
    State(state): State<AppState>,
    Path(uid): Path<UserId>,
    requester: AuthedUser,
    Json(new_permissions): Json<UserPermission>,
) -> Result<Json<()>, Error> {
    requester
        .require(&state, UserAction::ManagePermission)
        .await?;
    let caused_by = requester.caused_by();
    let mut users_manager = state.users_manager.write().await;
    // re-read both users under the write lock, so the checks use their current permissions;
    // for an API key, the requester keeps only the key's powers
    let requester = users_manager
        .refresh_user(&requester)
        .ok_or_else(|| Error {
            kind: ErrorKind::Unauthorized,
            source: eyre!("Unauthorized"),
        })?;
    let mut target = users_manager.get_user(&uid).ok_or_else(|| Error {
        kind: ErrorKind::NotFound,
        source: eyre!("User not found"),
    })?;
    requester.update_permission(&mut target, new_permissions)?;
    users_manager
        .update_permissions(&uid, target.permissions, caused_by)
        .await?;
    Ok(Json(()))
}

pub async fn get_self_info(requester: AuthedUser) -> Json<PublicUser> {
    Json(requester.into_inner().into())
}

pub async fn get_user_info(
    State(state): State<AppState>,
    Path(uid): Path<UserId>,
    requester: AuthedUser,
) -> Result<Json<PublicUser>, Error> {
    require_self_or_manage_user(&requester, &uid, "get other users info")?;
    Ok(Json(
        state
            .users_manager
            .read()
            .await
            .get_user(&uid)
            .ok_or_else(|| Error {
                kind: ErrorKind::NotFound,
                source: eyre!("User not found"),
            })?
            .into(),
    ))
}

pub async fn rename_user(
    State(state): State<AppState>,
    Path(uid): Path<UserId>,
    requester: AuthedUser,
    Json(new_name): Json<String>,
) -> Result<Json<()>, Error> {
    require_self_or_manage_user(&requester, &uid, "rename other users")?;
    state
        .users_manager
        .write()
        .await
        .rename_user(uid, new_name, requester.caused_by())
        .await?;
    Ok(Json(()))
}

#[derive(Deserialize)]
pub struct ChangePasswordConfig {
    uid: UserId,
    old_password: Option<String>,
    new_password: String,
}

pub async fn change_password(
    State(state): State<AppState>,
    requester: AuthedUser,
    Json(config): Json<ChangePasswordConfig>,
) -> Result<Json<()>, Error> {
    requester.require_session()?;
    require_self_or_manage_user(&requester, &config.uid, "change other users password")?;
    let old_password = if requester.uid != config.uid {
        None
    } else {
        Some(config.old_password.ok_or_else(|| Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("You must provide your old password"),
        })?)
    };
    state
        .users_manager
        .write()
        .await
        .change_password(
            &config.uid,
            old_password,
            config.new_password,
            requester.caused_by(),
        )
        .await?;

    Ok(Json(()))
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct LoginReply {
    pub token: JwtToken,
    pub user: PublicUser,
}

/// Logs in with HTTP basic auth.
///
/// After too many failed attempts for a username, it answers 429 with a `Retry-After`
/// header until the window has passed (see [`crate::auth::login_limiter`]).
pub async fn login(
    State(state): State<AppState>,
    AuthBasic((username, password)): AuthBasic,
) -> Response {
    let Some(password) = password else {
        return Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("You must provide a password"),
        }
        .into_response();
    };
    // counts as a failure until the password is accepted; the same for unknown usernames,
    // so the limit does not show which usernames exist
    if let Err(limited) = state.login_limiter.begin_attempt(&username) {
        return limited.into_response();
    }
    check_login(&state, &username, password)
        .await
        .into_response()
}

async fn check_login(
    state: &AppState,
    username: &str,
    password: String,
) -> Result<Json<LoginReply>, Error> {
    let user = state
        .users_manager
        .read()
        .await
        .get_user_by_username(username);
    // check the password without holding the users lock
    let user = run_argon2(move || check_credentials(user, &password)).await??;
    state.login_limiter.record_success(username);
    Ok(Json(LoginReply {
        token: user.create_jwt()?,
        user: user.into(),
    }))
}

pub async fn get_all_users(
    State(state): State<AppState>,
    requester: AuthedUser,
) -> Result<Json<Vec<PublicUser>>, Error> {
    requester.require(&state, UserAction::ManageUser).await?;

    Ok(Json(
        state
            .users_manager
            .read()
            .await
            .as_ref()
            .values()
            .map(PublicUser::from)
            .collect(),
    ))
}

// return the thing created by Router::new() so we can nest it in main
pub fn get_user_routes(state: AppState) -> Router {
    Router::new()
        .route("/user/list", get(get_all_users))
        .route("/user", post(new_user))
        .route("/user/{uid}", get(get_user_info))
        .route("/user/{uid}", delete(delete_user))
        .route("/user/{uid}/update_perm", put(update_permissions))
        .route("/user/info", get(get_self_info))
        .route("/user/{uid}/rename", put(rename_user))
        .route("/user/{uid}/password", put(change_password))
        .route("/user/login", post(login))
        .route("/user/logout/{uid}", post(logout))
        .with_state(state)
}
