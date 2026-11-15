//! A server's backup exported by a Lodestone Core, as a Launcher instance can hold it
//! ("Download this world", and a core's `.lodeimg` opened in the Launcher:
//! `docs/instance-images.md`, "Sharing" and "M6b decisions").
//!
//! A core's export is a server's folder: `server.properties`, operators, plugins and their
//! settings, server-only mods, Paper's settings. A player never gets those from a server,
//! so the import takes what a player gets and the worlds, by the same rules a server's
//! players get its files by ("Server to player", M4b): the client view of the server's
//! content ([`lodestone_image::view`], `Side::Client`), and only what a Launcher's content
//! images hold (the content folders, `images::scope::is_content`), so a file the client
//! view lets through that no server may place on a player's machine is left out, never
//! placed. The worlds come as the server had them, without the files only a server reads
//! (`share::is_server_world_file`: Paper's per-world settings and its split Nether's and
//! End's own files; the dimensions come back as `DIM-1` and `DIM1` through the canonical
//! paths). The server's settings (properties, operators, whitelist) never come.
//!
//! A server built on a Modrinth modpack lists every file it has in its content images, so
//! the client view holds every pack file a player needs that the server also runs. The
//! pack's client share (its client-only index files and `client-overrides/`), which a
//! player of the server installs from the base (M4b, "What a view places"), is placed
//! too: [`place_pack_share`] gets the pack as M4b does (from Modrinth, checked by the
//! SHA-512 the base names) and places the share's files the images don't place, less the
//! paths the server removed from the pack. When the pack can't be had (offline, gone from
//! Modrinth, a CurseForge pack), the instance is made without those files and the import
//! says so ([`PackShare::Skipped`]).
//!
//! [`client_side`] makes the images an import places from a core's file; the rest of the
//! import (checks, downloads, placing) is the snapshot import's, so there is one builder.

use std::collections::BTreeSet;
use std::path::Path;

use lodestone_image::manifest::{App, Producer};
use lodestone_image::paths::in_names;
use lodestone_image::share::{is_server_world_file, placed_entries};
use lodestone_image::view::view;
use lodestone_image::{Base, CanonicalPath, EnvValue, Image, Kind, Loader, Side};
use serde::Serialize;

use super::ImageError;
use super::scope::is_content;
use crate::content::{Platforms, install::InstallProgress, provenance::Provenance};
use crate::cores::image_sync::{SyncError, base};
use crate::game::download::CancelToken;
use crate::modpack::{self, fetch, merge::Wanted, slash_path};

/// A core's file as a Launcher instance holds it.
#[derive(Debug, Clone)]
pub struct ClientSide {
    /// The data image: the worlds, linked to `linked`'s content image. Sealed.
    pub image: Image,
    /// The content image: what the server's players get. Sealed.
    pub linked: Vec<Image>,
    pub server: ServerFiles,
    /// The modpack the server is built on, whose client share the import places
    /// ([`place_pack_share`]).
    pub pack: Option<ServerPack>,
}

/// The modpack a server is built on (its content image's `base`), and the pack's paths the
/// server removed that the client view keeps removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerPack {
    pub base: Base,
    pub removed: Vec<CanonicalPath>,
}

impl ServerPack {
    /// Where the pack comes from, as an import's preview says it.
    pub fn platform(&self) -> PackPlatform {
        match self.base {
            Base::CurseforgePack { .. } => PackPlatform::Curseforge,
            _ => PackPlatform::Modrinth,
        }
    }
}

/// Where a server's modpack comes from: the Launcher downloads a Modrinth pack's client
/// share, and can't download a CurseForge one from a server (as for players, M4b).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PackPlatform {
    Modrinth,
    Curseforge,
}

/// What became of a server's modpack's client share in an import.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "result")]
pub enum PackShare {
    /// Its files the images didn't place were placed (`files` of them; 0 when the images
    /// had them all).
    Placed { files: usize },
    /// The pack couldn't be had or its files couldn't be placed, so the instance lacks the
    /// files only the pack's players use: `why`, for a window to say in its languages, and
    /// `reason` in English (the control API's).
    Skipped { why: SkipReason, reason: String },
}

