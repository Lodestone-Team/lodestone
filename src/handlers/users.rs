use crate::{
    auth::{
        permission::UserPermission,
        user::{PublicUser, User, UserAction},
    },
    events::{CausedBy, Event, EventInner, UserEvent, UserEventInner},
    traits::{Error, ErrorInner},
    types::{Snowflake, UserId},
    util::{hash_password, rand_alphanumeric},
    AppState,
};

use axum::{
    extract::Path,
    routing::{delete, get, post, put},
    Extension, Json, Router,
};
use axum_auth::{AuthBasic, AuthBearer};

use log::error;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

#[derive(Deserialize, Serialize)]
pub struct Claim {
    pub uid: UserId,
    pub exp: usize,
}

#[derive(Deserialize, Serialize)]
pub struct NewUserSchema {
    pub username: String,
    pub password: String,
}

pub async fn new_user(
    Extension(state): Extension<AppState>,
    Json(config): Json<NewUserSchema>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<LoginReply>, Error> {
    let mut users_manager = state.users_manager.write().await;
    let requester = users_manager.try_auth(&token).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to create users".to_string(),
        });
    }

    let hashed_psw = hash_password(&config.password);
    let uid = UserId::default();
    let user = User {
        uid: uid.clone(),
        username: config.username.clone(),
        hashed_psw: hashed_psw.clone(),
        is_admin: false,
        is_owner: false,
        permissions: UserPermission::new(),
        secret: rand_alphanumeric(32),
    };
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    users_manager
        .add_user(user.clone(), caused_by.clone())
        .await?;
    let _ = state
        .event_broadcaster
        .send(Event {
            event_inner: EventInner::UserEvent(UserEvent {
                user_id: uid.clone(),
                user_event_inner: UserEventInner::UserCreated,
            }),
            details: "".to_string(),
            snowflake: Snowflake::default(),
            caused_by,
        })
        .map_err(|e| error!("Error sending event: {}", e));
    Ok(Json(LoginReply {
        token: user.create_jwt()?,
        user: user.into(),
    }))
}

pub async fn delete_user(
    Extension(state): Extension<AppState>,
    Path(uid): Path<UserId>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let mut users_manager = state.users_manager.write().await;
    let requester = users_manager.try_auth(&token).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to create users".to_string(),
        });
    }

    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    users_manager
        .delete_user(uid.clone(), caused_by.clone())
        .await?;
    let _ = state
        .event_broadcaster
        .send(Event {
            event_inner: EventInner::UserEvent(UserEvent {
                user_id: uid,
                user_event_inner: UserEventInner::UserDeleted,
            }),
            details: "".to_string(),
            snowflake: Snowflake::default(),
            caused_by,
        })
        .map_err(|e| error!("Error sending event: {}", e));
    Ok(Json(json!("ok")))
}

pub async fn logout(
    Extension(state): Extension<AppState>,
    Path(uid): Path<UserId>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<String>, Error> {
    let mut users_manager = state.users_manager.write().await;

    let requester = users_manager.try_auth(&token).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if requester.uid != uid && !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to log out this user".to_string(),
        });
    }
    let user_id = uid.clone();
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username,
    };
    users_manager
        .logout_user(uid.clone(), caused_by.clone())
        .await?;
    let _ = state
        .event_broadcaster
        .send(Event {
            event_inner: EventInner::UserEvent(UserEvent {
                user_id,
                user_event_inner: UserEventInner::UserLoggedOut,
            }),
            details: "".to_string(),
            snowflake: Snowflake::default(),
            caused_by,
        })
        .map_err(|e| error!("Error sending event: {}", e));
    Ok(Json("ok".to_string()))
}

pub async fn update_permissions(
    Extension(state): Extension<AppState>,
    Path(uid): Path<UserId>,
    Json(new_permissions): Json<UserPermission>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let mut users_manager = state.users_manager.write().await;

    let requester = users_manager.try_auth(&token).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to update permissions".to_string(),
        });
    }
    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username.clone(),
    };
    users_manager
        .update_permissions(uid, new_permissions, caused_by)
        .await?;
    Ok(Json(json!("ok")))
}

pub async fn get_self_info(
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<PublicUser>, Error> {
    Ok(Json(
        (&state
            .users_manager
            .read()
            .await
            .try_auth(&token)
            .ok_or(Error {
                inner: ErrorInner::Unauthorized,
                detail: "Token error".to_string(),
            })?)
            .into(),
    ))
}

pub async fn get_user_info(
    Extension(state): Extension<AppState>,
    Path(uid): Path<UserId>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<PublicUser>, Error> {
    let users_manager = state.users_manager.read().await;

    let requester = users_manager.try_auth(&token).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "".to_string(),
    })?;
    if requester.uid != uid && !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to get this user's info".to_string(),
        });
    }
    Ok(Json(
        users_manager
            .get_user(&uid)
            .ok_or(Error {
                inner: ErrorInner::NotFound,
                detail: "User not found".to_string(),
            })?
            .into(),
    ))
}

#[derive(Deserialize)]
pub struct ChangePasswordConfig {
    uid: UserId,
    password: String,
}

pub async fn change_password(
    Extension(state): Extension<AppState>,
    Json(config): Json<ChangePasswordConfig>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let mut users_manager = state.users_manager.write().await;
    let requester = users_manager.try_auth(&token).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Invalid authorization".to_string(),
    })?;

    if requester.uid != config.uid && requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to change this user's password".to_string(),
        });
    }

    let caused_by = CausedBy::User {
        user_id: requester.uid.clone(),
        user_name: requester.username,
    };
    users_manager
        .change_password(config.uid, config.password, caused_by)
        .await?;

    Ok(Json(()))
}

#[derive(Serialize, TS)]
#[ts(export)]
pub struct LoginReply {
    token: String,
    user: PublicUser,
}

pub async fn login(
    Extension(state): Extension<AppState>,
    AuthBasic((username, password)): AuthBasic,
) -> Result<Json<LoginReply>, Error> {
    if let Some(password) = password {
        let users_manager = state.users_manager.read().await;

        Ok(Json(LoginReply {
            token: users_manager.login(&username, &password)?,
            user: users_manager
                .get_user_by_username(&username)
                .ok_or_else(|| Error {
                    inner: ErrorInner::UserNotFound,
                    detail: "User not found".to_string(),
                })?
                .into(),
        }))
    } else {
        Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Invalid request, password must be present".to_string(),
        })
    }
}

pub async fn get_all_users(
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<PublicUser>>, Error> {
    let users_manager = state.users_manager.read().await;
    let requester = users_manager.try_auth(&token).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Invalid authorization".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to get all users".to_string(),
        });
    }

    Ok(Json(
        users_manager
            .as_ref()
            .iter()
            .map(|(_, v)| v.into())
            .collect(),
    ))
}

// return the thing created by Router::new() so we can nest it in main
pub fn get_user_routes() -> Router {
    Router::new()
        .route("/user/list", get(get_all_users))
        .route("/user", post(new_user))
        .route("/user/:uid", get(get_user_info))
        .route("/user/:uid", delete(delete_user))
        .route("/user/:uid/update_perm", put(update_permissions))
        .route("/user/info", get(get_self_info))
        .route("/user/password", put(change_password))
        .route("/user/login", post(login))
        .route("/user/logout/:uid", post(logout))
}
