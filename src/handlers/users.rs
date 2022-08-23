use std::collections::{HashMap, HashSet};

use crate::{
    db::{permission::Permission, user::User},
    traits::{Error, ErrorInner},
    util::rand_alphanumeric,
    AppState,
};
use axum::{extract::Path, Extension, Json};
use axum_auth::{AuthBasic, AuthBearer};
use crypto::{digest::Digest, sha3::Sha3};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use systemstat::Utc;

use super::util::{is_authorized, try_auth};
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
        detail: "".to_string(),
    })?;
    if !is_authorized(&requester, "", Permission::CanManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to create users".to_string(),
        });
    }
    let login_request: NewUserSchema = serde_json::from_value(config.clone()).or(Err(Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Invalid request".to_string(),
    }))?;
    let salt: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(5)
        .map(char::from)
        .collect();
    let mut hasher = Sha3::sha3_256();
    hasher.input_str(format!("{}{}", salt, login_request.password).as_str());
    let hashed_psw = hasher.result_str();
    let uid = uuid::Uuid::new_v4().to_string();
    let mut users = state.users.lock().await;
    if !users
        .get_ref()
        .iter()
        .find(|(_, user)| user.username == login_request.username)
        .is_none()
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
                        salt: salt.clone(),
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
        detail: "".to_string(),
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
        serde_json::from_value(config.clone()).or(Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Invalid request".to_string(),
        }))?;
    users
        .transform(Box::new(move |v| {
            let user = v.get_mut(&uid).unwrap();
            user.permissions = permissions_update_request.permissions.clone();
            Ok(())
        }))
        .unwrap();
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
            inner: ErrorInner::UserNotFound,
            detail: "".to_string(),
        })?
        .to_owned();
    user.salt = "".to_string();
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
            let mut hasher = Sha3::sha3_256();
            hasher.input_str(format!("{}{}", user.salt, new_psw).as_str());
            let hashed_psw = hasher.result_str();
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
    if let Some((_uuid, user)) = state
        .users
        .lock()
        .await
        .get_ref()
        .iter()
        .find(|(_, user)| user.username == username)
    {
        let mut hasher = Sha3::sha3_256();
        hasher.input_str(
            format!(
                "{}{}",
                user.salt,
                psw.ok_or(Error {
                    inner: ErrorInner::MalformedRequest,
                    detail: "".to_string()
                })?
            )
            .as_str(),
        );
        let hashed_psw = hasher.result_str();
        if user.hashed_psw != hashed_psw {
            return Err(Error {
                inner: ErrorInner::InvalidPassword,
                detail: "".to_string(),
            });
        }
        Ok(Json(json!(create_jwt(&user, &user.secret)?)))
    } else {
        Err(Error {
            inner: ErrorInner::UserNotFound,
            detail: "".to_string(),
        })
    }
}
