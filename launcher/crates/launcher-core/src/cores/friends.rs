//! Friends' servers: servers shared with Lodestone Connect that the user joined, kept
//! in `<data>/friends.json` so Play joins them again later.
//!
//! The file holds what the Servers page shows (the server's name, version and loader,
//! who shares it) and whether the user trusts it for mods. The invite link itself is a
//! secret (its token lets anyone join), so it is kept in the [`SecretStore`] (the
//! Keychain) under [`invite_secret_name`], never in the file.
//!
//! A friend's server is known by [`friend_id`] of the first link it was joined with:
//! `connect:<service>/<code>`. Instances made for it are linked to that id
//! (`cores::link`), as instances made for a server on a core are linked to the core's
//! id. A new link for the same server (the same instance of the same core, as the host
//! says) replaces the old link and keeps the id, so the server stays one entry and its
//! instances stay linked ([`FriendsFile::upsert`]).
//!
//! A server joined from the friend list (`crate::friend_list`), without a link, is
//! known by [`friend_list_id`]: `friend:<core fingerprint>/<instance uuid>`. It keeps
//! the friend's key ([`FriendServer::friend_key`]) and the core's fingerprint, which
//! its mod trust is tied to; nothing about it goes to the Keychain.
//!
//! Each server keeps its own game port ([`FriendServer::game_port`]), picked on the
//! first join from its id, so Minecraft finds it at the same `127.0.0.1:<port>` every
//! session ([`FriendsFile::game_port`]).

