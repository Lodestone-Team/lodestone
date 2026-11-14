//! Views: what one side gets of an image.
//!
//! ```text
//! view(image, side, rules) = entries with env[side] ≠ unsupported that the side can hold
//!                            (on the client, configs only when marked for players)
//!                          + the base (each side reads its own share of the pack)
//!                          + the side's settings
//!                          + { side, projected_from: image id, rules }
//! ```
//!
//! - Server: every entry a server can hold ([`crate::paths::placement`]) whose `env.server`
//!   isn't `unsupported`, configs included.
//! - Client: every entry a client can hold whose `env.client` isn't `unsupported`, so
//!   `optional` mods (JEI) go to players and players can always join. Configs (`config/`,
//!   `defaultconfigs/`, `kubejs/`) only when the owner marked them for players: a server's
//!   configs can hold things players shouldn't see, and the base pack already gives
//!   players its own. Never Lodestone's own files ([`crate::paths::is_lodestone_path`]: a
//!   server's modpack record and history are its bookkeeping, and images from a server
//!   can't write Lodestone's files), nor a server's own state that a client never uses
//!   ([`is_server_state`]: Bukkit plugins and their settings, the player-name caches,
//!   Forge's start scripts, the server jars in `versions/` and at the top level), nor
//!   disabled files and Lodestone's bookkeeping deeper down
//!   ([`is_inactive_or_bookkeeping`]).
//!
//! Views are computed, never edited, and canonical: entries sorted, no timestamps, and
//! none of the image's informational fields (label, producer, parent, created). So the same
//! image and rules give the same view byte for byte, and a view's id is its hash: "is the
//! player in step?" is comparing two ids. `projected_from` is left out of the view's id
//! (see [`crate::identity`]), so a change only the server sees leaves the client view's id
//! as it was. The core recomputes views; it doesn't store them.
//!
//! Losslessness: the server view, the client view and the entries no view carries give back
//! the image ([`reassemble`], [`check_lossless`]). Local settings are never in an image, so
//! they don't take part.

use std::collections::BTreeMap;

use crate::env::{EnvFrom, EnvValue};
use crate::manifest::{Entry, Image, Producer, RULES};
use crate::paths::{in_names, is_config, is_lodestone_path, placement, CanonicalPath};
use crate::settings::Settings;
use crate::Side;

/// Why a view couldn't be computed or put back together.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ViewError {
    #[error("view rules {0} aren't known to this version, which has rules {RULES}")]
    UnsupportedRules(u32),
    #[error("the image is already the {0:?} side's view")]
    AlreadyView(Side),
    #[error("the views don't give back the image: {0}")]
    Mismatch(String),
}

/// Whether the owner marked `entry` for players.
pub fn marked_for_players(entry: &Entry) -> bool {
    entry.env_from.contains(&EnvFrom::OwnerMark) && entry.env.client != EnvValue::Unsupported
}

/// Top-level files a server keeps for itself that a client never uses: Bukkit's and its
/// forks' settings (Spigot, Paper, Purpur, Pufferfish), Paper's console history (commands
/// the admin typed) and version history, the player-name caches (a client writes its own),
/// and Forge's and NeoForge's start scripts and their JVM arguments.
pub const SERVER_STATE_FILES: [&str; 15] = [
    "bukkit.yml",
    "spigot.yml",
    "commands.yml",
    "help.yml",
    "permissions.yml",
    "paper.yml",
    "purpur.yml",
    "pufferfish.yml",
    ".console_history",
    "version_history.json",
    "usercache.json",
    "usernamecache.json",
    "run.sh",
    "run.bat",
    "user_jvm_args.txt",
];
/// Top-level folders of a server's own state: Bukkit plugins and their data, and
/// `versions/`, where vanilla's bundler (1.18 on) and Paper's paperclip unpack the server
/// jar (`versions/1.21.1/server-1.21.1.jar`, `paper-1.21.1.jar`).
pub const SERVER_STATE_FOLDERS: [&str; 2] = ["plugins", "versions"];