/// Why a server's modpack's client share wasn't placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// The pack's version is gone from Modrinth.
    Gone,
    /// A CurseForge pack, which the Launcher doesn't install from a server.
    Curseforge,
    /// Modrinth couldn't be reached, a download failed or failed its check, or the files
    /// couldn't be written (no room for them, say).
    Failed,
}

/// What a core's file holds that the instance doesn't take.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerFiles {
    /// Mods only a server runs (their client side is unsupported), by their name in
    /// `mods/`, sorted.
    pub server_only_mods: Vec<String>,
    /// How many of the file's files a player's instance never takes: the server's settings
    /// and software, its operators, plugins and their settings, configs not shared with
    /// players, server-only mods, and the server's own world files.
    pub not_taken: u64,
}

/// Whether `image` was exported by a Lodestone Core: a server's files, which an import
/// takes the client side of ([`client_side`]).
pub fn is_from_core(image: &Image) -> bool {
    image.producer.app == App::Core
}

/// Whether the export `image` (with its content link `linked`) holds a server's files
/// whatever app it says made it: a `server.properties` or a `plugins/` folder at the top,
/// or a server's settings. A file opened by hand that does is taken as a core's export
/// ([`client_side`]), so a label written by the sender can't carry a server's settings,
/// plugins or unshared files into an instance. No Launcher instance has them.
pub fn holds_server_files(image: &Image, linked: &[Image]) -> bool {
    std::iter::once(image)
        .chain(linked)
        .any(|i| i.settings.server.is_some())
        || placed_entries(image, linked).iter().any(|e| {
            let path = e.path.as_str();
            match path.split_once('/') {
                Some((first, _)) => in_names(first, &["plugins"]),
                None => in_names(path, &["server.properties"]),
            }
        })
}

/// The images an import places from a core's export `image` and its content link
/// `linked` (see the module): a content image of the client view of the server's content,
/// in the content folders only, and a data image of the worlds linked to it. Both are
/// labelled `label` and made by this Launcher. The core's content images list every file
/// the server has, pack files too, so the images hold every file a player of the server
/// needs but the pack's client share, which [`ClientSide::pack`] names for
/// [`place_pack_share`].
pub fn client_side(image: &Image, linked: &[Image], label: &str) -> Result<ClientSide, ImageError> {
    let invalid = |e: String| ImageError::Invalid(format!("The server's files can't be read: {e}"));
    let all = placed_entries(image, linked);
    let total = all.len() as u64;
    let producer = Producer {
        app: App::Launcher,
        version: env!("CARGO_PKG_VERSION").into(),
    };
    let game = client_game(&linked.first().unwrap_or(image).game);

    // The server's content: every file but the worlds (the backup's own copy of each, over
    // its content image's, which may be older), with the content image's base and pack
    // removals, or the backup's when it has no content image.
    let mut content = linked.first().cloned().unwrap_or_else(|| image.clone());
    content.kind = Kind::Content;
    content.entries = with_content_sides(
        all.iter().filter(|e| e.world.is_none()).cloned().collect(),
        linked,
    );
    content.worlds.clear();
    content.content = None;
    content.side = None;
    content.projected_from = None;
    content.id = String::new();
    content.withheld.retain(|w| !under_saves(&w.path));
    content.normalize();
    let mut client =
        view(&content, Side::Client, content.rules).map_err(|e| invalid(e.to_string()))?;
    let server_only_mods = {
        let mut names: Vec<String> = content
            .entries
            .iter()
            .filter(|e| is_mod(&e.path) && e.env.client == EnvValue::Unsupported)
            .map(|e| lodestone_image::display_text(&mod_file(&e.path)).into_owned())
            .collect();
        names.sort();
        names.dedup();
        names
    };
    // only what a Launcher's content holds: a player never gets anything else from a server
    client.entries.retain(|e| is_content(&e.path));
    client.withheld.retain(|w| {
        CanonicalPath::new(&w.path).map_or_else(
            |_| {
                w.path
                    .split('/')
                    .next()
                    .is_some_and(|first| first.eq_ignore_ascii_case("mods"))
            },
            |path| is_content(&path),
        )
    });
    // The pack's client share is placed by `place_pack_share`, beside the entries; the
    // instance isn't linked to the pack (no `modpack.json`: the copy follows neither the
    // server nor the pack's updates), so the image claims no pack, and the content
    // recorded once the files are placed holds them all.
    let pack = match &client.base {
        Some(base @ (Base::ModrinthPack { .. } | Base::CurseforgePack { .. })) => {
            Some(ServerPack {
                base: base.clone(),
                removed: client.removed.clone(),
            })
        }
        Some(Base::Loader {}) | None => None,
    };
    client.base = None;
    client.removed.clear();
    client.side = None;
    client.projected_from = None;
    client.kind = Kind::Content;
    client.game = game.clone();
    client.settings.server = None;
    client.label = Some(label.to_string());
    client.producer = producer.clone();
    client.parent = None;
    client.created = image.created;
    client.seal();
    client
        .validate()
        .map_err(|e| invalid(format!("its players' files aren't valid: {e}")))?;

    // The worlds, without the files only a server reads.
    let mut data = image.clone();
    data.kind = Kind::Data;
    data.entries = image
        .entries
        .iter()
        .filter(|e| {
            e.world.is_some() && !is_server_world_file(&e.path) && !is_world_permissions(&e.path)
        })
        .cloned()
        .collect();
    data.withheld.retain(|w| under_saves(&w.path));
    data.settings.server = None;
    data.removed.clear();
    data.base = client.base.clone();
    data.game = game;
    data.content = Some(client.id.clone());
    data.side = None;
    data.projected_from = None;
    data.label = Some(label.to_string());
    data.producer = producer;
    data.parent = None;
    data.seal();
    data.validate()
        .map_err(|e| invalid(format!("its worlds aren't valid: {e}")))?;

    let taken = (client.entries.len() + data.entries.len()) as u64;
    Ok(ClientSide {
        server: ServerFiles {
            server_only_mods,
            not_taken: total.saturating_sub(taken),
        },
        image: data,
        linked: vec![client],
        pack,
    })
}

