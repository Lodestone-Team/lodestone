//! Linking Minecraft accounts to Lodestone users, see [`crate::minecraft_links`]:
//!
//! - `GET /user/minecraft_links`: the requester's links;
//! - `POST /user/minecraft_links`: links the requester's Minecraft account, proven with
//!   its Minecraft access token (used once, never stored or logged);
//! - `DELETE /user/minecraft_links/{minecraft_uuid}`: removes one of the requester's
//!   links;
//! - `GET /minecraft_links`, `DELETE /minecraft_links/{minecraft_uuid}`: every link, for
//!   the owner.

use axum::{
    extract::{Path, State},
    routing::{delete, get},
    Json, Router,
};
use color_eyre::eyre::eyre;

use super::util::AuthedUser;
use crate::{
    auth::api_key::now,
    error::{Error, ErrorKind},
    minecraft_links::{LinkMinecraftAccount, MinecraftLink, MinecraftLinkInfo},
    AppState,
};

fn not_linked() -> Error {
    Error {
        kind: ErrorKind::NotFound,
        source: eyre!("That Minecraft account isn't linked"),
    }
}

async fn get_own_links(
    State(state): State<AppState>,
    requester: AuthedUser,
) -> Json<Vec<MinecraftLink>> {
    Json(state.minecraft_links.for_user(&requester.uid).await)
}

async fn link_account(
    State(state): State<AppState>,
    requester: AuthedUser,
    Json(request): Json<LinkMinecraftAccount>,
) -> Result<Json<MinecraftLink>, Error> {
    Ok(Json(
        state
            .minecraft_links
            .link(&requester.uid, request, now())
            .await?,
    ))
}

async fn unlink_own(
    State(state): State<AppState>,
    requester: AuthedUser,
    Path(minecraft_uuid): Path<String>,
) -> Result<Json<()>, Error> {
    if state
        .minecraft_links
        .remove(&minecraft_uuid, Some(&requester.uid))
        .await?
    {
        Ok(Json(()))
    } else {
        Err(not_linked())
    }
}

async fn get_all_links(
    State(state): State<AppState>,
    requester: AuthedUser,
) -> Result<Json<Vec<MinecraftLinkInfo>>, Error> {
    requester.require_owner()?;
    let links = state.minecraft_links.all().await;
    let users = state.users_manager.read().await;
    Ok(Json(
        links
            .into_iter()
            .filter_map(|link| {
                // links of a user deleted while the file could not be saved are left out
                let username = users.get_user(&link.uid)?.username;
                Some(MinecraftLinkInfo::new(link, username))
            })
            .collect(),
    ))
}

async fn unlink_any(
    State(state): State<AppState>,
    requester: AuthedUser,
    Path(minecraft_uuid): Path<String>,
) -> Result<Json<()>, Error> {
    requester.require_owner()?;
    if state.minecraft_links.remove(&minecraft_uuid, None).await? {
        Ok(Json(()))
    } else {
        Err(not_linked())
    }
}

pub fn get_minecraft_link_routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/user/minecraft_links",
            get(get_own_links).post(link_account),
        )
        .route("/user/minecraft_links/{minecraft_uuid}", delete(unlink_own))
        .route("/minecraft_links", get(get_all_links))
        .route("/minecraft_links/{minecraft_uuid}", delete(unlink_any))
        .with_state(state)
}

#[cfg(test)]
mod tests;