/// What disabling a file appends to its name, the Launcher's and the core's way.
const DISABLED_SUFFIX: &str = ".disabled";

/// Whether `path` is a server's own state, which the client view leaves out even though
/// a client could hold it (rules 1): [`SERVER_STATE_FILES`] and [`SERVER_STATE_FOLDERS`],
/// and jars at the top level (old Forge's `forge-<version>.jar` and
/// `minecraft_server.<version>.jar`, a server's own jar under any name: a client loads
/// mods from `mods/`, never jars beside its game folder), whatever their letter case.
/// Unlike server-only files ([`placement`]) these stay in a Launcher's images (a client
/// writes its own `usercache.json`); they just never go from a server to players.
pub fn is_server_state(path: &CanonicalPath) -> bool {
    let mut parts = path.components();
    let first = parts.next().unwrap_or_default();
    match parts.next() {
        None => {
            in_names(first, &SERVER_STATE_FILES) || first.to_ascii_lowercase().ends_with(".jar")
        }
        Some(_) => in_names(first, &SERVER_STATE_FOLDERS),
    }
}

/// Whether `path` is a file nothing loads or Lodestone's own bookkeeping, at any depth: a
/// disabled file (`mods/x.jar.disabled`; today's client pack sends only enabled mods) or a
/// name starting with `.lodestone` (`mods/.lodestone_client_mods.json`), whatever their
/// letter case. The client view leaves them out (rules 1).
pub fn is_inactive_or_bookkeeping(path: &CanonicalPath) -> bool {
    path.components().any(|part| {
        let part = part.to_ascii_lowercase();
        part.starts_with(".lodestone")
    }) || path
        .as_str()
        .to_ascii_lowercase()
        .ends_with(DISABLED_SUFFIX)
}

/// Whether `entry` belongs in `side`'s view (rules 1).
pub fn in_view(entry: &Entry, side: Side) -> bool {
    if !placement(&entry.path).allows(side) || !entry.env.allows(side) {
        return false;
    }
    match side {
        Side::Server => true,
        Side::Client => {
            !is_lodestone_path(&entry.path)
                && !is_server_state(&entry.path)
                && !is_inactive_or_bookkeeping(&entry.path)
                && (!is_config(&entry.path) || marked_for_players(entry))
        }
    }
}

fn check_rules(rules: u32) -> Result<(), ViewError> {
    if rules == 0 || rules > RULES {
        return Err(ViewError::UnsupportedRules(rules));
    }
    Ok(())
}

/// `side`'s view of `image` under `rules`, sealed. The image's own rules must be known too,
/// since its entries' `env` follows them.
pub fn view(image: &Image, side: Side, rules: u32) -> Result<Image, ViewError> {
    check_rules(rules)?;
    check_rules(image.rules)?;
    match image.side {
        Some(already) if already == side => {
            // Filtered again, though for a view this computed it changes nothing: a view
            // from elsewhere (a server's answer) may carry what the side's view never does,
            // so callers check one by comparing its id with its view's.
            let mut same = image.clone();
            same.entries.retain(|entry| in_view(entry, side));
            same.settings = same.settings.for_side(side);
            same.seal();
            return Ok(same);
        }
        Some(other) => return Err(ViewError::AlreadyView(other)),
        None => {}
    }
    let source_id = if image.id.is_empty() {
        image.compute_id()
    } else {
        image.id.clone()
    };
    let entries = image
        .entries
        .iter()
        .filter(|entry| in_view(entry, side))
        .map(|entry| Entry {
            modified: None,
            ..entry.clone()
        })
        .collect();
    let mut view = Image {
        schema: image.schema,
        id: String::new(),
        kind: image.kind,
        parent: None,
        created: 0,
        producer: Producer::default(),
        label: None,
        base: image.base.clone(),
        game: image.game.clone(),
        settings: image.settings.for_side(side),
        entries,
        removed: image.removed.clone(),
        worlds: image.worlds.clone(),
        content: image.content.clone(),
        excludes: image.excludes.clone(),
        withheld: image.withheld.clone(),
        rules,
        side: Some(side),
        projected_from: Some(source_id),
        extra: image.extra.clone(),
    };
    view.seal();
    Ok(view)
}