/// `entries` (the backup's copy of each file over its content image's) with the sides
/// the content images in `linked` give each path. The core's export copies a content
/// entry's sides and owner mark into the backup's only when their bytes match; a newer
/// copy in the backup (a config the server rewrote, a jar replaced on disk) comes with
/// unknown sides. The owner's mark belongs to the path and a jar's sides are the best
/// guess for its newer copy, so the content image's `env` and `env_from` carry over and a
/// player gets the same files as one joining the server through M4b: a config marked for
/// players comes and a server-only mod doesn't.
fn with_content_sides(
    mut entries: Vec<lodestone_image::Entry>,
    linked: &[Image],
) -> Vec<lodestone_image::Entry> {
    let sides: std::collections::BTreeMap<&str, &lodestone_image::Entry> = linked
        .iter()
        .flat_map(|c| c.entries.iter())
        .map(|e| (e.path.as_str(), e))
        .collect();
    for entry in &mut entries {
        if let Some(content) = sides.get(entry.path.as_str())
            && content.sha256 != entry.sha256
        {
            entry.env = content.env;
            entry.env_from = content.env_from.clone();
        }
    }
    entries
}

/// Whether `path` is a world's permissions data, which says who on the server may run
/// operators' commands (the core's own list, "M6a decisions"): FTB Ranks' ranks and players
/// (`saves/<world>/serverconfig/ftbranks/`) and ForgeEssentials' (`saves/<world>/FEData/`).
/// A player's copy of the world never takes them: they name the server's players.
fn is_world_permissions(path: &CanonicalPath) -> bool {
    let parts: Vec<&str> = path.components().collect();
    // Each arm is guarded, so a path matching one arm's shape but not its names still
    // reaches the next (FEData's files sit in subfolders as deep as FTB Ranks').
    match parts.as_slice() {
        [saves, _, serverconfig, ftbranks, _, ..]
            if in_names(saves, &["saves"])
                && in_names(serverconfig, &["serverconfig"])
                && in_names(ftbranks, &["ftbranks"]) =>
        {
            true
        }
        [saves, _, data, _, ..] if in_names(saves, &["saves"]) && in_names(data, &["fedata"]) => {
            true
        }
        _ => false,
    }
}

