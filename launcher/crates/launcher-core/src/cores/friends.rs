//! Friends' servers: servers shared with Lodestone Connect that the user joined, kept
//! in `<data>/friends.json` so Play joins them again later.
//!
//! The file holds what the Servers page shows (the server's name, version and loader,
//! who shares it) and whether the user trusts it for mods. The invite link itself is a
//! secret (its token lets anyone join), so it is kept in the [`SecretStore`] (the
//! Keychain) under [`invite_secret_name`], never in the file.
//!
//! A friend's server is known by [`friend_id`]: `connect:<service>/<code>`. Instances
//! made for it are linked to that id (`cores::link`), as instances made for a server on
//! a core are linked to the core's id.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    connect::{Invite, SharedServer, loader},
    link::ServerLink,
    matching_loader,
    models::ServerLoader,
};
use crate::{
    Secret, SecretStore,
    instances::{Instance, Loader},
    storage::{self, Document, Migration, StorageError},
};

/// The start of every friend's server's id.
pub const ID_PREFIX: &str = "connect:";

/// The id of the server an invite is for: `connect:connect.lodestone.cc/ABCDEFGH23`.
pub fn friend_id(invite: &Invite) -> String {
    format!("{ID_PREFIX}{}/{}", invite.service.link(), invite.code)
}

/// Whether `id` (a core id in a server link, say) is a friend's server.
pub fn is_friend_id(id: &str) -> bool {
    id.starts_with(ID_PREFIX)
}

/// The invite link without its secret part, to show where a server is shared from.
pub fn public_link(invite: &Invite) -> String {
    format!("{}/j/{}", invite.service.base_url(), invite.code)
}

/// A friend's server the user joined.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FriendServer {
    /// [`friend_id`].
    pub id: String,
    /// The invite link without its secret part ([`public_link`]).
    pub link: String,
    /// The SHA-256 of the host's certificate, in hex, from the invite.
    pub fingerprint: String,
    /// What the host said when last joined.
    pub name: String,
    pub core_name: String,
    pub game_version: String,
    pub loader: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
    #[serde(default)]
    pub modpack: bool,
    /// The launcher instance last played on it, when it isn't one linked to it (a
    /// server without mods plays on any instance that matches).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
    /// Unix seconds.
    #[serde(default)]
    pub added_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_joined: Option<i64>,
    /// The fingerprint the user trusted this server's mods under, if they did. Trust
    /// ends when the host's certificate changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trusted_as: Option<String>,
}

impl FriendServer {
    /// A new entry for `invite`, with what the host said.
    pub fn new(invite: &Invite, server: &SharedServer, now: i64) -> Self {
        let mut entry = Self {
            id: friend_id(invite),
            link: public_link(invite),
            added_at: now,
            ..Self::default()
        };
        entry.update(invite, server, now);
        entry
    }

    /// Takes what the host says now. A new certificate ends the trust.
    pub fn update(&mut self, invite: &Invite, server: &SharedServer, now: i64) {
        let fingerprint = hex(&invite.fingerprint);
        if self.fingerprint != fingerprint {
            self.trusted_as = None;
        }
        self.fingerprint = fingerprint;
        self.link = public_link(invite);
        self.name = server.name.clone();
        self.core_name = server.core_name.clone();
        self.game_version = server.game_version.clone();
        self.loader = server.loader.clone();
        self.loader_version = server.loader_version.clone().filter(|v| !v.is_empty());
        self.modpack = server.modpack;
        self.last_joined = Some(now);
    }

    /// Whether the user trusts this server for mods, under its current certificate.
    pub fn is_trusted(&self) -> bool {
        self.trusted_as
            .as_deref()
            .is_some_and(|trusted| !trusted.is_empty() && trusted == self.fingerprint)
    }

    pub fn set_trusted(&mut self, trusted: bool) {
        self.trusted_as = trusted.then(|| self.fingerprint.clone());
    }