/// The entries of `image` that neither view carries: they stay only where the image is.
pub fn local_entries(image: &Image) -> Vec<Entry> {
    image
        .entries
        .iter()
        .filter(|entry| !in_view(entry, Side::Server) && !in_view(entry, Side::Client))
        .cloned()
        .collect()
}

/// Puts an image back together from its server view, its client view and the entries no
/// view carries ([`local_entries`]). Errors when the views don't come from one image or
/// the result's id isn't the one they were projected from.
pub fn reassemble(server: &Image, client: &Image, local: &[Entry]) -> Result<Image, ViewError> {
    let mismatch = |what: &str| Err(ViewError::Mismatch(what.to_string()));
    if server.side != Some(Side::Server) || client.side != Some(Side::Client) {
        return mismatch("need a server view and a client view");
    }
    if server.projected_from != client.projected_from {
        return mismatch("the views come from different images");
    }
    if server.settings.client.is_some() || client.settings.server.is_some() {
        return mismatch("a view has the other side's settings");
    }
    let same_header = server.schema == client.schema
        && server.kind == client.kind
        && server.base == client.base
        && server.game == client.game
        && server.removed == client.removed
        && server.worlds == client.worlds
        && server.content == client.content
        && server.excludes == client.excludes
        && server.withheld == client.withheld
        && server.rules == client.rules
        && server.extra == client.extra;
    if !same_header {
        return mismatch("the views differ outside their entries and settings");
    }
    let mut entries: BTreeMap<&CanonicalPath, Entry> = BTreeMap::new();
    for entry in server.entries.iter().chain(&client.entries) {
        match entries.get(&entry.path) {
            Some(existing) if existing != entry => {
                return mismatch(&format!("{} differs between the views", entry.path));
            }
            Some(_) => {}
            None => {
                entries.insert(&entry.path, entry.clone());
            }
        }
    }
    for entry in local {
        if entries.insert(&entry.path, entry.clone()).is_some() {
            return mismatch(&format!("{} is both local and in a view", entry.path));
        }
    }
    let mut image = Image {
        entries: entries.into_values().collect(),
        settings: Settings {
            server: server.settings.server.clone(),
            client: client.settings.client.clone(),
        },
        side: None,
        projected_from: None,
        ..server.clone()
    };
    image.seal();
    if let Some(expected) = &server.projected_from {
        if image.id != *expected {
            return mismatch("the reassembled image has another id");
        }
    }
    Ok(image)
}

