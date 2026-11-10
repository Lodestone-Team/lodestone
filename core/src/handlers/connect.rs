//! Sharing an instance with friends through Lodestone Connect (see [`crate::connect`]):
//!
//! - `GET /instance/{uuid}/share`: whether it is shared, the link, who is connected;
//! - `POST /instance/{uuid}/share`: starts sharing; answers once the link is known;
//! - `DELETE /instance/{uuid}/share`: stops sharing; the link stops working for good.
//!
//! All three need the instance's settings permission (the owner has it). The link lets
//! anyone who has it join the server, so only those who may manage the server see it.
//!
//! The friend list (see [`crate::connect::friends`]):
//!
//! - `GET /connect/friends`: the friend certificate's fingerprint, the friends (no
//!   secrets), what's shared with each, and who is connected;
//! - `PUT /connect/friends`: replaces the list, with a higher version.
//!
//! Both need the owner, signed in (not an API key), and a connection that can't be read
//! on the way: loopback, or HTTPS. The list carries the secrets this core shares with
//! each friend.

use std::net::IpAddr;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
    Json, Router,
};
use color_eyre::eyre::eyre;

use super::util::{get_instance, AuthedUser, ClientIp};
use crate::{
    auth::user::UserAction,
    connect::{
        friends::{FriendList, FriendsView},
        InstanceShare,
    },
    error::{Error, ErrorKind},
    tls::{is_loopback, TlsMode},
    types::InstanceUuid,
    AppState,
};

async fn allowed(
    state: &AppState,
    requester: &AuthedUser,
    uuid: &InstanceUuid,
) -> Result<(), Error> {
    requester
        .require(state, UserAction::AccessSetting(uuid.clone()))
        .await?;
    get_instance(state, uuid)?;
    Ok(())
}

async fn get_share(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<InstanceShare>, Error> {
    allowed(&state, &requester, &uuid).await?;
    Ok(Json(state.connect.status(&uuid).await))
}

async fn start_share(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<InstanceShare>, Error> {
    allowed(&state, &requester, &uuid).await?;
    Ok(Json(state.connect.start(&state, &uuid).await?))
}

async fn stop_share(
    State(state): State<AppState>,
    Path(uuid): Path<InstanceUuid>,
    requester: AuthedUser,
) -> Result<Json<InstanceShare>, Error> {
    allowed(&state, &requester, &uuid).await?;
    state.connect.stop(&uuid, true).await;
    Ok(Json(state.connect.status(&uuid).await))
}

/// The owner, signed in, over loopback or HTTPS. Plain HTTP from another device is only
/// served when the core has no certificate or "Allow LAN access over plain HTTP" is on,
/// so outside those cases a request from another device came over HTTPS. A request a
/// reverse proxy on this computer passed on (it says who it forwarded) isn't counted as
/// loopback: it may have come from anywhere, over anything.
///
/// This can't catch every local proxy: one that sends none of [`FORWARDED_HEADERS`],
/// or a raw TCP tunnel (`ssh -L`, a TCP tunnel service), still looks like loopback.
/// launcher/docs/friends.md says so.
/// Headers that reverse proxies and tunnels commonly add to say who they passed a
/// request on from (or that they passed it on at all).
const FORWARDED_HEADERS: &[&str] = &[
    "forwarded",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-forwarded-proto",
    "x-real-ip",
    "x-client-ip",
    "x-cluster-client-ip",
    "x-original-forwarded-for",
    "cf-connecting-ip",
    "cf-connecting-ipv6",
    "true-client-ip",
    "fastly-client-ip",
    "via",
];

async fn friends_allowed(
    state: &AppState,
    requester: &AuthedUser,
    peer: Option<IpAddr>,
    headers: &HeaderMap,
) -> Result<(), Error> {
    requester.require_session()?;
    requester.require_owner()?;
    let forwarded = FORWARDED_HEADERS
        .iter()
        .any(|name| headers.contains_key(*name));
    let loopback = !forwarded && peer.is_some_and(is_loopback);
    let https_only = state.tls_info.mode != TlsMode::None
        && !state.global_settings.lock().await.allow_lan_http();
    if !loopback && !https_only {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!(
                "The friend list can only be changed over HTTPS or from this computer. Turn off \"Allow LAN access over plain HTTP\" in Core Settings."
            ),
        });
    }
    Ok(())
}

async fn get_friends(
    State(state): State<AppState>,
    ClientIp(peer): ClientIp,
    headers: HeaderMap,
    requester: AuthedUser,
) -> Result<Json<FriendsView>, Error> {
    friends_allowed(&state, &requester, peer, &headers).await?;
    Ok(Json(state.connect.friends.view().await?))
}

async fn put_friends(
    State(state): State<AppState>,
    ClientIp(peer): ClientIp,
    headers: HeaderMap,
    requester: AuthedUser,
    Json(list): Json<FriendList>,
) -> Result<Json<FriendsView>, Error> {
    friends_allowed(&state, &requester, peer, &headers).await?;
    Ok(Json(state.connect.friends.replace(&state, list).await?))
}

pub fn get_connect_routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/instance/{uuid}/share",
            get(get_share).post(start_share).delete(stop_share),
        )
        .route("/connect/friends", get(get_friends).put(put_friends))
        .with_state(state)
}