/// The name of the folder in an import's staging folder that a pack's share is staged in
/// before its files are moved into the game folder.
const PACK_STAGING: &str = ".lodestone-pack-share";

/// Places the client share of the server's modpack `pack` in the game folder `game` of an
/// import's staging folder `staging` (see the module): the pack from `cache` or Modrinth
/// (checked by the SHA-512 the base names), then every file of its client share that
/// `placed` (the paths the images place, which win) doesn't name, less the paths the server
/// removed and anything in `saves/` (the server's world is the one that comes), each
/// checked by the pack's hashes. A share file that would land on a file already there is
/// skipped. Downloaded mods are recorded in `content.json` as a pack install records them.
/// When the pack can't be had or a file of it can't be placed, nothing of it is placed
/// and the result says why; only cancelling fails.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn place_pack_share(
    pack: &ServerPack,
    placed: &[lodestone_image::Entry],
    platforms: Platforms<'_>,
    downloader: &crate::game::download::Downloader,
    cache: &Path,
    staging: &Path,
    game: &Path,
    cancel: &CancelToken,
) -> Result<PackShare, ImageError> {
    let skipped = |reason: String| {
        Ok(PackShare::Skipped {
            why: SkipReason::Failed,
            reason,
        })
    };
    let install = InstallProgress::default();
    if let Err(e) = std::fs::create_dir_all(cache) {
        return skipped(format!("the modpack cache can't be made: {e}"));
    }
    let got = base::base_pack(
        platforms.modrinth,
        downloader,
        cache,
        Some(&pack.base),
        &install,
        cancel,
    )
    .await;
    base::prune_cache(cache);
    let found = match got {
        Ok(Some(found)) => found,
        Ok(None) => return Ok(PackShare::Placed { files: 0 }),
        Err(SyncError::Cancelled) => return Err(ImageError::Cancelled),
        Err(e) => {
            let why = match &e {
                SyncError::BasePackGone { .. } => SkipReason::Gone,
                SyncError::CurseForgeBase => SkipReason::Curseforge,
                _ => SkipReason::Failed,
            };
            return Ok(PackShare::Skipped {
                why,
                reason: e.to_string(),
            });
        }
    };
    let writes = share_writes(&found.pack, &pack.removed, placed, game);
    if writes.is_empty() {
        return Ok(PackShare::Placed { files: 0 });
    }
    // The import's own room check couldn't count these (their sizes are known only once
    // the pack is read), so a share that doesn't fit with room to spare isn't placed.
    let needed = writes
        .iter()
        .fold(super::recorder::SPARE_BYTES, |sum, (fetch, _)| {
            sum.saturating_add(fetch.size())
        });
    if let Some(free) = crate::disk::free_space(staging)
        && needed > free
    {
        return skipped(format!(
            "the modpack's files need {needed} bytes free, and {free} are"
        ));
    }
    let temp = staging.join(PACK_STAGING);
    if let Err(e) = std::fs::create_dir_all(&temp) {
        return skipped(format!("{}: {e}", temp.display()));
    }
    let staged = fetch::stage(
        &fetch::Sources {
            platforms,
            downloader,
            pack: Some(&found.path),
            objects: None,
        },
        &temp,
        &found.pack.name,
        &writes,
        &install,
        cancel,
    )
    .await;
    let moved = match staged {
        Ok(()) => move_staged(&temp, game, &writes),
        Err(modpack::ModpackError::Cancelled) => {
            let _ = super::remove_tree(&temp);
            return Err(ImageError::Cancelled);
        }
        Err(e) => Err(e.to_string()),
    };
    let _ = super::remove_tree(&temp);
    if let Err(reason) = moved {
        // nothing of the pack stays: what was moved is taken back out
        for (_, path) in &writes {
            let _ = std::fs::remove_file(game.join(path));
        }
        return skipped(reason);
    }
    // content.json learns the pack's downloads, as installing a pack records them
    let downloads: Vec<(String, &lodestone_minecraft::mrpack::PackDownload)> = writes
        .iter()
        .filter_map(|(fetch, path)| match fetch {
            modpack::merge::Fetch::Download { download, .. } => Some((path.clone(), download)),
            _ => None,
        })
        .collect();
    let mut provenance = Provenance::load(staging);
    for (kind, file, entry) in
        modpack::import::content_entries(platforms.modrinth, game, &downloads).await
    {
        provenance.insert(kind, &file, entry);
    }
    provenance
        .save(staging)
        .map_err(|e| ImageError::Invalid(format!("content.json can't be saved: {e}")))?;
    Ok(PackShare::Placed {
        files: writes.len(),
    })
}

