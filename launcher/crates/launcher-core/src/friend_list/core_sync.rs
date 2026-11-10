//! Keeping a core of this person's in step with their friend list, for the app with an
//! owner session to it (Lodestone Host): `GET` and `PUT /connect/friends`.
//!
//! The core gets, for each friend who is active, the core secret it shares with them
//! (never the pair secret), and keeps what's shared with each. A hidden friend is sent
//! with `hidden: true`: the core keeps what's shared with them but publishes nothing
//! for them and serves them nothing, so showing them again gives it all back. Friends
//! removed, retired or pending here are dropped from the core.

use std::collections::HashMap;

use http_client::Method;
use lodestone_connect::friend::crypto::core_secret;
use serde::{Deserialize, Serialize};

use super::{FriendStatus, Friends, unhex32};
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
    /// Hidden: the core keeps `shared`, but serves them nothing.
    #[serde(default)]
    pub hidden: bool,
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

#[derive(Debug, Serialize)]
struct EntryBody {
    key: String,
    name: String,
    secret: String,
    shared: Vec<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    hidden: bool,
}

#[derive(Debug, Serialize)]
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
    let fingerprint = unhex32(&current.fingerprint)?;
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
        .filter(|f| f.status == FriendStatus::Active)
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
                hidden: friend.hidden,
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
/// [`Friends::publish_states`]). `source` names which core this is (see
/// [`super::MyCore::source`]): when it comes back with another friend certificate, the
/// old one leaves the state. Returns the core's list as it is now.
pub async fn sync(
    friends: &mut Friends,
    admin: &AdminClient,
    core_name: &str,
    source: Option<&str>,
) -> Result<CoreFriends, SyncError> {
    sync_with(friends, admin, core_name, source, None).await
}

/// [`sync`], sharing exactly `instances` with the friend `key`.
pub async fn share(
    friends: &mut Friends,
    admin: &AdminClient,
    core_name: &str,
    source: Option<&str>,
    key: &str,
    instances: Vec<String>,
) -> Result<CoreFriends, SyncError> {
    sync_with(friends, admin, core_name, source, Some((key, instances))).await
}

/// Empties the core's friend list: for a friend identity that was retired, whose
/// friendships are over on both sides. Only ever on purpose: a friend file that's
/// missing never empties a core.
pub async fn clear(admin: &AdminClient) -> Result<CoreFriends, SyncError> {
    if !is_local(admin) {
        return Err(SyncError::NotLocal);
    }
    let current = read(admin).await?;
    if current.friends.is_empty() {
        return Ok(current);
    }
    let body = ListBody {
        version: current.version + 1,
        friends: Vec::new(),
    };
    Ok(admin
        .json(Method::PUT, "connect/friends", Some(body))
        .await?)
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
    source: Option<&str>,
    shared: Option<(&str, Vec<String>)>,
) -> Result<CoreFriends, SyncError> {
    if !is_local(admin) {
        return Err(SyncError::NotLocal);
    }
    let current = read(admin).await?;
    friends.add_my_core(&current.fingerprint, core_name, source)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MemorySecretStore, friend_list::Friend, paths::DataDir};

    #[test]
    fn hidden_friends_go_to_the_core_with_their_shares() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemorySecretStore::new();
        let mut friends = futures::executor::block_on(Friends::create(
            &DataDir::at(dir.path()),
            &store,
            lodestone_connect::Service::parse("http://127.0.0.1:9").unwrap(),
        ))
        .unwrap();
        let key = || {
            lodestone_connect::friend::PersonKey::generate()
                .unwrap()
                .public()
                .to_hex()
        };
        let (alex, sam, kim) = (key(), key(), key());
        friends
            .modify(|book| {
                for (k, hidden, status) in [
                    (&alex, false, FriendStatus::Active),
                    (&sam, true, FriendStatus::Active),
                    (&kim, false, FriendStatus::Pending),
                ] {
                    book.friends.push(Friend {
                        key: k.clone(),
                        name: "x".into(),
                        pair: "cd".repeat(32),
                        status,
                        hidden,
                        ..Friend::default()
                    });
                }
                Ok(())
            })
            .unwrap();
        let current = CoreFriends {
            fingerprint: "11".repeat(32),
            version: 4,
            friends: vec![
                CoreFriend {
                    key: alex.clone(),
                    shared: vec!["one".into()],
                    ..CoreFriend::default()
                },
                CoreFriend {
                    key: sam.clone(),
                    shared: vec!["two".into()],
                    ..CoreFriend::default()
                },
            ],
            host: None,
        };
        let body = list_for(&friends, &current, None).unwrap();
        assert_eq!(body.version, 5);
        let entry = |k: &str| body.friends.iter().find(|e| e.key == k);
        assert_eq!(entry(&alex).unwrap().shared, ["one"]);
        assert!(!entry(&alex).unwrap().hidden);
        // hidden: still on the core, shares kept, marked hidden
        let hidden = entry(&sam).unwrap();
        assert!(hidden.hidden);
        assert_eq!(hidden.shared, ["two"]);
        assert!(entry(&kim).is_none(), "pending friends get nothing");
        let json = serde_json::to_value(&body).unwrap();
        assert!(
            json["friends"][0].get("hidden").is_none(),
            "only when hidden"
        );
    }
}