use std::{
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// The friend list's service and links, for the apps (they don't depend on
/// `lodestone_connect` themselves).
pub use lodestone_connect::{Service, friend::pair::FriendLink};

use super::{
    connect::{GamePort, Invite, SharedServer, loader},
    link::ServerLink,
    matching_loader,
    models::ServerLoader,
    server_list::{self, Listed},
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

/// The start of the id of a server joined from the friend list.
pub const FRIEND_LIST_PREFIX: &str = "friend:";

/// The id of the server `instance` on a friend's core whose friend certificate has
/// `fingerprint` (hex): `friend:<fingerprint>/<instance>`.
pub fn friend_list_id(fingerprint: &str, instance: &str) -> String {
    format!("{FRIEND_LIST_PREFIX}{}/{instance}", fingerprint.to_ascii_lowercase())
}

/// The core fingerprint (hex) and instance of a [`friend_list_id`].
pub fn parse_friend_list_id(id: &str) -> Option<(&str, &str)> {
    let rest = id.strip_prefix(FRIEND_LIST_PREFIX)?;
    let (fingerprint, instance) = rest.split_once('/')?;
    (fingerprint.len() == 64 && !instance.is_empty()).then_some((fingerprint, instance))
}

/// Whether `id` is a server joined from the friend list.
pub fn is_friend_list_id(id: &str) -> bool {
    id.starts_with(FRIEND_LIST_PREFIX)
}

/// Whether `id` (a core id in a server link, say) is a friend's server: one joined
/// with an invite link, or from the friend list.
pub fn is_friend_id(id: &str) -> bool {
    id.starts_with(ID_PREFIX) || is_friend_list_id(id)
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
    /// The host core's id and the shared instance's, as the host said: which server
    /// this is, whatever the link. Empty in entries saved before they were kept.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub core_uuid: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub instance_uuid: String,
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
    /// For a server joined from the friend list: the friend's key (64 hex digits).
    /// Empty for one joined with an invite link.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub friend_key: String,
    /// For a server joined from the friend list: the client pack (its ETag) the user
    /// trusted its mods with. Trust holds for that pack only: a changed pack is asked
    /// about again (launcher/docs/friends.md, "Mods").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trusted_pack: Option<String>,
    /// The local game port Minecraft joins this server on (`127.0.0.1:<port>`), the
    /// same every session. Picked on the first join ([`port_choices`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_port: Option<u16>,
    /// The entries the launcher added to instances' Minecraft server lists for it
    /// ([`super::server_list`]), one per instance.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub listed: Vec<Listed>,
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

    /// A new entry for a server joined from the friend list: `server` on the core of
    /// `friend_key` whose friend certificate has `fingerprint` (hex). Its mods start
    /// untrusted.
    pub fn from_friend_list(
        friend_key: &str,
        fingerprint: &str,
        server: &SharedServer,
        now: i64,
    ) -> Self {
        let fingerprint = fingerprint.to_ascii_lowercase();
        let mut entry = Self {
            id: friend_list_id(&fingerprint, &server.instance_uuid),
            fingerprint,
            added_at: now,
            ..Self::default()
        };
        entry.update_from_friend_list(friend_key, server, now);
        entry
    }

    /// Takes what a friend's core says now, for a server joined from the friend list.
    /// Its id holds the core's fingerprint, so the certificate can't change under it.
    pub fn update_from_friend_list(&mut self, friend_key: &str, server: &SharedServer, now: i64) {
        if !self.friend_key.is_empty() && self.friend_key != friend_key {
            // another friend lists this core now: the trust was given to the first
            self.set_trusted(false);
        }
        self.friend_key = friend_key.to_string();
        // shown where the server came from, as an invite's public link is
        self.link = server.core_name.clone();
        self.core_uuid = server.core_uuid.clone();
        self.instance_uuid = server.instance_uuid.clone();
        self.name = server.name.clone();
        self.core_name = server.core_name.clone();
        self.game_version = server.game_version.clone();
        self.loader = server.loader.clone();
        self.loader_version = server.loader_version.clone().filter(|v| !v.is_empty());
        self.modpack = server.modpack;
        self.last_joined = Some(now);
    }

    /// Whether this server was joined from the friend list.
    pub fn is_friend_list(&self) -> bool {
        is_friend_list_id(&self.id)
    }

    /// Takes what the host says now. A new certificate ends the trust.
    pub fn update(&mut self, invite: &Invite, server: &SharedServer, now: i64) {
        let fingerprint = hex(&invite.fingerprint);
        if self.fingerprint != fingerprint {
            self.trusted_as = None;
        }
        self.fingerprint = fingerprint;
        self.link = public_link(invite);
        if !server.core_uuid.is_empty() && !server.instance_uuid.is_empty() {
            self.core_uuid = server.core_uuid.clone();
            self.instance_uuid = server.instance_uuid.clone();
        }
        self.name = server.name.clone();
        self.core_name = server.core_name.clone();
        self.game_version = server.game_version.clone();
        self.loader = server.loader.clone();
        self.loader_version = server.loader_version.clone().filter(|v| !v.is_empty());
        self.modpack = server.modpack;
        self.last_joined = Some(now);
    }

    /// Whether this is `server`, whose host's certificate has `fingerprint` (hex): the
    /// same instance of the same core. An entry saved before the launcher kept those
    /// is the server whose certificate it has.
    pub fn is_server(&self, fingerprint: &str, server: &SharedServer) -> bool {
        if !self.core_uuid.is_empty() && !server.core_uuid.is_empty() {
            return self.core_uuid == server.core_uuid
                && self.instance_uuid == server.instance_uuid;
        }
        !self.fingerprint.is_empty() && self.fingerprint == fingerprint
    }

    /// Whether the user trusts this server for mods, under its current certificate.
    pub fn is_trusted(&self) -> bool {
        self.trusted_as
            .as_deref()
            .is_some_and(|trusted| !trusted.is_empty() && trusted == self.fingerprint)
    }

    pub fn set_trusted(&mut self, trusted: bool) {
        self.trusted_as = trusted.then(|| self.fingerprint.clone());
        if !trusted {
            self.trusted_pack = None;
        }
    }

    /// Whether the user trusts this server's mods for the client pack with `etag`: as
    /// [`FriendServer::is_trusted`], and for a server from the friend list, only for
    /// the very pack trusted (a pack without an ETag is never trusted).
    pub fn trusts_pack(&self, etag: Option<&str>) -> bool {
        if !self.is_trusted() {
            return false;
        }
        if !self.is_friend_list() {
            return true;
        }
        matches!((etag, self.trusted_pack.as_deref()), (Some(etag), Some(trusted)) if !etag.is_empty() && etag == trusted)
    }

    /// Trusts this server's mods, with the client pack `etag` for one from the friend
    /// list.
    pub fn trust_pack(&mut self, etag: Option<&str>) {
        self.set_trusted(true);
        self.trusted_pack = etag.filter(|e| !e.is_empty()).map(str::to_string);
    }

    /// The entry the launcher added to `instance_id`'s server list.
    pub fn listed_in(&self, instance_id: &str) -> Option<&Listed> {
        self.listed.iter().find(|l| l.instance_id == instance_id)
    }

    /// Adds this server to the Minecraft server list of the instance `instance_id`
    /// (its game folder `game_dir`) under its name at its usual address, or brings the
    /// entry up to date. Never with a port used for one session only: without a saved
    /// port, nothing is listed. Nothing is written while the instance's game `runs`,
    /// since the game rewrites the list; the next join does it.
    pub fn list_in_game(
        &mut self,
        instance_id: &str,
        game_dir: &Path,
        runs: bool,
    ) -> std::io::Result<()> {
        let Some(port) = self.game_port.filter(|_| !runs) else {
            return Ok(());
        };
        let previous = self.listed_in(instance_id);
        let address = game_address(port);
        let listed =
            server_list::list_server(game_dir, previous, instance_id, &self.name, &address)?;
        if let Some(listed) = listed {
            self.listed.retain(|l| l.instance_id != instance_id);
            self.listed.push(listed);
        }
        Ok(())
    }

    /// Takes this server out of the server lists it was added to, except those of
    /// instances whose game runs (`runs`); `game_dir` finds an instance's game folder.
    /// Returns the instances whose list couldn't be read or written.
    pub fn unlist_in_games(
        &self,
        game_dir: impl Fn(&str) -> PathBuf,
        runs: impl Fn(&str) -> bool,
    ) -> Vec<(String, std::io::Error)> {
        self.listed
            .iter()
            .filter(|listed| !runs(&listed.instance_id))
            .filter_map(|listed| {
                server_list::unlist_server(&game_dir(&listed.instance_id), listed)
                    .err()
                    .map(|err| (listed.instance_id.clone(), err))
            })
            .collect()
    }

    /// Notes the game port a join used, which asked for `wanted`: a first join keeps
    /// the port it picked. Returns the saved port when it was taken, so this join used
    /// another one (the saved port stays for next time).
    pub fn joined_on_port(&mut self, wanted: &GamePort, used: u16) -> Option<u16> {
        match (self.game_port, wanted) {
            (Some(saved), GamePort::Saved(asked)) if *asked == saved && used != saved => {
                Some(saved)
            }
            (None, GamePort::Choose(ports)) if ports.contains(&used) => {
                self.game_port = Some(used);
                None
            }
            _ => None,
        }
    }

    /// What the host said last, as a [`SharedServer`] (without the live parts).
    pub fn shared(&self) -> SharedServer {
        SharedServer {
            core_name: self.core_name.clone(),
            core_uuid: self.core_uuid.clone(),
            instance_uuid: self.instance_uuid.clone(),
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

/// Where Minecraft joins a friend's server whose game port is `port`.
pub fn game_address(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

/// Where game ports are picked: unprivileged, and away from Minecraft's 25565 and the
/// core's 16662.
pub const GAME_PORTS: RangeInclusive<u16> = 30000..=39999;

/// How many ports a first join tries before it takes any free port.
const PORT_TRIES: usize = 8;

/// The game ports a first join of the server `id` tries, in order: one picked from the
/// id (always the same for it), then the ports after it, skipping `taken` (other saved
/// servers' ports).
pub fn port_choices(id: &str, taken: &[u16]) -> Vec<u16> {
    let (start, end) = (*GAME_PORTS.start(), *GAME_PORTS.end());
    let size = u32::from(end - start) + 1;
    let first = fnv1a(id.as_bytes()) % size;
    (0..size)
        .map(|step| start + ((first + step) % size) as u16)
        .filter(|port| !taken.contains(port))
        .take(PORT_TRIES)
        .collect()
}

/// FNV-1a: a small hash that stays the same in every build and on every computer.
fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c_9dc5, |hash, &byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
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
    /// Adds the server, or updates its entry: the one joined with this link, else the
    /// one of the same server joined with another link (which then takes this link,
    /// keeping its id). Returns its index. Keep the link under the entry's id
    /// ([`write_invite`]).
    pub fn upsert(&mut self, invite: &Invite, server: &SharedServer, now: i64) -> usize {
        let fingerprint = hex(&invite.fingerprint);
        let found = position_of(&self.friends, invite).or_else(|| {
            self.friends
                .iter()
                .position(|f| f.is_server(&fingerprint, server))
        });
        match found {
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

    /// Adds a server joined from the friend list, or updates its entry. Returns its
    /// index.
    pub fn upsert_friend_list(
        &mut self,
        friend_key: &str,
        fingerprint: &str,
        server: &SharedServer,
        now: i64,
    ) -> usize {
        let id = friend_list_id(fingerprint, &server.instance_uuid);
        match self.friends.iter().position(|f| f.id == id) {
            Some(index) => {
                self.friends[index].update_from_friend_list(friend_key, server, now);
                index
            }
            None => {
                self.friends.push(FriendServer::from_friend_list(
                    friend_key,
                    fingerprint,
                    server,
                    now,
                ));
                self.friends.len() - 1
            }
        }
    }

    /// Stops trusting the mods of every server joined from the friend list on the core
    /// with `fingerprint` (hex): its owner retired it, or the friend was removed.
    /// Returns whether any trust ended.
    pub fn clear_core_trust(&mut self, fingerprint: &str) -> bool {
        let fingerprint = fingerprint.to_ascii_lowercase();
        let mut changed = false;
        for entry in &mut self.friends {
            let on_core =
                parse_friend_list_id(&entry.id).is_some_and(|(fp, _)| fp == fingerprint);
            if on_core && entry.trusted_as.is_some() {
                entry.set_trusted(false);
                changed = true;
            }
        }
        changed
    }

    /// Stops trusting the mods of every server joined from the friend list of the
    /// friend with `key`: they retired their key, or were removed. Returns whether any
    /// trust ended.
    pub fn clear_friend_trust(&mut self, key: &str) -> bool {
        let mut changed = false;
        for entry in &mut self.friends {
            if entry.is_friend_list() && entry.friend_key == key && entry.trusted_as.is_some() {
                entry.set_trusted(false);
                changed = true;
            }
        }
        changed
    }

    pub fn get(&self, id: &str) -> Option<&FriendServer> {
        self.friends.iter().find(|f| f.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut FriendServer> {
        self.friends.iter_mut().find(|f| f.id == id)
    }

    /// Which game port joining the server `id` asks for: its saved port, or, on its
    /// first join, ports picked from its id that no other saved server has.
    pub fn game_port(&self, id: &str) -> GamePort {
        if let Some(port) = self.get(id).and_then(|f| f.game_port) {
            return GamePort::Saved(port);
        }
        let taken: Vec<u16> = self
            .friends
            .iter()
            .filter(|f| f.id != id)
            .filter_map(|f| f.game_port)
            .collect();
        GamePort::Choose(port_choices(id, &taken))
    }

    /// Takes the entry out; returns it.
    pub fn remove(&mut self, id: &str) -> Option<FriendServer> {
        let index = self.friends.iter().position(|f| f.id == id)?;
        Some(self.friends.remove(index))
    }
}

/// Where the entry `invite` is the link of is in `friends`: the one joined with it, or
/// the one that took it in place of an older link.
fn position_of(friends: &[FriendServer], invite: &Invite) -> Option<usize> {
    let id = friend_id(invite);
    let link = public_link(invite);
    friends
        .iter()
        .position(|f| f.id == id)
        .or_else(|| friends.iter().position(|f| f.link == link))
}

/// The id of the saved server `invite` is the link of: its entry's (which may be an
/// older link's [`friend_id`]), or, for a link not saved yet, the link's own.
pub fn saved_id<'a, I>(friends: I, invite: &Invite) -> String
where
    I: IntoIterator<Item = &'a FriendServer>,
    I::IntoIter: Clone,
{
    let id = friend_id(invite);
    let link = public_link(invite);
    let friends = friends.into_iter();
    friends
        .clone()
        .find(|f| f.id == id)
        .or_else(|| friends.clone().find(|f| f.link == link))
        .map_or(id, |f| f.id.clone())
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

/// Keeps `invite` as the link of the saved server `id`, in place of any older one.
pub async fn write_invite(
    store: &dyn SecretStore,
    id: &str,
    invite: &Invite,
) -> anyhow::Result<()> {
    store
        .write(&invite_secret_name(id), &Secret::new(invite.link()))
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
            let id = friend_id(&invite);
            write_invite(&store, &id, &invite).await.unwrap();
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

    fn invite_for(code: &str, fingerprint: u8) -> Invite {
        let text = format!(
            "https://connect.lodestone.cc/j/{code}#{}.{}",
            format!("{fingerprint:02x}").repeat(32),
            "02".repeat(16)
        );
        Invite::parse(&text).unwrap()
    }

    #[test]
    fn a_new_link_for_the_same_server_replaces_the_old_one() {
        let mut file = FriendsFile::default();
        let first = invite(0xab);
        let server = shared("fabric", Some("0.16.14"), true);
        assert_eq!(file.upsert(&first, &server, 1), 0);
        file.friends[0].set_trusted(true);
        let id = friend_id(&first);

        // the friend stopped sharing and shared again: a new code and certificate
        let second = invite_for("NEWCODE777", 0xcd);
        assert_eq!(saved_id(&file.friends, &second), friend_id(&second));
        assert_eq!(file.upsert(&second, &server, 2), 0);
        assert_eq!(file.friends.len(), 1);
        let entry = &file.friends[0];
        // the id stays, so instances made for it stay linked; the link is the new one
        assert_eq!(entry.id, id);
        assert_eq!(entry.link, public_link(&second));
        assert_eq!(entry.fingerprint, "cd".repeat(32));
        assert_eq!((entry.added_at, entry.last_joined), (1, Some(2)));
        assert!(!entry.is_trusted(), "a new certificate ends the trust");
        // the new link finds the entry; the old one too
        assert_eq!(saved_id(&file.friends, &second), id);
        assert_eq!(saved_id(&file.friends, &first), id);
        assert_eq!(file.upsert(&second, &server, 3), 0);
        assert_eq!(file.friends.len(), 1);

        // the stored link is replaced under the same id
        let store = MemorySecretStore::new();
        block_on(async {
            write_invite(&store, &id, &first).await.unwrap();
            write_invite(&store, &id, &second).await.unwrap();
            assert_eq!(store.keys(), [invite_secret_name(&id)]);
            assert_eq!(read_invite(&store, &id).await.unwrap(), Some(second));
        });

        // another instance of the same core is another server
        let mut other = server.clone();
        other.instance_uuid = "creative".into();
        assert_eq!(file.upsert(&invite_for("OTHER1", 0xcd), &other, 4), 1);
        // as is the same instance id on another core
        let mut elsewhere = server.clone();
        elsewhere.core_uuid = "another core".into();
        assert_eq!(file.upsert(&invite_for("OTHER2", 0xee), &elsewhere, 5), 2);
        assert_eq!(file.friends.len(), 3);
    }

    #[test]
    fn entries_saved_before_the_ids_were_kept_match_by_certificate() {
        let server = shared("vanilla", None, false);
        let mut old = FriendServer::new(&invite(0xab), &server, 1);
        (old.core_uuid, old.instance_uuid) = (String::new(), String::new());
        // as an older launcher wrote it: without the ids
        let text = serde_json::to_string(&old).unwrap();
        assert!(!text.contains("core_uuid"), "{text}");
        let mut file = FriendsFile {
            friends: vec![serde_json::from_str(&text).unwrap()],
        };
        // a new link from the same host (its certificate): the same entry, which now
        // keeps the ids
        assert_eq!(file.upsert(&invite_for("SAMEHOST1", 0xab), &server, 2), 0);
        assert_eq!(file.friends[0].core_uuid, "core");
        assert_eq!(file.friends[0].instance_uuid, "server");
        assert_eq!(file.friends[0].shared().instance_uuid, "server");
        // another certificate and other ids: another server
        let mut other = server.clone();
        other.core_uuid = "other".into();
        assert_eq!(file.upsert(&invite_for("ELSE1", 0x11), &other, 3), 1);
    }

    #[test]
    fn servers_from_the_friend_list_have_their_own_ids_and_trust() {
        let fingerprint = "AB".repeat(32);
        let id = friend_list_id(&fingerprint, "server");
        assert_eq!(id, format!("friend:{}/server", "ab".repeat(32)));
        assert!(is_friend_id(&id));
        assert!(is_friend_list_id(&id));
        assert!(!is_friend_list_id("connect:connect.lodestone.cc/ABCDEFGH23"));
        let lower = "ab".repeat(32);
        assert_eq!(parse_friend_list_id(&id), Some((lower.as_str(), "server")));
        assert_eq!(parse_friend_list_id("friend:short/server"), None);
        assert_eq!(parse_friend_list_id(&format!("friend:{lower}/")), None);

        let mut file = FriendsFile::default();
        // an invite's entry for the same server stays apart
        file.upsert(&invite(0xab), &shared("vanilla", None, false), 1);
        let server = shared("fabric", Some("0.16.14"), true);
        assert_eq!(file.upsert_friend_list("k1", &fingerprint, &server, 2), 1);
        let entry = &file.friends[1];
        assert_eq!(entry.id, id);
        assert!(entry.is_friend_list());
        assert!(!file.friends[0].is_friend_list());
        assert_eq!(entry.friend_key, "k1");
        assert_eq!(entry.fingerprint, lower);
        assert_eq!(entry.link, "Alex's core");
        assert!(!entry.is_trusted(), "a friend's server starts untrusted");
        file.friends[1].set_trusted(true);
        file.friends[0].set_trusted(true);
        // joining again updates it and keeps the trust
        let mut renamed = server.clone();
        renamed.name = "SMP 2".into();
        assert_eq!(file.upsert_friend_list("k1", &fingerprint, &renamed, 3), 1);
        assert_eq!(file.friends.len(), 2);
        assert_eq!(file.friends[1].name, "SMP 2");
        assert_eq!(file.friends[1].added_at, 2);
        assert!(file.friends[1].is_trusted());
        // the core retired: trust in its servers ends, not in others
        assert!(!file.clear_core_trust(&"cd".repeat(32)));
        assert!(file.clear_core_trust(&fingerprint));
        assert!(!file.friends[1].is_trusted());
        assert!(file.friends[0].is_trusted());
        assert!(!file.clear_core_trust(&fingerprint));
        // another friend listing the core starts untrusted
        file.friends[1].set_trusted(true);
        file.upsert_friend_list("k2", &fingerprint, &server, 4);
        assert!(!file.friends[1].is_trusted());
        // trust holds for the trusted pack only
        file.friends[1].trust_pack(Some("pack-1"));
        assert!(file.friends[1].trusts_pack(Some("pack-1")));
        assert!(!file.friends[1].trusts_pack(Some("pack-2")));
        assert!(!file.friends[1].trusts_pack(None));
        file.friends[1].trust_pack(None);
        assert!(!file.friends[1].trusts_pack(None), "a pack without an ETag isn't trusted");
        // an invite's server keeps trusting any pack under its certificate
        assert!(file.friends[0].trusts_pack(Some("anything")));
        assert!(file.friends[0].trusts_pack(None));
        // the friend retired their key: their servers' trust ends
        file.friends[1].trust_pack(Some("pack-1"));
        assert!(!file.clear_friend_trust("k1"));
        assert!(file.clear_friend_trust("k2"));
        assert!(!file.friends[1].is_trusted());
        assert_eq!(file.friends[1].trusted_pack, None);
        assert!(file.friends[0].is_trusted());
        // invite entries don't write the key, so older launchers read them as before
        let text = serde_json::to_string(&file.friends[0]).unwrap();
        assert!(!text.contains("friend_key"), "{text}");
    }

    #[test]
    fn game_ports_are_picked_from_the_id_in_range() {
        let id = "connect:connect.lodestone.cc/ABCDEFGH23";
        let ports = port_choices(id, &[]);
        assert_eq!(ports.len(), PORT_TRIES);
        // the same id picks the same ports, on any computer and in any build
        assert_eq!(ports, port_choices(id, &[]));
        assert_eq!(ports[0], 30000 + (fnv1a(id.as_bytes()) % 10000) as u16);
        for port in &ports {
            assert!(GAME_PORTS.contains(port), "{port}");
        }
        assert!(!GAME_PORTS.contains(&25565) && !GAME_PORTS.contains(&16662));
        // the next ones follow, and wrap around at the end of the range
        for pair in ports.windows(2) {
            assert!(pair[1] == pair[0] + 1 || (pair[0] == 39999 && pair[1] == 30000));
        }
        // another id picks another port
        assert_ne!(
            port_choices("connect:connect.lodestone.cc/OTHER1", &[])[0],
            ports[0]
        );
        // other servers' ports are skipped
        let skipped = port_choices(id, &[ports[0], ports[2]]);
        assert_eq!(skipped[..2], [ports[1], ports[3]]);
        assert!(!skipped.contains(&ports[0]) && !skipped.contains(&ports[2]));
        // ids from all over the range stay in it
        for n in 0..500 {
            let port = port_choices(&format!("connect:x/{n}"), &[])[0];
            assert!(GAME_PORTS.contains(&port), "{port}");
        }
    }

    #[test]
    fn a_server_keeps_its_game_port() {
        let dir = tempfile::tempdir().unwrap();
        let path = friends_file(dir.path());
        let mut file = FriendsFile::default();
        let first = invite_for("FIRST1", 0xab);
        let id = friend_id(&first);
        // a first join picks a port from the id
        let wanted = file.game_port(&id);
        let GamePort::Choose(ports) = &wanted else {
            panic!("{wanted:?}");
        };
        assert_eq!(ports, &port_choices(&id, &[]));
        let index = file.upsert(&first, &shared("vanilla", None, false), 1);
        // the first choice was busy, so it took the second
        assert_eq!(file.friends[index].joined_on_port(&wanted, ports[1]), None);
        let saved = ports[1];
        assert_eq!(file.friends[index].game_port, Some(saved));

        // it survives saving and loading
        save(&path, &file).unwrap();
        let mut file = load(&path).unwrap();
        assert_eq!(file.friends[0].game_port, Some(saved));
        assert_eq!(file.game_port(&id), GamePort::Saved(saved));

        // later joins ask for it; when it is busy, that join uses another and it stays
        let entry = &mut file.friends[0];
        assert_eq!(entry.joined_on_port(&GamePort::Saved(saved), saved), None);
        assert_eq!(
            entry.joined_on_port(&GamePort::Saved(saved), 54321),
            Some(saved)
        );
        assert_eq!(entry.game_port, Some(saved));
        // a Check (any port) changes nothing
        assert_eq!(entry.joined_on_port(&GamePort::Any, 40000), None);
        assert_eq!(entry.game_port, Some(saved));

        // a first join that found none of its ports free saves nothing; the next one
        // tries them again
        let second = invite_for("SECOND2", 0xcd);
        let mut other = shared("vanilla", None, false);
        other.instance_uuid = "other".into();
        let index = file.upsert(&second, &other, 2);
        let wanted = file.game_port(&friend_id(&second));
        assert_eq!(file.friends[index].joined_on_port(&wanted, 61000), None);
        assert_eq!(file.friends[index].game_port, None);
        assert_eq!(file.game_port(&friend_id(&second)), wanted);

        // two saved servers never get the same port, even when one's first choice is
        // the other's port
        let clash = FriendServer {
            id: "connect:x/clash".into(),
            ..FriendServer::default()
        };
        let clash_first = port_choices(&clash.id, &[])[0];
        file.friends[0].game_port = Some(clash_first);
        file.friends.push(clash);
        let GamePort::Choose(ports) = file.game_port("connect:x/clash") else {
            panic!("a first join chooses");
        };
        assert!(!ports.contains(&clash_first));
        assert_eq!(ports[0], port_choices("connect:x/clash", &[])[1]);
    }

    #[test]
    fn the_server_goes_in_the_instances_server_list() {
        use super::server_list::SERVERS_DAT;
        let dir = tempfile::tempdir().unwrap();
        let game_dir = |instance: &str| dir.path().join(instance).join("minecraft");
        let list = |instance: &str| std::fs::read(game_dir(instance).join(SERVERS_DAT));
        let mut entry = FriendServer::new(&invite(0xab), &shared("vanilla", None, false), 1);

        // no saved port (this session's port was a fallback): nothing is listed
        entry.list_in_game("a", &game_dir("a"), false).unwrap();
        assert!(list("a").is_err());

        entry.game_port = Some(31234);
        // never while the game runs
        entry.list_in_game("a", &game_dir("a"), true).unwrap();
        assert!(list("a").is_err());
        assert!(entry.listed.is_empty());

        entry.list_in_game("a", &game_dir("a"), false).unwrap();
        entry.list_in_game("b", &game_dir("b"), false).unwrap();
        let a = entry.listed_in("a").unwrap().clone();
        assert_eq!((a.name.as_str(), a.ip.as_str()), ("SMP", "127.0.0.1:31234"));
        assert_eq!(entry.listed.len(), 2);
        let written = list("a").unwrap();
        // again: the same entry, nothing written twice
        entry.list_in_game("a", &game_dir("a"), false).unwrap();
        assert_eq!(list("a").unwrap(), written);
        assert_eq!(entry.listed.len(), 2);

        // what was listed survives saving and loading
        let path = friends_file(dir.path());
        let file = FriendsFile {
            friends: vec![entry.clone()],
        };
        save(&path, &file).unwrap();
        assert_eq!(load(&path).unwrap(), file);

        // removing the friend's server takes it out, except where the game runs
        let failed = entry.unlist_in_games(game_dir, |instance| instance == "b");
        assert!(failed.is_empty(), "{failed:?}");
        assert_ne!(list("a").unwrap(), written);
        assert!(!String::from_utf8_lossy(&list("a").unwrap()).contains("31234"));
        assert!(String::from_utf8_lossy(&list("b").unwrap()).contains("31234"));
    }

    #[test]
    fn an_older_file_without_game_ports_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = friends_file(dir.path());
        let mut file = FriendsFile::default();
        file.upsert(&invite(0xab), &shared("vanilla", None, false), 1);
        save(&path, &file).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        // as an older launcher wrote it: no game port
        assert!(!text.contains("game_port"), "{text}");
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.friends[0].game_port, None);
        assert!(matches!(
            loaded.game_port(&loaded.friends[0].id),
            GamePort::Choose(_)
        ));
        // once saved, the port is one more field, which older launchers ignore
        let mut entry = loaded.friends[0].clone();
        entry.game_port = Some(31234);
        let text = serde_json::to_string(&entry).unwrap();
        assert!(text.contains(r#""game_port":31234"#), "{text}");
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