    /// What the host said last, as a [`SharedServer`] (without the live parts).
    pub fn shared(&self) -> SharedServer {
        SharedServer {
            core_name: self.core_name.clone(),
            core_uuid: String::new(),
            instance_uuid: String::new(),
            name: self.name.clone(),
            game_version: self.game_version.clone(),
            loader: self.loader.clone(),
            loader_version: self.loader_version.clone(),
            modpack: self.modpack,
            state: None,
            player_count: None,
            max_player_count: None,
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FriendsFile {
    #[serde(default)]
    pub friends: Vec<FriendServer>,
}

impl Document for FriendsFile {
    const VERSION: u32 = 1;
    const MIGRATIONS: &'static [Migration] = &[Ok];
}

impl FriendsFile {
    /// Adds the server, or updates the entry with the same id. Returns its index.
    pub fn upsert(&mut self, invite: &Invite, server: &SharedServer, now: i64) -> usize {
        let id = friend_id(invite);
        match self.friends.iter().position(|f| f.id == id) {
            Some(index) => {
                self.friends[index].update(invite, server, now);
                index
            }
            None => {
                self.friends.push(FriendServer::new(invite, server, now));
                self.friends.len() - 1
            }
        }
    }

    pub fn get(&self, id: &str) -> Option<&FriendServer> {
        self.friends.iter().find(|f| f.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut FriendServer> {
        self.friends.iter_mut().find(|f| f.id == id)
    }

    /// Takes the entry out; returns it.
    pub fn remove(&mut self, id: &str) -> Option<FriendServer> {
        let index = self.friends.iter().position(|f| f.id == id)?;
        Some(self.friends.remove(index))
    }
}

/// `friends.json` in the data directory.
pub fn friends_file(data_root: &Path) -> PathBuf {
    data_root.join("friends.json")
}

pub fn load(path: &Path) -> Result<FriendsFile, StorageError> {
    storage::load(path).map(|loaded| loaded.document)
}

pub fn save(path: &Path, file: &FriendsFile) -> Result<(), StorageError> {
    storage::save(path, file)
}

/// The name of a friend's invite link in the [`SecretStore`].
pub fn invite_secret_name(id: &str) -> String {
    format!("lodestone-connect-invite:{id}")
}

pub async fn read_invite(store: &dyn SecretStore, id: &str) -> anyhow::Result<Option<Invite>> {
    let Some(secret) = store.read(&invite_secret_name(id)).await? else {
        return Ok(None);
    };
    Ok(Invite::parse(secret.expose()).ok())
}

pub async fn write_invite(store: &dyn SecretStore, invite: &Invite) -> anyhow::Result<()> {
    store
        .write(
            &invite_secret_name(&friend_id(invite)),
            &Secret::new(invite.link()),
        )
        .await
}

pub async fn delete_invite(store: &dyn SecretStore, id: &str) -> anyhow::Result<()> {
    store.delete(&invite_secret_name(id)).await
}

/// Which launcher instance plays a friend's server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstanceMatch {
    /// An instance made for this server (linked to it): Play keeps its mods in step.
    Linked(String),
    /// An instance of the user's with the same Minecraft version and loader: for a
    /// server without mods.
    Matching(String),
    /// None fits: make one (with the server's mods, when it has a client pack).
    Create,
}

impl InstanceMatch {
    pub fn instance_id(&self) -> Option<&str> {
        match self {
            InstanceMatch::Linked(id) | InstanceMatch::Matching(id) => Some(id),
            InstanceMatch::Create => None,
        }
    }
}

/// Whether `instance` can join `server` as it is: the same Minecraft version, and the
/// same loader at the same version (none for vanilla, Paper and Spigot servers).
pub fn instance_fits(instance: &Instance, server: &SharedServer) -> bool {
    if instance.game_version != server.game_version {
        return false;
    }
    let as_core_server = super::connect::server_instance(server);
    let Ok(wanted) = matching_loader(&as_core_server) else {
        return false;
    };
    match wanted {
        None => instance.loader == Loader::None,
        Some(choice) => {
            Loader::from_kind(Some(choice.kind)) == instance.loader
                && instance.loader_version.as_deref() == Some(choice.version.as_str())
        }
    }
}

/// The instance to play `server` (the friend's server `id`) on:
///
/// 1. one linked to it (made for it);
/// 2. for a server without mods: the instance last played on it, while it still fits,
///    else the most recently played instance that fits and isn't linked to another
///    server (its mods stay the user's);
/// 3. otherwise a new one.
///
/// A server with mods only plays on an instance made for it, since Play replaces the
/// instance's mods with the server's.
pub fn match_instance(
    id: &str,
    server: &SharedServer,
    remembered: Option<&str>,
    instances: &[Instance],
    links: &[(String, ServerLink)],
) -> InstanceMatch {
    let exists = |instance_id: &str| instances.iter().any(|i| i.id == instance_id);
    if let Some((instance_id, _)) = links
        .iter()
        .find(|(instance_id, link)| link.core_uuid == id && exists(instance_id))
    {
        return InstanceMatch::Linked(instance_id.clone());
    }
    if server.modpack || loader(&server.loader) == ServerLoader::Other {
        return InstanceMatch::Create;
    }
    let linked_elsewhere =
        |instance_id: &str| links.iter().any(|(linked, _)| linked == instance_id);
    let fits =
        |instance: &&Instance| instance_fits(instance, server) && !linked_elsewhere(&instance.id);
    if let Some(instance) = remembered
        .and_then(|remembered| instances.iter().find(|i| i.id == remembered))
        .filter(fits)
    {
        return InstanceMatch::Matching(instance.id.clone());
    }
    instances
        .iter()
        .filter(fits)
        .max_by_key(|i| (i.last_played.unwrap_or(0), i.created))
        .map_or(InstanceMatch::Create, |i| {
            InstanceMatch::Matching(i.id.clone())
        })
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;

    use super::*;
    use crate::MemorySecretStore;

    fn invite(fingerprint: u8) -> Invite {
        let text = format!(
            "https://connect.lodestone.cc/j/ABCDEFGH23#{}.{}",
            format!("{fingerprint:02x}").repeat(32),
            "01".repeat(16)
        );
        Invite::parse(&text).unwrap()
    }

    fn shared(loader: &str, version: Option<&str>, modpack: bool) -> SharedServer {
        SharedServer {
            core_name: "Alex's core".into(),
            core_uuid: "core".into(),
            instance_uuid: "server".into(),
            name: "SMP".into(),
            game_version: "1.21.4".into(),
            loader: loader.into(),
            loader_version: version.map(str::to_string),
            modpack,
            state: Some("Running".into()),
            player_count: Some(1),
            max_player_count: Some(10),
        }
    }

    fn instance(id: &str, version: &str, loader: Loader, loader_version: Option<&str>) -> Instance {
        Instance {
            id: id.into(),
            name: id.into(),
            game_version: version.into(),
            loader,
            loader_version: loader_version.map(str::to_string),
            created: 1,
            ..Instance::default()
        }
    }

    fn link(core: &str) -> ServerLink {
        ServerLink {
            core_uuid: core.into(),
            core_url: "https://connect.lodestone.cc/j/ABCDEFGH23".into(),
            server_uuid: "server".into(),
            server_name: "SMP".into(),
            pack_etag: None,
        }
    }

    #[test]
    fn ids_and_links_leave_out_the_secret() {
        let invite = invite(0xab);
        let id = friend_id(&invite);
        assert_eq!(id, "connect:connect.lodestone.cc/ABCDEFGH23");
        assert!(is_friend_id(&id));
        assert!(!is_friend_id("6f1c-core-uuid"));
        assert_eq!(
            public_link(&invite),
            "https://connect.lodestone.cc/j/ABCDEFGH23"
        );
        let entry = FriendServer::new(&invite, &shared("vanilla", None, false), 5);
        let text = serde_json::to_string(&entry).unwrap();
        assert!(!text.contains(&"01".repeat(16)), "{text}");
        assert_eq!(entry.fingerprint, "ab".repeat(32));
        assert_eq!(entry.added_at, 5);
        assert_eq!(entry.last_joined, Some(5));
    }

    #[test]
    fn the_store_round_trips_and_keeps_the_link_in_the_secret_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = friends_file(dir.path());
        assert_eq!(load(&path).unwrap(), FriendsFile::default());
        let mut file = FriendsFile::default();
        let invite = invite(0xab);
        assert_eq!(file.upsert(&invite, &shared("vanilla", None, false), 1), 0);
        // joining again updates the same entry
        let mut newer = shared("fabric", Some("0.16.14"), true);
        newer.name = "SMP 2".into();
        assert_eq!(file.upsert(&invite, &newer, 2), 0);
        assert_eq!(file.friends.len(), 1);
        assert_eq!(file.friends[0].name, "SMP 2");
        assert_eq!(file.friends[0].added_at, 1);
        assert_eq!(file.friends[0].last_joined, Some(2));
        save(&path, &file).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains(&"01".repeat(16)), "{text}");
        assert_eq!(load(&path).unwrap(), file);

        let store = MemorySecretStore::new();
        block_on(async {
            write_invite(&store, &invite).await.unwrap();
            let id = friend_id(&invite);
            assert_eq!(store.keys(), [invite_secret_name(&id)]);
            assert_eq!(
                read_invite(&store, &id).await.unwrap(),
                Some(invite.clone())
            );
            delete_invite(&store, &id).await.unwrap();
            assert_eq!(read_invite(&store, &id).await.unwrap(), None);
        });
        assert!(file.remove(&friend_id(&invite)).is_some());
        assert!(file.friends.is_empty());
    }

    #[test]
    fn trust_ends_with_a_new_certificate() {
        let server = shared("fabric", Some("0.16.14"), true);
        let mut entry = FriendServer::new(&invite(0xab), &server, 1);
        assert!(!entry.is_trusted());
        entry.set_trusted(true);
        assert!(entry.is_trusted());
        entry.update(&invite(0xab), &server, 2);
        assert!(entry.is_trusted());
        entry.update(&invite(0xcd), &server, 3);
        assert!(!entry.is_trusted());
        entry.set_trusted(false);
        assert_eq!(entry.trusted_as, None);
    }

    #[test]
    fn instances_that_fit() {
        let vanilla = shared("vanilla", None, false);
        assert!(instance_fits(
            &instance("a", "1.21.4", Loader::None, None),
            &vanilla
        ));
        assert!(!instance_fits(
            &instance("a", "1.21.3", Loader::None, None),
            &vanilla
        ));
        assert!(!instance_fits(
            &instance("a", "1.21.4", Loader::Fabric, Some("0.16.14")),
            &vanilla
        ));
        // Paper and Spigot servers take a vanilla game
        assert!(instance_fits(
            &instance("a", "1.21.4", Loader::None, None),
            &shared("paper", Some("196"), false)
        ));
        let fabric = shared("fabric", Some("0.16.14"), false);
        assert!(instance_fits(
            &instance("a", "1.21.4", Loader::Fabric, Some("0.16.14")),
            &fabric
        ));
        assert!(!instance_fits(
            &instance("a", "1.21.4", Loader::Fabric, Some("0.16.10")),
            &fabric
        ));
        assert!(!instance_fits(
            &instance("a", "1.21.4", Loader::Quilt, Some("0.16.14")),
            &fabric
        ));
        // a Forge server's short version is the full Maven version of the instance
        let mut forge = shared("forge", Some("54.0.6"), false);
        forge.game_version = "1.21.4".into();
        assert!(instance_fits(
            &instance("a", "1.21.4", Loader::Forge, Some("1.21.4-54.0.6")),
            &forge
        ));
        // a Fabric server that doesn't say its loader's version fits nothing
        assert!(!instance_fits(
            &instance("a", "1.21.4", Loader::Fabric, Some("0.16.14")),
            &shared("fabric", None, false)
        ));
    }

    #[test]
    fn matching_prefers_linked_then_remembered_then_recent() {
        let id = "connect:connect.lodestone.cc/ABCDEFGH23";
        let vanilla = shared("vanilla", None, false);
        let mut old = instance("old", "1.21.4", Loader::None, None);
        old.last_played = Some(10);
        let mut recent = instance("recent", "1.21.4", Loader::None, None);
        recent.last_played = Some(20);
        let mut other_server = instance("other-server", "1.21.4", Loader::None, None);
        other_server.last_played = Some(30);
        let modded = instance("modded", "1.21.4", Loader::Fabric, Some("0.16.14"));
        let made_for_it = instance("made-for-it", "1.20.1", Loader::Fabric, Some("0.15.0"));
        let instances = vec![
            old.clone(),
            recent.clone(),
            other_server.clone(),
            modded,
            made_for_it,
        ];
        let links = vec![("other-server".to_string(), link("another-core"))];

        // the most recently played that fits, never one linked to another server
        assert_eq!(
            match_instance(id, &vanilla, None, &instances, &links),
            InstanceMatch::Matching("recent".into())
        );
        // the one played last time, while it fits
        assert_eq!(
            match_instance(id, &vanilla, Some("old"), &instances, &links),
            InstanceMatch::Matching("old".into())
        );
        assert_eq!(
            match_instance(id, &vanilla, Some("modded"), &instances, &links),
            InstanceMatch::Matching("recent".into())
        );
        // one made for this server wins, whatever its version (Play brings it in step)
        let mut linked = links.clone();
        linked.push(("made-for-it".to_string(), link(id)));
        assert_eq!(
            match_instance(id, &vanilla, Some("old"), &instances, &linked),
            InstanceMatch::Linked("made-for-it".into())
        );
        // a link to an instance that is gone doesn't count
        let mut gone = links.clone();
        gone.push(("deleted".to_string(), link(id)));
        assert_eq!(
            match_instance(id, &vanilla, None, &instances, &gone),
            InstanceMatch::Matching("recent".into())
        );
        // a server with mods needs an instance made for it
        assert_eq!(
            match_instance(
                id,
                &shared("fabric", Some("0.16.14"), true),
                None,
                &instances,
                &links
            ),
            InstanceMatch::Create
        );
        // nothing fits
        assert_eq!(
            match_instance(id, &shared("vanilla", None, false), None, &[], &[]),
            InstanceMatch::Create
        );
        assert_eq!(
            match_instance(id, &shared("other", None, false), None, &instances, &links),
            InstanceMatch::Create
        );
    }
}
