//! Keeping a core of this person's in step with their friend list, for the app with an
//! owner session to it (Lodestone Host): `GET` and `PUT /connect/friends`.
//!
//! The core gets, for each friend who is active and not hidden, the core secret it
//! shares with them (never the pair secret), and keeps what's shared with each. Friends
//! removed, retired, pending or hidden here are dropped from the core.

use std::collections::HashMap;

use http_client::Method;
use lodestone_connect::friend::crypto::core_secret;
use serde::{Deserialize, Serialize};

use super::{FriendStatus, Friends, crate_unhex32};
use crate::cores::{CoreError, admin::AdminClient};

/// A friend, as `GET /connect/friends` reports it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreFriend {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub shared: Vec<String>,
    #[serde(default)]
    pub connected: bool,
}

/// `GET /connect/friends`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreFriends {
    /// SHA-256 of the core's friend certificate, 64 hex digits.
    pub fingerprint: String,
    pub version: u64,
    #[serde(default)]
    pub friends: Vec<CoreFriend>,
    /// The friend host's status, while it runs.
    #[serde(default)]
    pub host: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct EntryBody {
    key: String,
    name: String,
    secret: String,
    shared: Vec<String>,
}

#[derive(Serialize)]
struct ListBody {
    version: u64,
    friends: Vec<EntryBody>,
}

/// The core's friend list.
pub async fn read(admin: &AdminClient) -> Result<CoreFriends, CoreError> {
    admin.get("connect/friends").await
}

/// Builds the list for a core whose friend certificate is `current.fingerprint`, keeping
/// what it shares with each friend, with `shared` overriding it for one friend.
fn list_for(
    friends: &Friends,
    current: &CoreFriends,
    shared: Option<(&str, Vec<String>)>,
) -> Option<ListBody> {
    let fingerprint = crate_unhex32(&current.fingerprint)?;
    let me = friends.me();
    let kept: HashMap<&str, &Vec<String>> = current
        .friends
        .iter()
        .map(|f| (f.key.as_str(), &f.shared))
        .collect();
    let entries = friends
        .book
        .friends
        .iter()
        .filter(|f| f.status == FriendStatus::Active && !f.hidden)
        .filter_map(|friend| {
            let key = friend.public_key()?;
            let pair = friend.pair_secret()?;
            let shared = match &shared {
                Some((which, list)) if *which == friend.key => list.clone(),
                _ => kept
                    .get(friend.key.as_str())
                    .map(|l| (*l).clone())
                    .unwrap_or_default(),
            };
            Some(EntryBody {
                key: friend.key.clone(),
                name: friend.display_name().to_string(),
                secret: core_secret(&pair, &me, &key, &fingerprint).to_hex(),
                shared,
            })
        })
        .collect();
    Some(ListBody {
        version: current.version + 1,
        friends: entries,
    })
}

/// Puts this person's current friends on the core, keeping what's shared with each, and
/// lists the core in their state (publish it afterwards with
/// [`Friends::publish_states`]). Returns the core's list as it is now.
pub async fn sync(
    friends: &mut Friends,
    admin: &AdminClient,
    core_name: &str,
) -> Result<CoreFriends, SyncError> {
    sync_with(friends, admin, core_name, None).await
}

/// [`sync`], sharing exactly `instances` with the friend `key`.
pub async fn share(
    friends: &mut Friends,
    admin: &AdminClient,
    core_name: &str,
    key: &str,
    instances: Vec<String>,
) -> Result<CoreFriends, SyncError> {
    sync_with(friends, admin, core_name, Some((key, instances))).await
}

/// Whether `admin` talks to a core on this computer. Only such a core is synced: the
/// friend certificate's fingerprint comes from the core itself, and a core elsewhere
/// that lied about it could be handed the secrets meant for another of this person's
/// cores. A core this person runs here (Lodestone Host's) has no one to lie for.
fn is_local(admin: &AdminClient) -> bool {
    match admin.core().base().host() {
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        Some(url::Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        None => false,
    }
}

async fn sync_with(
    friends: &mut Friends,
    admin: &AdminClient,
    core_name: &str,
    shared: Option<(&str, Vec<String>)>,
) -> Result<CoreFriends, SyncError> {
    if !is_local(admin) {
        return Err(SyncError::NotLocal);
    }
    let current = read(admin).await?;
    friends.add_my_core(&current.fingerprint, core_name)?;
    let body = list_for(friends, &current, shared)
        .ok_or_else(|| SyncError::Core(CoreError::Invalid("the core's fingerprint".into())))?;
    let updated: CoreFriends = admin
        .json(Method::PUT, "connect/friends", Some(body))
        .await?;
    Ok(updated)
}

/// Why syncing a core failed.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error(transparent)]
    Friends(#[from] super::FriendsError),
    #[error("Only a core on this computer can share with friends for now.")]
    NotLocal,
}