/// Checks `view(server) ∪ view(client) ∪ local entries = image`: nothing an image holds is
/// lost between its two views and what stays local.
pub fn check_lossless(image: &Image) -> Result<(), ViewError> {
    let server = view(image, Side::Server, image.rules)?;
    let client = view(image, Side::Client, image.rules)?;
    let local = local_entries(image);
    let reassembled = reassemble(&server, &client, &local)?;
    if reassembled.id != image.compute_id() {
        return Err(ViewError::Mismatch(
            "the reassembled image has another id".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::env::{self, EnvInputs, JarEnvironment, OwnerMark, SideValues};
    use crate::hex::sha256_hex;
    use crate::manifest::{Base, Game, Kind, Loader, Source, World};
    use crate::settings::{ClientSettings, ServerFlavour, ServerSettings};

    fn entry(path: &str, inputs: EnvInputs) -> Entry {
        let mut entry = Entry::new(
            CanonicalPath::new(path).unwrap(),
            path.len() as u64,
            sha256_hex(path.as_bytes()),
            Source::Object {},
        );
        let (env, from) = env::resolve(&inputs);
        entry.env = env;
        entry.env_from = from;
        entry.modified = Some(1_794_645_624_000_000_000);
        entry
    }

    fn modrinth(environment: &str) -> EnvInputs {
        EnvInputs {
            modrinth_environment: env::modrinth_environment(environment),
            ..Default::default()
        }
    }

    fn marked(mark: OwnerMark) -> EnvInputs {
        EnvInputs {
            owner_mark: Some(mark),
            ..Default::default()
        }
    }

    /// A Fabric pack server: client-only, server-only, optional, unknown, marked and
    /// useless mods, shared and unshared configs, server and client files, and a world.
    pub(crate) fn fabric_pack() -> Image {
        let mut image = Image::new(
            Kind::Full,
            Game {
                minecraft: "1.21.1".into(),
                loader: Loader::Fabric,
                loader_version: Some("0.16.10".into()),
            },
        );
        image.base = Some(Base::ModrinthPack {
            project: "pack".into(),
            version: "1.0.0".into(),
            sha512: "c".repeat(128),
        });
        image.created = 1_794_645_624;
        image.label = Some("Fixture".into());
        image.entries = vec![
            entry("mods/sodium.jar", modrinth("client_only")),
            entry("mods/lithium.jar", modrinth("client_and_server")),
            entry("mods/ledger.jar", modrinth("server_only")),
            entry(
                "mods/jei.jar",
                EnvInputs {
                    modrinth_project: Some(SideValues::parse(Some("optional"), Some("required"))),
                    ..Default::default()
                },
            ),
            entry("mods/mystery.jar", EnvInputs::default()),
            entry(
                "mods/marked.jar",
                EnvInputs {
                    owner_mark: Some(OwnerMark::ForPlayers),
                    jar: Some(JarEnvironment::Server),
                    ..Default::default()
                },
            ),
            entry(
                "mods/kept-out.jar",
                EnvInputs {
                    owner_mark: Some(OwnerMark::NotForPlayers),
                    modrinth_environment: env::modrinth_environment("client_or_server"),
                    ..Default::default()
                },
            ),
            entry(
                "mods/useless.jar",
                EnvInputs {
                    base_pack: Some(SideValues::parse(Some("unsupported"), Some("unsupported"))),
                    ..Default::default()
                },
            ),
            entry("config/jei/jei-client.ini", marked(OwnerMark::ForPlayers)),
            entry("config/ledger.toml", EnvInputs::default()),
            entry(
                "kubejs/startup_scripts/items.js",
                marked(OwnerMark::ForPlayers),
            ),
            entry("defaultconfigs/server.toml", EnvInputs::default()),
            entry("server.properties", EnvInputs::default()),
            entry("server.jar", EnvInputs::default()),
            entry("libraries/net/fabricmc/loader.jar", EnvInputs::default()),
            entry("options.txt", EnvInputs::default()),
            entry("resourcepacks/faithful.zip", EnvInputs::default()),
            entry("saves/world/level.dat", EnvInputs::default()),
            entry("saves/world/DIM-1/region/r.0.0.mca", EnvInputs::default()),
            // The server's own: never to players.
            entry(".lodestone_modpack/record.json", EnvInputs::default()),
            entry("plugins/x.jar", EnvInputs::default()),
            entry("usercache.json", EnvInputs::default()),
        ];
        image.worlds = vec![World {
            name: "world".into(),
            level_name: Some("world".into()),
            data_version: Some(3953),
            game_version: Some("1.21.1".into()),
            size: Some(2),
            last_played: Some(1_794_645_624),
            quiesced: Some(true),
        }];
        image.removed = vec![CanonicalPath::new("mods/pack-mod.jar").unwrap()];
        image.excludes = vec!["dynmap/web/tiles/".into()];
        image.settings = Settings {
            server: Some(ServerSettings {
                port: Some(25565),
                flavour: Some(ServerFlavour::Fabric),
                ..Default::default()
            }),
            client: Some(ClientSettings {
                recommended_memory_mb: Some(6144),
                resource_packs: vec!["vanilla".into(), "file/faithful.zip".into()],
                ..Default::default()
            }),
        };
        image.seal();
        image.validate().unwrap();
        image
    }

    fn paths(image: &Image) -> Vec<&str> {
        image.entries.iter().map(|e| e.path.as_str()).collect()
    }

    #[test]
    fn server_view_of_a_fabric_pack() {
        let image = fabric_pack();
        let server = view(&image, Side::Server, RULES).unwrap();
        assert_eq!(
            paths(&server),
            [
                ".lodestone_modpack/record.json",
                "config/jei/jei-client.ini",
                "config/ledger.toml",
                "defaultconfigs/server.toml",
                "kubejs/startup_scripts/items.js",
                "libraries/net/fabricmc/loader.jar",
                "mods/jei.jar",
                "mods/kept-out.jar",
                "mods/ledger.jar",
                "mods/lithium.jar",
                "mods/marked.jar",
                "mods/mystery.jar",
                "plugins/x.jar",
                "saves/world/DIM-1/region/r.0.0.mca",
                "saves/world/level.dat",
                "server.jar",
                "server.properties",
                "usercache.json",
            ]
        );
        assert_eq!(server.side, Some(Side::Server));
        assert_eq!(server.projected_from.as_deref(), Some(image.id.as_str()));
        assert!(server.settings.client.is_none());
        assert_eq!(server.settings.server, image.settings.server);
        assert_eq!(server.base, image.base);
        assert_eq!(server.removed, image.removed);
        server.verify().unwrap();
    }

    #[test]
    fn client_view_of_a_fabric_pack() {
        let image = fabric_pack();
        let client = view(&image, Side::Client, RULES).unwrap();
        assert_eq!(
            paths(&client),
            [
                // Marked configs only.
                "config/jei/jei-client.ini",
                "kubejs/startup_scripts/items.js",
                // Optional (JEI), unknown and marked mods go to players; server-only, kept
                // out and useless ones don't.
                "mods/jei.jar",
                "mods/lithium.jar",
                "mods/marked.jar",
                "mods/mystery.jar",
                "mods/sodium.jar",
                "options.txt",
                "resourcepacks/faithful.zip",
                "saves/world/DIM-1/region/r.0.0.mca",
                "saves/world/level.dat",
            ]
        );
        assert!(client.settings.server.is_none());
        assert_eq!(client.settings.client, image.settings.client);
        client.verify().unwrap();
    }

    #[test]
    fn views_are_canonical() {
        let image = fabric_pack();
        let client = view(&image, Side::Client, RULES).unwrap();
        assert!(client.entries.iter().all(|e| e.modified.is_none()));
        assert_eq!(client.created, 0);
        assert!(client.label.is_none() && client.parent.is_none());
        let mut sorted = client.entries.clone();
        sorted.sort_by(|a, b| a.path.cmp(&b.path));
        assert_eq!(sorted, client.entries);
    }

    #[test]
    fn views_are_deterministic_byte_for_byte() {
        let image = fabric_pack();
        // The same content recorded elsewhere: other order, timestamps, label, producer.
        let mut other = image.clone();
        other.entries.reverse();
        for entry in &mut other.entries {
            entry.modified = Some(7);
        }
        other.created = 1;
        other.label = Some("Somewhere else".into());
        other.parent = Some("a".repeat(64));
        other.seal();
        assert_eq!(other.id, image.id);
        for side in Side::BOTH {
            let a = view(&image, side, RULES).unwrap().to_json_pretty();
            let b = view(&image, side, RULES).unwrap().to_json_pretty();
            let c = view(&other, side, RULES).unwrap().to_json_pretty();
            assert_eq!(a, b);
            assert_eq!(a, c);
        }
    }

    #[test]
    fn a_server_only_change_keeps_the_client_view_id() {
        let image = fabric_pack();
        let client_before = view(&image, Side::Client, RULES).unwrap();
        let mut changed = image.clone();
        changed
            .entries
            .iter_mut()
            .find(|e| e.path.as_str() == "config/ledger.toml")
            .unwrap()
            .sha256 = "1".repeat(64);
        changed.settings.server.as_mut().unwrap().port = Some(25566);
        changed.seal();
        assert_ne!(changed.id, image.id);
        let client_after = view(&changed, Side::Client, RULES).unwrap();
        assert_eq!(client_after.id, client_before.id);
        assert_ne!(client_after.projected_from, client_before.projected_from);
        // The server view did change.
        assert_ne!(
            view(&changed, Side::Server, RULES).unwrap().id,
            view(&image, Side::Server, RULES).unwrap().id
        );
    }

    #[test]
    fn lodestone_files_and_server_state_stay_off_the_client_view() {
        let mut image = fabric_pack();
        let client_before = view(&image, Side::Client, RULES).unwrap();
        for path in [
            ".lodestone_modpack/history.json",
            ".LODESTONE_extra",
            "Plugins/y.jar",
            "bukkit.yml",
            "SPIGOT.yml",
            "usernamecache.json",
            "run.sh",
            "run.bat",
            "user_jvm_args.txt",
            "versions/1.21.1/paper-1.21.1.jar",
            "versions/1.21.1/server-1.21.1.jar",
            "forge-1.16.5-36.2.39.jar",
            "minecraft_server.1.16.5.jar",
            "Server.JAR",
            "mods/x.jar.disabled",
            "mods/y.jar.DISABLED",
            "mods/.lodestone_client_mods.json",
            "config/.LODESTONE/x",
        ] {
            assert!(
                !in_view(&object_entry(path), Side::Client),
                "{path} went to players"
            );
            assert!(in_view(&object_entry(path), Side::Server), "{path}");
            image.entries.push(object_entry(path));
        }
        image.seal();
        // The server's bookkeeping changing leaves the client view as it was.
        let client_after = view(&image, Side::Client, RULES).unwrap();
        assert_eq!(client_after.id, client_before.id);
        check_lossless(&image).unwrap();
        // Deeper down, the names are ordinary.
        assert!(in_view(&object_entry("mods/plugins/x.jar"), Side::Client));
        assert!(in_view(&object_entry("mods/usercache.json"), Side::Client));
        assert!(in_view(&object_entry("mods/versions/x.jar"), Side::Client));
        assert!(in_view(&object_entry("mods/a.jar"), Side::Client));
        assert!(in_view(&object_entry("mods/disabled"), Side::Client));
        assert!(in_view(&object_entry("mods/x.lodestone"), Side::Client));
    }

    fn object_entry(path: &str) -> Entry {
        Entry::new(
            CanonicalPath::new(path).unwrap(),
            1,
            "0".repeat(64),
            crate::manifest::Source::Object {},
        )
    }

    #[test]
    fn the_fabric_pack_is_lossless() {
        let image = fabric_pack();
        check_lossless(&image).unwrap();
        let local = local_entries(&image);
        assert_eq!(paths_of(&local), ["mods/useless.jar"]);
    }

    fn paths_of(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|e| e.path.as_str()).collect()
    }

    #[test]
    fn reassembly_refuses_mixed_views() {
        let image = fabric_pack();
        let server = view(&image, Side::Server, RULES).unwrap();
        let client = view(&image, Side::Client, RULES).unwrap();
        let local = local_entries(&image);
        let back = reassemble(&server, &client, &local).unwrap();
        assert_eq!(back.id, image.id);

        // Without the local entries, the id doesn't match.
        assert!(reassemble(&server, &client, &[]).is_err());
        // Two server views.
        assert!(reassemble(&server, &server, &local).is_err());
        // A client view of another image.
        let mut other = image.clone();
        other.game.loader_version = Some("0.17.0".into());
        other.seal();
        let other_client = view(&other, Side::Client, RULES).unwrap();
        assert!(reassemble(&server, &other_client, &local).is_err());
        // An entry edited in one view.
        let mut edited = client.clone();
        edited.entries[2].size += 1;
        assert!(reassemble(&server, &edited, &local).is_err());
        // A local entry that a view has too.
        let mut doubled = local.clone();
        doubled.push(server.entries[0].clone());
        assert!(reassemble(&server, &client, &doubled).is_err());
    }

    #[test]
    fn rules_and_view_of_view() {
        let image = fabric_pack();
        assert_eq!(
            view(&image, Side::Client, 2),
            Err(ViewError::UnsupportedRules(2))
        );
        assert_eq!(
            view(&image, Side::Client, 0),
            Err(ViewError::UnsupportedRules(0))
        );
        let mut newer = image.clone();
        newer.rules = 2;
        assert_eq!(
            view(&newer, Side::Server, 1),
            Err(ViewError::UnsupportedRules(2))
        );

        let client = view(&image, Side::Client, RULES).unwrap();
        assert_eq!(view(&client, Side::Client, RULES).unwrap(), client);
        assert_eq!(
            view(&client, Side::Server, RULES),
            Err(ViewError::AlreadyView(Side::Client))
        );

        // A "client view" from elsewhere that carries what no client view does is
        // filtered, so its id differs from the view of it.
        let mut hostile = client.clone();
        hostile.entries.push(object_entry("plugins/x.jar"));
        hostile.entries.push(object_entry("config/secret.toml"));
        hostile.settings.server = Some(crate::settings::ServerSettings {
            port: Some(25565),
            ..Default::default()
        });
        hostile.seal();
        let checked = view(&hostile, Side::Client, RULES).unwrap();
        assert_ne!(checked.id, hostile.id);
        assert_eq!(checked.id, client.id);
    }

    #[test]
    fn unsealed_images_project_from_their_computed_id() {
        let mut image = fabric_pack();
        let id = image.id.clone();
        image.id.clear();
        let server = view(&image, Side::Server, RULES).unwrap();
        assert_eq!(server.projected_from, Some(id));
    }

    #[test]
    fn lossless_for_every_kind_of_image() {
        // An empty image, a client instance's snapshot, a plain server.
        let empty = Image::new(Kind::Content, fabric_pack().game);
        check_lossless(&empty).unwrap();

        let mut client = fabric_pack();
        client.settings.server = None;
        client.entries.retain(|e| {
            !matches!(
                e.path.as_str(),
                "server.properties" | "server.jar" | "libraries/net/fabricmc/loader.jar"
            )
        });
        client.seal();
        check_lossless(&client).unwrap();

        let mut both_unsupported = fabric_pack();
        for entry in &mut both_unsupported.entries {
            entry.env.client = EnvValue::Unsupported;
            entry.env.server = EnvValue::Unsupported;
        }
        both_unsupported.seal();
        check_lossless(&both_unsupported).unwrap();
        assert_eq!(
            local_entries(&both_unsupported).len(),
            both_unsupported.entries.len()
        );
    }

    /// Random images across every placement class (both sides, server-only, client-only,
    /// configs, Lodestone's files, server state, worlds) with random env inputs, owner
    /// marks included: the two views and the local entries always give the image back.
    #[test]
    fn lossless_for_random_images() {
        let mut rng = crate::legacy::TestRng::new(0x1055_1e55);
        let paths = [
            "mods/a.jar",
            "mods/b.jar",
            "config/a.toml",
            "defaultconfigs/b.toml",
            "kubejs/startup.js",
            "server.properties",
            "server.jar",
            "libraries/x.jar",
            "eula.txt",
            "banned-players.json",
            "options.txt",
            "optionsof.txt",
            "servers.dat",
            "resourcepacks/x.zip",
            "shaderpacks/s.zip",
            "screenshots/1.png",
            "saves/loose.txt",
            ".lodestone_modpack/record.json",
            ".lodestone_modpack/history.json",
            "plugins/p.jar",
            "Plugins/q.jar",
            "bukkit.yml",
            "PURPUR.yml",
            ".console_history",
            "usercache.json",
            "run.sh",
            "user_jvm_args.txt",
            "saves/w/level.dat",
            "saves/w/region/r.0.0.mca",
            "saves/other/level.dat",
            "README.txt",
            "Config/upper.toml",
        ];
        let values = [
            None,
            Some(EnvValue::Required),
            Some(EnvValue::Optional),
            Some(EnvValue::Unsupported),
            Some(EnvValue::Unknown),
        ];
        let side_values = |rng: &mut crate::legacy::TestRng| {
            (rng.below(3) > 0).then(|| SideValues::new(*rng.pick(&values), *rng.pick(&values)))
        };
        for case in 0..3000 {
            let mut image = Image::new(
                *rng.pick(&[Kind::Content, Kind::Data, Kind::Full]),
                fabric_pack().game,
            );
            let mut chosen: Vec<&str> = paths
                .iter()
                .copied()
                .filter(|_| rng.below(2) == 0)
                .collect();
            if image.kind == Kind::Content {
                chosen.retain(|p| !p.starts_with("saves/") || p.matches('/').count() == 1);
            }
            for path in chosen {
                let inputs = EnvInputs {
                    owner_mark: *rng.pick(&[
                        None,
                        Some(OwnerMark::ForPlayers),
                        Some(OwnerMark::NotForPlayers),
                    ]),
                    base_pack: side_values(&mut rng),
                    modrinth_environment: side_values(&mut rng),
                    modrinth_project: side_values(&mut rng),
                    jar: *rng.pick(&[
                        None,
                        Some(JarEnvironment::Client),
                        Some(JarEnvironment::Server),
                    ]),
                };
                image.entries.push(entry(path, inputs));
            }
            let worlds: std::collections::BTreeSet<String> = image
                .entries
                .iter()
                .filter_map(|e| e.world.clone())
                .collect();
            image.worlds = worlds
                .into_iter()
                .map(|name| World {
                    name,
                    level_name: None,
                    data_version: None,
                    game_version: None,
                    size: None,
                    last_played: None,
                    quiesced: None,
                })
                .collect();
            if rng.below(2) == 0 {
                image.settings.server = Some(ServerSettings {
                    port: Some(25565),
                    ..ServerSettings::default()
                });
            }
            if rng.below(2) == 0 {
                image.settings.client = Some(ClientSettings {
                    recommended_memory_mb: Some(4096),
                    ..ClientSettings::default()
                });
            }
            image.seal();
            image
                .validate()
                .unwrap_or_else(|e| panic!("case {case}: {e}"));
            check_lossless(&image).unwrap_or_else(|e| panic!("case {case}: {e}"));
            // Nothing of Lodestone's or a server's own state ever reaches players.
            let client = view(&image, Side::Client, RULES).unwrap();
            assert!(client.entries.iter().all(|e| !is_lodestone_path(&e.path)
                && !is_server_state(&e.path)
                && !is_inactive_or_bookkeeping(&e.path)));
        }
    }

    #[test]
    fn marked_configs() {
        let mark = entry("config/a.toml", marked(OwnerMark::ForPlayers));
        assert!(marked_for_players(&mark));
        assert!(in_view(&mark, Side::Client));
        let kept = entry("config/a.toml", marked(OwnerMark::NotForPlayers));
        assert!(!marked_for_players(&kept));
        assert!(!in_view(&kept, Side::Client));
        assert!(in_view(&kept, Side::Server));
        let plain = entry("config/a.toml", EnvInputs::default());
        assert!(!in_view(&plain, Side::Client));
    }
}