/// The files of `pack`'s client share an import places (see [`place_pack_share`]), with
/// where each comes from, by path in the game folder `game`.
fn share_writes(
    pack: &modpack::PackInfo,
    removed: &[CanonicalPath],
    placed: &[lodestone_image::Entry],
    game: &Path,
) -> Vec<(modpack::merge::Fetch, String)> {
    // Compared in any letter case: on a disk that ignores it, `Mods/A.jar` is `mods/a.jar`.
    let fold = |path: &str| path.to_lowercase();
    let taken: BTreeSet<String> = placed
        .iter()
        .map(|e| fold(e.path.as_str()))
        .chain(removed.iter().map(|p| fold(p.as_str())))
        .collect();
    let share = pack
        .downloads
        .iter()
        .map(|d| (slash_path(&d.path), Wanted::download(d)))
        .chain(
            pack.overrides
                .iter()
                .map(|f| (f.path.clone(), Wanted::override_file(f))),
        );
    let mut seen = BTreeSet::new();
    let mut writes = Vec::new();
    // overrides after downloads, so an override at a download's path wins, as the pack
    // installs it
    let share: Vec<(String, Wanted)> = share.collect();
    for (path, wanted) in share.into_iter().rev() {
        let first = path.split('/').next().unwrap_or_default();
        if in_names(first, &["saves"])
            || taken.contains(&fold(&path))
            || !seen.insert(fold(&path))
            || std::fs::symlink_metadata(game.join(&path)).is_ok()
        {
            continue;
        }
        writes.push((wanted.fetch, path));
    }
    writes.reverse();
    writes
}

/// Moves the files `writes` staged in `temp` to their paths in `game`. A path whose place
/// is taken by then (a folder of the same name) fails the move. Returns why, for people.
fn move_staged(
    temp: &Path,
    game: &Path,
    writes: &[(modpack::merge::Fetch, String)],
) -> Result<(), String> {
    for (_, path) in writes {
        let from = temp.join(path);
        let to = game.join(path);
        if std::fs::symlink_metadata(&to).is_ok() {
            return Err(format!("{path} is in the way"));
        }
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{path}: {e}"))?;
        }
        std::fs::rename(&from, &to).map_err(|e| format!("{path}: {e}"))?;
    }
    Ok(())
}

/// The game as an instance names it: Forge's version as a full Maven version
/// (`1.20.1-47.4.28`), as a server's players get it (`image_sync::plan::loader_of`).
fn client_game(game: &lodestone_image::Game) -> lodestone_image::Game {
    let mut game = game.clone();
    if game.loader == Loader::Forge
        && let Some(version) = &game.loader_version
        && !version.starts_with(&format!("{}-", game.minecraft))
    {
        game.loader_version = Some(format!("{}-{version}", game.minecraft));
    }
    game
}

/// Whether a path as found (maybe not canonical) is in `saves/`, any letter case.
fn under_saves(path: &str) -> bool {
    path.split('/')
        .next()
        .is_some_and(|first| first.eq_ignore_ascii_case("saves"))
}

fn is_mod(path: &CanonicalPath) -> bool {
    let mut parts = path.components();
    parts
        .next()
        .is_some_and(|first| first.eq_ignore_ascii_case("mods"))
        && parts.next().is_some()
}

