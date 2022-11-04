use crate::{
    auth::{
        permission::UserPermission,
        user::{PublicUser, User, UserAction},
    },
    events::{Event, EventInner, UserEvent, UserEventInner},
    traits::{Error, ErrorInner},
    util::rand_alphanumeric,
    AppState, prelude::get_snowflake,
};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    extract::Path,
    routing::{delete, get, post, put},
    Extension, Json, Router,
};
use axum_auth::{AuthBasic, AuthBearer};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use super::util::{hash_password, try_auth};
#[derive(Deserialize, Serialize)]
pub struct Claim {
    pub uid: String,
    pub exp: usize,
}

#[derive(Deserialize, Serialize)]
struct NewUserSchema {
    pub username: String,
    pub password: String,
}

fn create_jwt(user: &User, jwt_secret: &str) -> Result<String, Error> {
    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::days(60))
        .expect("valid timestamp")
        .timestamp();
    let claim = Claim {
        uid: user.uid.clone(),
        exp: exp as usize,
    };
    let header = Header::new(Algorithm::HS512);
    Ok(encode(
        &header,
        &claim,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .unwrap())
}

pub async fn new_user(
    Extension(state): Extension<AppState>,
    Json(config): Json<Value>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<LoginReply>, Error> {
    let requester = try_auth(&token, state.users.lock().await.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to create users".to_string(),
        });
    }
    let login_request: NewUserSchema =
        serde_json::from_value(config.clone()).map_err(|_| Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Invalid request".to_string(),
        })?;
    let hashed_psw = hash_password(&login_request.password);
    let uid = uuid::Uuid::new_v4().to_string();
    let users = state.users.lock().await;
    if users
        .get_ref()
        .iter()
        .any(|(_, user)| user.username == login_request.username)
    {
        return Err(Error {
            inner: ErrorInner::UserAlreadyExists,
            detail: "".to_string(),
        });
    }
    let user = User {
        uid: uid.clone(),
        username: login_request.username.clone(),
        hashed_psw: hashed_psw.clone(),
        is_admin: false,
        is_owner: false,
        permissions: UserPermission::new(),
        secret: rand_alphanumeric(32),
    };
    tokio::task::spawn({
        let uid = uid.clone();
        let users = state.users.clone();
        let user = user.clone();
        async move {
            users
                .lock()
                .await
                .transform({
                    let uid = uid.clone();
                    Box::new(move |v| {
                        v.insert(uid.clone(), user.clone());
                        Ok(())
                    })
                })
                .unwrap();
        }
    });
    state
        .event_broadcaster
        .send(Event {
            event_inner: EventInner::UserEvent(UserEvent {
                user_id: uid.clone(),
                user_event_inner: UserEventInner::UserCreated,
            }),
            details: "".to_string(),
            snowflake: get_snowflake(),
            idempotency: rand_alphanumeric(5),
        })
        .map_err(|e| error!("Error sending event: {}", e));
    Ok(Json(LoginReply {
        token: create_jwt(&user, &user.secret)?,
        user: user.into(),
    }))
}

pub async fn delete_user(
    Extension(state): Extension<AppState>,
    Path(uid): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let mut users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to create users".to_string(),
        });
    }
    users
        .transform(Box::new({
            let uid = uid.clone();
            move |v| {
                v.remove(&uid);
                Ok(())
            }
        }))
        .unwrap();
    state
        .event_broadcaster
        .send(Event {
            event_inner: EventInner::UserEvent(UserEvent {
                user_id: uid,
                user_event_inner: UserEventInner::UserDeleted,
            }),
            details: "".to_string(),
            snowflake: get_snowflake(),
            idempotency: rand_alphanumeric(5),
        })
        .map_err(|e| error!("Error sending event: {}", e));
    Ok(Json(json!("ok")))
}

