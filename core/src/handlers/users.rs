use crate::{
    auth::{
        jwt_token::JwtToken,
        permission::UserPermission,
        user::{PublicUser, User, UserAction},
        user_id::UserId,
    },
    error::{Error, ErrorKind},
    events::CausedBy,
    AppState,
};

use axum::{
    extract::Path,
    routing::{delete, get, post, put},
    Json, Router,
};
use axum_auth::{AuthBasic, AuthBearer};

use color_eyre::eyre::eyre;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

#[derive(Deserialize, Serialize)]
pub struct NewUser {
    pub username: String,
    pub password: String,
}

pub async fn new_user(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
    Json(config): Json<NewUser>,
) -> Result<Json<LoginReply>, Error> {
    let mut users_manager = state.users_manager.write().await;
    let requester = users_manager.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::ManageUser)?;
    let user = User::new(
        config.username,
        config.password,
        false,
        false,
        UserPermission::default(),
    );
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    users_manager
        .add_user(user.clone(), caused_by.clone())
        .await?;
    Ok(Json(LoginReply {
        token: user.create_jwt()?,
        user: user.into(),
    }))
}

pub async fn delete_user(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uid): Path<UserId>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let mut users_manager = state.users_manager.write().await;
    let requester = users_manager.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::ManageUser)?;

    if uid == requester.uid {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("You cannot delete yourself"),
        });
    }

    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    users_manager
        .delete_user(uid.clone(), caused_by.clone())
        .await?;
    Ok(Json(json!("ok")))
}

pub async fn logout(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uid): Path<UserId>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let mut users_manager = state.users_manager.write().await;

    let requester = users_manager.try_auth_or_err(&token)?;

    if requester.uid != uid && !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("You are not authorized to logout other users"),
        });
    }
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username,
    };
    users_manager
        .logout_user(uid.clone(), caused_by.clone())
        .await?;
    Ok(Json(()))
}

pub async fn update_permissions(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uid): Path<UserId>,
    AuthBearer(token): AuthBearer,
    Json(new_permissions): Json<UserPermission>,
) -> Result<Json<()>, Error> {
    let mut users_manager = state.users_manager.write().await;

    let requester = users_manager.try_auth_or_err(&token)?;
    requester.try_action(&UserAction::ManagePermission)?;
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    users_manager
        .update_permissions(uid, new_permissions, caused_by)
        .await?;
    Ok(Json(()))
}

pub async fn get_self_info(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<PublicUser>, Error> {
    Ok(Json(
        state
            .users_manager
            .read()
            .await
            .try_auth_or_err(&token)?
            .into(),
    ))
}

pub async fn get_user_info(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uid): Path<UserId>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<PublicUser>, Error> {
    let users_manager = state.users_manager.read().await;

    let requester = users_manager.try_auth_or_err(&token)?;
    if requester.uid != uid && !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("You are not authorized to get other users info"),
        });
    }
    Ok(Json(
        users_manager
            .get_user(&uid)
            .ok_or(Error {
                kind: ErrorKind::NotFound,
                source: eyre!("User not found"),
            })?
            .into(),
    ))
}

pub async fn rename_user(
    axum::extract::State(state): axum::extract::State<AppState>,
    Path(uid): Path<UserId>,
    AuthBearer(token): AuthBearer,
    Json(new_name): Json<String>,
) -> Result<Json<()>, Error> {
    let mut users_manager = state.users_manager.write().await;

    let requester = users_manager.try_auth_or_err(&token)?;

    if requester.uid != uid && !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("You are not authorized to rename other users"),
        });
    }

    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    users_manager.rename_user(uid, new_name, caused_by).await?;
    Ok(Json(()))
}

#[derive(Deserialize)]
pub struct ChangePasswordConfig {
    uid: UserId,
    old_password: Option<String>,
    new_password: String,
}

pub async fn change_password(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
    Json(config): Json<ChangePasswordConfig>,
) -> Result<Json<()>, Error> {
    let mut users_manager = state.users_manager.write().await;

    let requester = users_manager.try_auth_or_err(&token)?;

    if requester.uid != config.uid || !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!("You are not authorized to change other users password"),
        });
    }

    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username,
    };
    users_manager
        .change_password(
            &config.uid,
            if requester.uid != config.uid {
                None
            } else {
                Some(config.old_password.ok_or_else(|| Error {
                    kind: ErrorKind::BadRequest,
                    source: eyre!("You must provide your old password"),
                })?)
            },
            config.new_password,
            caused_by,
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

pub async fn login(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBasic((username, password)): AuthBasic,
) -> Result<Json<LoginReply>, Error> {
    if let Some(password) = password {
        let users_manager = state.users_manager.read().await;

        Ok(Json(LoginReply {
            token: users_manager.login(&username, &password)?,
            user: users_manager
                .get_user_by_username(&username)
                .ok_or_else(|| Error {
                    kind: ErrorKind::NotFound,
                    source: eyre!("User not found"),
                })?
                .into(),
        }))
    } else {
        Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("You must provide a password"),
        })
    }
}

pub async fn get_all_users(
    axum::extract::State(state): axum::extract::State<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<PublicUser>>, Error> {
    let users_manager = state.users_manager.read().await;

    let requester = users_manager.try_auth_or_err(&token)?;

    requester.try_action(&UserAction::ManageUser)?;

    Ok(Json(
        users_manager
            .as_ref()
            .iter()
            .map(|(_, v)| v.into())
            .collect(),
    ))
}

// return the thing created by Router::new() so we can nest it in main
pub fn get_user_routes(state: AppState) -> Router {
    Router::new()
        .route("/user/list", get(get_all_users))
        .route("/user", post(new_user))
        .route("/user/:uid", get(get_user_info))
        .route("/user/:uid", delete(delete_user))
        .route("/user/:uid/update_perm", put(update_permissions))
        .route("/user/info", get(get_self_info))
        .route("/user/:uid/rename", put(rename_user))
        .route("/user/:uid/password", put(change_password))
        .route("/user/login", post(login))
        .route("/user/logout/:uid", post(logout))
        .with_state(state)
}
