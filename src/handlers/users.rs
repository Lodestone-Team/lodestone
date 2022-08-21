use std::collections::{HashMap, HashSet};

use crate::{
    db::{permission::Permission, user::User},
    handlers::util::JWT_SECRET,
    traits::{Error, ErrorInner},
    AppState,
};
use axum::{extract::Path, Extension, Json};
use axum_auth::{AuthBasic, AuthBearer};
use crypto::{digest::Digest, sha3::Sha3};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use log::debug;
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use systemstat::Utc;

use super::util::{decode_token, is_authorized};
#[derive(Deserialize, Serialize)]
pub struct Claim {
    pub user: User,
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

fn create_jwt(user: &User) -> Result<String, Error> {
    let exp = Utc::now()
        .checked_add_signed(chrono::Duration::days(1))
        .expect("valid timestamp")
        .timestamp();
    debug!("exp: {}", exp);
    let claim = Claim {
        user: user.clone(),
        exp: exp as usize,
    };
    let header = Header::new(Algorithm::HS512);
    Ok(encode(
        &header,
        &claim,
        &EncodingKey::from_secret(&*JWT_SECRET.as_bytes()),
    )
    .unwrap())
}

pub async fn new_user(
    Extension(state): Extension<AppState>,
    Json(config): Json<Value>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    if !is_authorized(&token, "", Permission::CanManageUser) {
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
    let uuid = uuid::Uuid::new_v4().to_string();
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
        .transform(Box::new({
            let uuid = uuid.clone();
            move |v| {
                v.insert(
                    uuid.clone(),
                    User {
                        uuid: uuid.clone(),
                        username: login_request.username.clone(),
                        hashed_psw: hashed_psw.clone(),
                        salt: salt.clone(),
                        is_admin: false,
                        is_owner: false,
                        permissions: HashMap::new(),
                    },
                );
                Ok(())
            }
        }))
        .unwrap();
    Ok(Json(json!(uuid)))
}

pub async fn delete_user(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    if !is_authorized(&token, "", Permission::CanManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to delete users".to_string(),
        });
    }
    let mut users = state.users.lock().await;
    users
        .transform(Box::new(move |v| {
            v.remove(&uuid);
            Ok(())
        }))
        .unwrap();
    Ok(Json(json!("ok")))
}

pub async fn update_permissions(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
    Json(config): Json<Value>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    if !is_authorized(&token, "", Permission::CanManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to update permissions".to_string(),
        });
    }
    let permissions_update_request: PermissionsUpdateSchema =
        serde_json::from_value(config.clone()).or(Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Invalid request".to_string(),
        }))?;
    let mut users = state.users.lock().await;
    users
        .transform(Box::new(move |v| {
            let user = v.get_mut(&uuid).unwrap();
            user.permissions = permissions_update_request.permissions.clone();
            Ok(())
        }))
        .unwrap();
    Ok(Json(json!("ok")))
}

pub async fn get_user_info(
    Extension(state): Extension<AppState>,
    Path(uuid): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Value>, Error> {
    let user: User = decode_token(token.as_str()).ok_or(Error {
        inner: ErrorInner::PermissionDenied,
        detail: "Invalid token".to_string(),
    })?;
    if user.uuid != uuid && !is_authorized(&token, "", Permission::CanManageUser) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "You are not authorized to get this user's info".to_string(),
        });
    }
    let users = state.users.lock().await;
    let user = users.get_ref().get(&uuid).ok_or(Error {
        inner: ErrorInner::UserNotFound,
        detail: "".to_string(),
    })?;
    Ok(Json(json!(user)))
}

pub async fn login(
    Extension(state): Extension<AppState>,
    AuthBasic((username, psw)): AuthBasic,
) -> Result<Json<Value>, Error> {
    if let Some(user) = state
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
                user.1.salt,
                psw.ok_or(Error {
                    inner: ErrorInner::MalformedRequest,
                    detail: "".to_string()
                })?
            )
            .as_str(),
        );
        let hashed_psw = hasher.result_str();
        if user.1.hashed_psw != hashed_psw {
            return Err(Error {
                inner: ErrorInner::InvalidPassword,
                detail: "".to_string(),
            });
        }
        Ok(Json(json!(create_jwt(&user.1)?)))
    } else {
        Err(Error {
            inner: ErrorInner::UserNotFound,
            detail: "".to_string(),
        })
    }
}