/// A mod's path under `mods/`.
fn mod_file(path: &CanonicalPath) -> String {
    path.as_str()
        .split_once('/')
        .map_or(path.as_str(), |(_, rest)| rest)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lodestone_image::manifest::Base;
    use lodestone_image::settings::{ServerSettings, Settings};
    use lodestone_image::{Entry, EnvFrom, Game, Source, Withheld, WithheldReason, World};

    fn entry(path: &str) -> Entry {
        Entry::new(
            CanonicalPath::new(path).unwrap(),
            path.len() as u64,
            lodestone_image::hex::sha256_hex(path.as_bytes()),
            Source::Object {},
        )
    }

    fn world_entry(path: &str) -> Entry {
        let mut entry = entry(path);
        entry.world = Some(path.split('/').nth(1).unwrap().to_string());
        entry
    }

    #[test]
    fn a_worlds_permissions_data_never_comes() {
        for path in [
            "saves/world/serverconfig/ftbranks/ranks.snbt",
            "saves/world/serverconfig/FTBRanks/players.snbt",
            "saves/world/FEData/groups.json",
            "saves/world/FEData/json/PlayerInfo/x.json",
            "saves/world/FEData/json/groups.json",
            "saves/world/fedata/json/Zone/a/b.json",
            "saves/world/serverconfig/ftbranks/sub/x.snbt",
        ] {
            assert!(
                is_world_permissions(&CanonicalPath::new(path).unwrap()),
                "{path}"
            );
        }
        for path in [
            "saves/world/serverconfig/create-server.toml",
            "saves/world/level.dat",
            "config/ftbranks/x.snbt",
            "saves/world/serverconfig/other/sub/x.snbt",
            "saves/world/region/r.0.0.mca",
            "saves/world/data/json/a/b.json",
        ] {
            assert!(
                !is_world_permissions(&CanonicalPath::new(path).unwrap()),
                "{path}"
            );
        }
    }

    fn sided(path: &str, client: EnvValue, server: EnvValue) -> Entry {
        let mut entry = entry(path);
        entry.env.client = client;
        entry.env.server = server;
        entry.env_from = vec![EnvFrom::Jar];
        entry
    }

    fn game(loader: Loader, version: Option<&str>) -> Game {
        Game {
            minecraft: "1.20.1".into(),
            loader,
            loader_version: version.map(str::to_string),
        }
    }

    /// A core's export of a server: a content image and a backup linked to it.
    fn export(loader: Loader, version: Option<&str>) -> (Image, Vec<Image>) {
        let core = Producer {
            app: App::Core,
            version: "0.6.0".into(),
        };
        let mut content = Image::new(Kind::Content, game(loader.clone(), version));
        content.producer = core.clone();
        content.entries = vec![
            sided("mods/both.jar", EnvValue::Required, EnvValue::Required),
            sided(
                "mods/server-only.jar",
                EnvValue::Unsupported,
                EnvValue::Required,
            ),
            sided(
                "mods/client-only.jar",
                EnvValue::Required,
                EnvValue::Unsupported,
            ),
            entry("config/unshared.toml"),
            entry("plugins/Spy.jar"),
            entry("plugins/Spy/config.yml"),
            entry("server.properties"),
            entry("ops.json"),
            entry("bukkit.yml"),
        ];
        content.removed = vec![
            CanonicalPath::new("mods/pack-removed.jar").unwrap(),
            CanonicalPath::new("config/pack.toml").unwrap(),
        ];
        content.base = Some(Base::Loader {});
        content.normalize();
        content.seal();
        let mut data = Image::new(Kind::Data, game(loader, version));
        data.producer = core;
        data.label = Some("SMP – Before the update".into());
        data.worlds = vec![World {
            name: "world".into(),
            level_name: None,
            data_version: None,
            game_version: None,
            size: None,
            last_played: None,
            quiesced: Some(true),
        }];
        data.entries = content.entries.clone();
        data.entries.extend([
            world_entry("saves/world/level.dat"),
            world_entry("saves/world/DIM-1/region/r.0.0.mca"),
            world_entry("saves/world/DIM1/region/r.0.0.mca"),
            world_entry("saves/world/paper-world.yml"),
            world_entry("saves/world/.bukkit_nether/level.dat"),
            world_entry("saves/world/.bukkit_the_end/paper-world.yml"),
        ]);
        data.withheld = vec![
            Withheld {
                path: "saves/world/region/r.9.9.mca".into(),
                reason: WithheldReason::KeptChanging,
            },
            Withheld {
                path: "plugins/big.db".into(),
                reason: WithheldReason::Unreadable,
            },
        ];
        data.settings = Settings {
            server: Some(ServerSettings {
                port: Some(25565),
                ..Default::default()
            }),
            client: None,
        };
        data.content = Some(content.id.clone());
        data.normalize();
        data.seal();
        (data, vec![content])
    }

    fn paths(image: &Image) -> Vec<&str> {
        image.entries.iter().map(|e| e.path.as_str()).collect()
    }

    #[test]
    fn a_player_gets_the_client_view_of_the_content_and_the_worlds() {
        let (data, linked) = export(Loader::Fabric, Some("0.16.10"));
        assert!(is_from_core(&data));
        let side = client_side(&data, &linked, "Downloaded from SMP").unwrap();
        let content = &side.linked[0];
        // server-only mods, unshared configs, plugins and server files are left out
        assert_eq!(paths(content), ["mods/both.jar", "mods/client-only.jar"]);
        assert_eq!(content.kind, Kind::Content);
        assert_eq!(content.side, None);
        assert_eq!(content.label.as_deref(), Some("Downloaded from SMP"));
        assert_eq!(content.producer.app, App::Launcher);
        // no pack is claimed, so nothing is removed from one
        assert!(content.removed.is_empty());
        assert_eq!(content.base, None);
        // the worlds, merged as a client has them, without Paper's own files
        let data = &side.image;
        assert_eq!(
            paths(data),
            [
                "saves/world/DIM-1/region/r.0.0.mca",
                "saves/world/DIM1/region/r.0.0.mca",
                "saves/world/level.dat"
            ]
        );
        assert_eq!(data.content.as_deref(), Some(content.id.as_str()));
        assert_eq!(data.settings.server, None);
        assert_eq!(data.worlds.len(), 1);
        let withheld: Vec<&str> = data.withheld.iter().map(|w| w.path.as_str()).collect();
        assert_eq!(withheld, ["saves/world/region/r.9.9.mca"]);
        data.verify().unwrap();
        content.verify().unwrap();
        assert_eq!(side.server.server_only_mods, ["server-only.jar"]);
        // 9 content files and 6 world files; 2 mods and 3 world files taken
        assert_eq!(side.server.not_taken, 15 - 5);
    }

    #[test]
    fn forge_versions_are_full_maven_versions() {
        let (data, linked) = export(Loader::Forge, Some("47.4.28"));
        let side = client_side(&data, &linked, "x").unwrap();
        assert_eq!(
            side.image.game.loader_version.as_deref(),
            Some("1.20.1-47.4.28")
        );
        assert_eq!(side.linked[0].game, side.image.game);
        let (data, linked) = export(Loader::Forge, Some("1.20.1-47.4.28"));
        let side = client_side(&data, &linked, "x").unwrap();
        assert_eq!(
            side.image.game.loader_version.as_deref(),
            Some("1.20.1-47.4.28")
        );
    }

    #[test]
    fn without_a_content_image_the_backups_own_files_are_the_content() {
        let (mut data, _) = export(Loader::Fabric, Some("0.16.10"));
        data.content = None;
        data.seal();
        let side = client_side(&data, &[], "x").unwrap();
        assert_eq!(
            paths(&side.linked[0]),
            ["mods/both.jar", "mods/client-only.jar"]
        );
        assert_eq!(side.image.entries.len(), 3);
    }

    #[test]
    fn the_packs_share_skips_what_the_images_place_in_any_letter_case() {
        let download = |path: &str| lodestone_minecraft::mrpack::PackDownload {
            path: path.into(),
            urls: vec![
                url::Url::parse("https://cdn.modrinth.com/data/a/versions/b/c.jar").unwrap(),
            ],
            sha1: "0".repeat(40),
            sha512: "0".repeat(128),
            size: 1,
        };
        let file = |path: &str, index: usize| modpack::OverrideFile {
            path: path.into(),
            index,
            size: 1,
            sha1: "0".repeat(40),
        };
        let pack = modpack::PackInfo {
            name: "Pack".into(),
            version_number: None,
            summary: None,
            game_version: "1.20.1".into(),
            loader: None,
            recommended_memory: None,
            downloads: vec![
                download("mods/Server.jar"),
                download("mods/client.jar"),
                download("mods/gone.jar"),
                download("mods/both.jar"),
            ],
            overrides: vec![
                file("mods/both.jar", 0),
                file("config/x.toml", 1),
                file("SAVES/Tutorial/level.dat", 2),
            ],
            skipped: Vec::new(),
        };
        let game = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(game.path().join("config")).unwrap();
        std::fs::write(game.path().join("config/x.toml"), b"already there").unwrap();
        let writes = share_writes(
            &pack,
            &[CanonicalPath::new("mods/GONE.jar").unwrap()],
            &[entry("mods/server.jar")],
            game.path(),
        );
        let paths: Vec<&str> = writes.iter().map(|(_, p)| p.as_str()).collect();
        assert_eq!(paths, ["mods/client.jar", "mods/both.jar"]);
        // an override at a download's path wins, as the pack installs it
        assert!(matches!(
            writes[1].0,
            modpack::merge::Fetch::Override { .. }
        ));
    }

    #[test]
    fn a_path_outside_the_content_folders_is_never_placed() {
        let (mut data, mut linked) = export(Loader::Fabric, Some("0.16.10"));
        // the client view lets these through: a player's own files, which a server never
        // writes on a player's machine
        let mut options = sided("options.txt", EnvValue::Required, EnvValue::Unknown);
        options.env_from = vec![EnvFrom::OwnerMark];
        linked[0].entries.push(options);
        linked[0].entries.push(entry("servers.dat"));
        linked[0].normalize();
        linked[0].seal();
        data.content = Some(linked[0].id.clone());
        data.seal();
        let side = client_side(&data, &linked, "x").unwrap();
        let content = paths(&side.linked[0]);
        assert!(!content.contains(&"options.txt"), "{content:?}");
        assert!(!content.contains(&"servers.dat"), "{content:?}");
    }

    #[test]
    fn a_newer_copy_in_the_backup_keeps_the_content_images_marks_and_sides() {
        let (mut data, mut linked) = export(Loader::Fabric, Some("0.16.10"));
        // a config the owner shared with players, and the server-only mod
        let mut shared = sided("config/shared.toml", EnvValue::Required, EnvValue::Required);
        shared.env_from = vec![EnvFrom::OwnerMark];
        linked[0].entries.push(shared);
        linked[0].normalize();
        linked[0].seal();
        // the backup's copies are newer: other bytes, and the snapshot's unknown sides
        let newer = |path: &str| {
            let mut e = entry(path);
            e.sha256 = lodestone_image::hex::sha256_hex(b"newer");
            e
        };
        data.entries
            .retain(|e| e.path.as_str() != "mods/server-only.jar");
        data.entries.push(newer("config/shared.toml"));
        data.entries.push(newer("mods/server-only.jar"));
        data.content = Some(linked[0].id.clone());
        data.normalize();
        data.seal();
        let side = client_side(&data, &linked, "x").unwrap();
        let content = &side.linked[0];
        assert_eq!(
            paths(content),
            [
                "config/shared.toml",
                "mods/both.jar",
                "mods/client-only.jar"
            ]
        );
        // the backup's bytes are the ones placed
        let config = content
            .entries
            .iter()
            .find(|e| e.path.as_str() == "config/shared.toml")
            .unwrap();
        assert_eq!(config.sha256, lodestone_image::hex::sha256_hex(b"newer"));
        assert_eq!(side.server.server_only_mods, ["server-only.jar"]);
    }
}
