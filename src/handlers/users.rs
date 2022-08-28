use std::collections::{HashMap, HashSet};

use crate::{
    json_store::{permission::Permission, user::User},
    traits::{Error, ErrorInner},
    util::rand_alphanumeric,
    AppState,
};
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{extract::Path, Extension, Json};
use axum_auth::{AuthBasic, AuthBearer};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use systemstat::Utc;

use super::util::{hash_password, is_authorized, try_auth};
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

#[derive(Deserialize, Serialize)]
struct PermissionsUpdateSchema {
    pub permissions: HashMap<Permission, HashSet<String>>,
}

fn create_jwt(user: &User, jwt_secret: &str) -> Result<String, Error> {
    let exp = Utc::now()
        .checked_add_signed(chrono::Duration::days(1))
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
) -> Result<Json<Value>, Error> {
    let requester = try_auth(&token, state.users.lock().await.get_ref()).ok_or(Error {
        inner: ErrorInner::PermissionDenied,
        detail: "Token error".to_string(),
    })?;
    if !is_authorized(&requester, "", Permission::CanManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to create users".to_string(),
        });
    }
    let login_request: NewUserSchema = serde_json::from_value(config.clone()).map_err(|_| Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Invalid request".to_string(),
    })?;
    let hashed_psw = hash_password(&login_request.password);
    let uid = uuid::Uuid::new_v4().to_string();
    let mut users = state.users.lock().await;
    if users
        .get_ref()
        .iter().any(|(_, user)| user.username == login_request.username)
    {
        return Err(Error {
            inner: ErrorInner::UserAlreadyExists,
            detail: "".to_string(),
        });
    }
    users
        .transform({
            let uid = uid.clone();
            Box::new(move |v| {
                v.insert(
                    uid.clone(),
                    User {
                        uid: uid.clone(),
                        username: login_request.username.clone(),
                        hashed_psw: hashed_psw.clone(),
                        is_admin: false,
                        is_owner: false,
                        permissions: HashMap::new(),
                        secret: rand_alphanumeric(32),
                    },
                );
                Ok(())
            })
        })
        .unwrap();
    Ok(Json(json!(uid)))
}

pub async fn delete_user(
    Extension(state): Extension<AppState>,
    Path(uid): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let mut users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::PermissionDenied,
        detail: "Token error".to_string(),
    })?;
    if !is_authorized(&requester, "", Permission::CanManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to create users".to_string(),
        });
    }
    users
        .transform(Box::new(move |v| {
            v.remove(&uid);
            Ok(())
        }))
        .unwrap();
    Ok(Json(json!("ok")))
}

pub async fn update_permissions(
    Extension(state): Extension<AppState>,
    Path(uid): Path<String>,
    Json(config): Json<Value>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let mut users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::PermissionDenied,
        detail: "".to_string(),
    })?;
    if !is_authorized(&requester, "", Permission::CanManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to update permissions".to_string(),
        });
    }
    if config["uid"].is_null() {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Invalid request".to_string(),
        });
    }
    let permissions_update_request: PermissionsUpdateSchema =
        serde_json::from_value(config.clone()).map_err(|_| Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Invalid request".to_string(),
        })?;
    users
        .transform(Box::new(move |v| {
            let user = v.get_mut(&uid).ok_or(Error {
                inner: ErrorInner::UserNotFound,
                detail: "".to_string(),
            })?;
            user.permissions = permissions_update_request.permissions.clone();
            Ok(())
        }))
        ?;
    Ok(Json(json!("ok")))
}

pub async fn get_user_info(
    Extension(state): Extension<AppState>,
    Path(uid): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::PermissionDenied,
        detail: "".to_string(),
    })?;
    if requester.uid != uid && !is_authorized(&requester, "", Permission::CanManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to get this user's info".to_string(),
        });
    }
    let mut user = users
        .get_ref()
        .get(&uid)
        .ok_or(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "".to_string(),
        })?
        .to_owned();
    user.hashed_psw = "".to_string();
    user.secret = "".to_string();
    Ok(Json(json!(user)))
}

pub async fn change_password(
    Extension(state): Extension<AppState>,
    Json(config): Json<Value>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let mut users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::PermissionDenied,
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
    Ok(Json(json!("ok")))
}

pub async fn login(
    Extension(state): Extension<AppState>,
    AuthBasic((username, psw)): AuthBasic,
) -> Result<Json<Value>, Error> {
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
        {}
        Ok(Json(json!(create_jwt(user, &user.secret)?)))
    } else {
        Err(Error {
            inner: ErrorInner::UserNotFound,
            detail: "".to_string(),
        })
    }
}
