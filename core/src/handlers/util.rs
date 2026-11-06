use std::ops::Deref;

use axum::{
    extract::FromRequestParts,
    http::request::Parts,
    response::{IntoResponse, Response},
};
use axum_auth::AuthBearer;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use color_eyre::eyre::{eyre, Context};

use crate::{
    auth::user::{User, UserAction},
    error::{Error, ErrorKind},
    events::CausedBy,
    prelude::GameInstance,
    types::InstanceUuid,
    AppState,
};

pub fn parse_bearer_token(token: &str) -> Option<String> {
    let mut split = token.split_ascii_whitespace();
    if split.next()? != "Bearer" {
        return None;
    }
    split.next().map(|s| s.to_string())
}

pub fn decode_base64(input: &str) -> Result<String, Error> {
    Ok(String::from_utf8(
        URL_SAFE_NO_PAD
            .decode(input)
            .context("Failed to decode base64")?,
    )
    .context("Invalid UTF-8")?)
}

fn unauthorized() -> Error {
    Error {
        kind: ErrorKind::Unauthorized,
        source: eyre!("Unauthorized"),
    }
}

/// Returns the user that `token` (a bare JWT) belongs to.
pub async fn authenticate(state: &AppState, token: &str) -> Result<User, Error> {
    state.users_manager.read().await.try_auth_or_err(token)
}

/// Returns the user for a WebSocket `token` query parameter, which has the form
/// `Bearer <jwt>` because browsers cannot set headers on WebSocket requests.
pub async fn authenticate_query_token(state: &AppState, token: &str) -> Result<User, Error> {
    let token = parse_bearer_token(token).ok_or_else(unauthorized)?;
    authenticate(state, &token).await
}

/// Reads the safe mode flag. All permission checks go through this.
pub async fn safe_mode(state: &AppState) -> bool {
    state.global_settings.lock().await.safe_mode()
}

/// Checks that `user` may perform `action`, taking safe mode into account.
pub async fn require(state: &AppState, user: &User, action: UserAction) -> Result<(), Error> {
    user.try_action(&action, safe_mode(state).await)
}

/// A logged-in user, taken from the `Authorization: Bearer <jwt>` header.
///
/// A missing or malformed header is rejected the same way `AuthBearer` rejects it; a token
/// that does not belong to a user gets a 401 with the usual JSON error body.
pub struct AuthedUser(pub User);

impl FromRequestParts<AppState> for AuthedUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Response> {
        let AuthBearer(token) = AuthBearer::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        authenticate(state, &token)
            .await
            .map(AuthedUser)
            .map_err(IntoResponse::into_response)
    }
}

impl Deref for AuthedUser {
    type Target = User;

    fn deref(&self) -> &User {
        &self.0
    }
}

impl AuthedUser {
    /// Checks that this user may perform `action`, taking safe mode into account.
    pub async fn require(&self, state: &AppState, action: UserAction) -> Result<(), Error> {
        require(state, &self.0, action).await
    }

    pub fn require_owner(&self) -> Result<(), Error> {
        if self.0.is_owner {
            Ok(())
        } else {
            Err(Error {
                kind: ErrorKind::PermissionDenied,
                source: eyre!("Only the owner can do this"),
            })
        }
    }

    pub fn caused_by(&self) -> CausedBy {
        CausedBy::User {
            user_id: self.0.uid.clone(),
            user_name: self.0.username.clone(),
        }
    }

    pub fn into_inner(self) -> User {
        self.0
    }
}

/// Returns a clone of the instance, so that no `DashMap` guard is held across an `.await`.
pub fn get_instance(state: &AppState, uuid: &InstanceUuid) -> Result<GameInstance, Error> {
    state
        .instances
        .get(uuid)
        .map(|instance| instance.value().clone())
        .ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Instance not found"),
        })
}

/// Returns clones of all instances, for the same reason as [`get_instance`].
pub fn all_instances(state: &AppState) -> Vec<GameInstance> {
    state
        .instances
        .iter()
        .map(|instance| instance.value().clone())
        .collect()
}