pub async fn logout(
    Extension(state): Extension<AppState>,
    Path(uid): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<String>, Error> {
    let mut users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
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
    users
        .transform(Box::new(move |v| {
            v.get_mut(&uid).unwrap().secret = rand_alphanumeric(32);
            Ok(())
        }))
        .unwrap();
    state
        .event_broadcaster
        .send(Event {
            event_inner: EventInner::UserEvent(UserEvent {
                user_id,
                user_event_inner: UserEventInner::UserLoggedOut,
            }),
            details: "".to_string(),
            snowflake: get_snowflake(),
            idempotency: rand_alphanumeric(5),
        })
        .map_err(|e| error!("Error sending event: {}", e));
    Ok(Json("ok".to_string()))
}

pub async fn update_permissions(
    Extension(state): Extension<AppState>,
    Path(uid): Path<String>,
    Json(new_permissions): Json<UserPermission>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let mut users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to update permissions".to_string(),
        });
    }
    users.transform(Box::new(move |v| {
        let user = v.get_mut(&uid).ok_or(Error {
            inner: ErrorInner::UserNotFound,
            detail: "".to_string(),
        })?;
        requester.update_permission(user, new_permissions.clone())
    }))?;
    Ok(Json(json!("ok")))
}

pub async fn get_self_info(
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<PublicUser>, Error> {
    let users = state.users.lock().await;
    Ok(Json(
        (&try_auth(&token, users.get_ref()).ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?)
            .into(),
    ))
}

pub async fn get_user_info(
    Extension(state): Extension<AppState>,
    Path(uid): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<PublicUser>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
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
        users
            .get_ref()
            .get(&uid)
            .ok_or(Error {
                inner: ErrorInner::MalformedRequest,
                detail: "".to_string(),
            })?
            .into(),
    ))
}

pub async fn change_password(
    Extension(state): Extension<AppState>,
    Json(config): Json<Value>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let mut users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Invalid authorization".to_string(),
    })?;
    let uid = config
        .get("uid")
        .ok_or(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Invalid request, field uid must be present".to_string(),
        })?
        .as_str()
        .ok_or(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Invalid request, field uid must be a string".to_string(),
        })?
        .to_string();
    if requester.uid != uid {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to change this user's password".to_string(),
        });
    }
    let new_psw = config
        .get("new_psw")
        .ok_or(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Invalid request, field new_psw must be present".to_string(),
        })?
        .as_str()
        .ok_or(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Invalid request, field new_psw must be a string".to_string(),
        })?
        .to_string();
    users
        .transform(Box::new(move |users| {
            let user = users.get_mut(&uid).unwrap();
            let hashed_psw = hash_password(&new_psw);
            user.hashed_psw = hashed_psw;
            user.secret = rand_alphanumeric(32);
            Ok(())
        }))
        .unwrap();
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
    AuthBasic((username, psw)): AuthBasic,
) -> Result<Json<LoginReply>, Error> {
    if psw.is_none() {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Invalid request, password must be present".to_string(),
        });
    }
    if let Some((_uuid, user)) = state
        .users
        .lock()
        .await
        .get_ref()
        .iter()
        .find(|(_, user)| user.username == username)
    {
        if Argon2::default()
            .verify_password(
                psw.unwrap().as_bytes(),
                &PasswordHash::new(&user.hashed_psw).unwrap(),
            )
            .is_err()
        {
            Err(Error {
                inner: ErrorInner::Unauthorized,
                detail: "Invalid username or password".to_string(),
            })
        } else {
            Ok(Json(LoginReply {
                token: create_jwt(user, &user.secret)?,
                user: user.into(),
            }))
        }
    } else {
        Err(Error {
            inner: ErrorInner::UserNotFound,
            detail: "".to_string(),
        })
    }
}

pub async fn get_all_users(
    Extension(state): Extension<AppState>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<PublicUser>>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Invalid authorization".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::ManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to get all users".to_string(),
        });
    }
    let users = users
        .get_ref()
        .iter()
        .map(|(_, user)| user.into())
        .collect();
    Ok(Json(users))
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
