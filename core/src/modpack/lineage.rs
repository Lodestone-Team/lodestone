//! A server's modpack history as the lineage of its content images
//! (`docs/instance-images.md`, "M2 decisions").
//!
//! [`super::history`] stays the lineage's bookkeeping: `history.json` is the "rest" beside
//! the images (what an image doesn't model: the entry's kind, origin, changes, kept files,
//! changelog and how to get files back), and `record.json` stays the update engine's record
//! for its three-way merge. Both live in the server folder, so a backup restores them with
//! the files they describe, and cores from before images read them as they always did.
//! Each entry names its content image ([`HistoryEntry::image`]); a separate index in the
//! image store would go out of step with `history.json` after a backup restore.
//!
//! - **Writes.** A modpack install, update or restore records a content image of the whole
//!   server once it succeeded, after writing `images/<uuid>/pack.json` ([`base_info`]: the
//!   pack as the image's base and the `env` its index and override folders give each file),
//!   and links the new history entry to it. Recording never fails the operation: without
//!   an image the entry is kept as before.
//! - **Older entries.** The first time a server's history is written with entries that have
//!   no image, [`backfill`] converts them with the legacy reader
//!   (`lodestone_image::legacy::core_modpack`), storing the files it can hash (the pack's
//!   files on disk and the history's stored copies) first, and links each entry to its
//!   image. Converted images have `env` unknown and never become the current content image:
//!   that is always one the core recorded from disk.
//! - **Restoring.** An entry restores through the update engine as it always did. A file it
//!   can't get back from the history's store or a download is read from its content image
//!   ([`ImageLookup`]), checked against the entry's SHA-1.
//! - **Retention.** [`LineageRefs`] tells the image service's pruning which images the
//!   history names, so those stay besides the newest 30 and those backups link to.
//!
//! Nothing here deletes `history.json`, `record.json` or the history's stored copies:
//! downgrades and restores of old entries still need them.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};

use async_trait::async_trait;
use dashmap::DashMap;
use lodestone_image::legacy::{core_modpack, KnownFiles};
use lodestone_image::store::{Store, NEVER};
use lodestone_image::{Base, DiskLayout, Entry, Env, EnvValue};
use lodestone_minecraft::mrpack::{for_each_override, open_pack, read_index, OverrideItem, Side};
use sha1::Digest;
use tracing::{info, warn};

use super::history::{object_path, EntryFiles, History, HistoryEntry, HistoryKind};
use super::store::PackStore;
use super::update::{on_disk, FileAction, FilePlan, ImageFile, PackFile};
use super::{InstancePathPolicy, ModpackOrigin};
use crate::error::Error;
use crate::images::base::{BaseFile, BaseInfo};
use crate::images::{ImageRefs, ImageService, InstanceContext, OpKind, Operation};
use crate::implementations::minecraft::Flavour;
use crate::mods::disabled_name;
use crate::prelude::GameInstance;
use crate::traits::t_configurable::TConfigurable;
use crate::types::InstanceUuid;

/// The label of the content image an install, update or restore records, as the image
/// service's labels read ("Updated Test Pack to 2").
pub fn label(kind: &HistoryKind, origin: &ModpackOrigin) -> String {
    let version = origin.version_number.as_deref().unwrap_or("").trim();
    let pack = origin.name.trim();
    let with_version = |verb: &str, joiner: &str| {
        if version.is_empty() {
            format!("{verb} {pack}")
        } else {
            format!("{verb} {pack}{joiner}{version}")
        }
    };
    match kind {
        HistoryKind::Install => with_version("Installed", " "),
        HistoryKind::Baseline => with_version("Modpack history began with", " "),
        HistoryKind::Update => with_version("Updated", " to "),
        HistoryKind::Restore { .. } => with_version("Restored", " to "),
    }
}

// ---- pack.json ----

/// The SHA-512 of the `.mrpack` at `path`, lowercase. Blocking.
fn pack_sha512(path: &Path) -> Option<String> {
    match lodestone_minecraft::hash::sha512_of(path) {
        Ok(hash) => Some(hash.to_ascii_lowercase()),
        Err(e) => {
            warn!("Could not read the modpack {}: {e}", path.display());
            None
        }
    }
}

/// What the pack at `path` says about each file it writes into a server: an index file's
/// `env` as written, an override's from its folder. `overrides/` is for both sides,
/// `server-overrides/` for the server only (it replaces an `overrides/` file of the same
/// path there), and an `overrides/` file that `client-overrides/` replaces on clients isn't
/// what clients get, so it is unsupported on their side. Blocking.
fn pack_envs(path: &Path) -> Result<HashMap<String, Env>, Error> {
    let mut archive = open_pack(path)?;
    let index = read_index(&mut archive)?;
    let mut envs = HashMap::new();
    for file in &index.files {
        let side = |value: Option<&String>| {
            value
                .and_then(|v| EnvValue::parse_loose(v))
                .unwrap_or(EnvValue::Unknown)
        };
        let env = match &file.env {
            Some(env) => Env::new(side(env.client.as_ref()), side(env.server.as_ref())),
            None => Env::UNKNOWN,
        };
        envs.insert(file.path.clone(), env);
    }
    // Which override paths clients get from their own folder. Only compared with the
    // checked server paths below, so the names needn't be checked themselves.
    let client_only: HashSet<String> = archive
        .file_names()
        .filter_map(|name| name.ok())
        .filter_map(|name| name.strip_prefix("client-overrides/").map(str::to_string))
        .filter(|name| !name.is_empty() && !name.ends_with('/'))
        .collect();
    for_each_override(
        &mut archive,
        Side::Server,
        &InstancePathPolicy {
            allow_scripts: true,
        },
        |prefix, item| {
            if let OverrideItem::File(path, _) = item {
                let path = super::slash_path(&path);
                let client = if prefix == "server-overrides/" || client_only.contains(&path) {
                    EnvValue::Unsupported
                } else {
                    EnvValue::Required
                };
                envs.insert(path, Env::new(client, EnvValue::Required));
            }
            Ok::<_, Error>(())
        },
    )?;
    Ok(envs)
}

/// The modrinth CDN URL of a download, if it has one.
fn cdn_url(file: &PackFile) -> Option<String> {
    file.download.as_ref()?.urls.iter().find_map(|url| {
        (url.scheme() == "https" && url.host_str() == Some(crate::modrinth::CDN_HOST))
            .then(|| url.to_string())
    })
}

/// `images/<uuid>/pack.json` for a server whose pack files are now `files` (by disk path,
/// as the record has them), from the pack `origin`:
///
/// - the base is the pack (`modrinth_pack`) when it is on Modrinth and its `.mrpack` is at
///   `pack_path` (hashed here), else `fallback` when that is the same Modrinth version (the
///   base a restored entry's image had), else none;
/// - each file has its SHA-1, its download's SHA-512, size and CDN URL, and the `env` the
///   pack at `pack_path` gives it (unknown without the pack). A mod is listed under its
///   disabled name too, so a pack mod disabled in the Mods tab keeps what the pack says.
///
/// Reads the pack, so call it on a blocking thread.
pub fn base_info(
    origin: &ModpackOrigin,
    pack_path: Option<&Path>,
    files: &BTreeMap<String, PackFile>,
    fallback: Option<Base>,
) -> BaseInfo {
    let on_modrinth = !origin.project_id.is_empty() && !origin.version_id.is_empty();
    let hashed = pack_path
        .filter(|_| on_modrinth)
        .and_then(pack_sha512)
        .map(|sha512| Base::ModrinthPack {
            project: origin.project_id.clone(),
            version: origin.version_id.clone(),
            sha512,
        });
    let fallback = fallback.filter(|base| {
        on_modrinth
            && matches!(base, Base::ModrinthPack { project, version, .. }
                if *project == origin.project_id && *version == origin.version_id)
    });
    let envs = match pack_path {
        Some(path) => pack_envs(path).unwrap_or_else(|e| {
            warn!("Could not read which sides the modpack's files are for: {e}");
            HashMap::new()
        }),
        None => HashMap::new(),
    };
    let mut info = BaseInfo {
        base: hashed.or(fallback),
        files: BTreeMap::new(),
    };
    for (path, file) in files {
        let base_file = BaseFile {
            sha1: file.sha1.to_ascii_lowercase(),
            sha256: None,
            size: file.download.as_ref().map(|d| d.size),
            env: envs.get(path).copied(),
            modrinth_url: cdn_url(file),
            sha512: file
                .download
                .as_ref()
                .map(|d| d.sha512.to_ascii_lowercase()),
        };
        if path.to_ascii_lowercase().ends_with(".jar") {
            info.files.insert(disabled_name(path), base_file.clone());
        }
        info.files.insert(path.clone(), base_file);
    }
    info
}

// ---- recording ----

/// Saves `info` as the server's `pack.json`, then ends `op` by recording the server's
/// content image now. Returns the image's id, or `None` when nothing could be recorded (or
/// images aren't kept for the server), which is logged and never fails the caller.
pub async fn record(
    images: &Arc<ImageService>,
    ctx: InstanceContext,
    op: Operation,
    info: BaseInfo,
) -> Option<String> {
    save_pack(images, &ctx.uuid, info).await;
    let uuid = ctx.uuid.clone();
    match op.succeed_and_record(ctx).await {
        Ok(recorded) => recorded.map(|r| r.id),
        Err(e) => {
            warn!(
                "Could not record the content image of {uuid}; the modpack history keeps the entry without one: {}",
                e.source
            );
            None
        }
    }
}

/// [`record`] for a server that has no operation to end: a new server, whose files nobody
/// else changes yet.
pub async fn record_new(
    images: &Arc<ImageService>,
    ctx: InstanceContext,
    info: BaseInfo,
    label: String,
) -> Option<String> {
    save_pack(images, &ctx.uuid, info).await;
    let uuid = ctx.uuid.clone();
    match images.record_now(ctx, OpKind::Modpack, label).await {
        Ok(recorded) => recorded.map(|r| r.id),
        Err(e) => {
            warn!(
                "Could not record the content image of {uuid}; the modpack history keeps the entry without one: {}",
                e.source
            );
            None
        }
    }
}

/// Records the first content image of a server just made from the pack at `pack_path`
/// (its files, as installed, are `files`), before the core lists it: `pack.json` from the
/// pack, then the image, labelled "Installed <pack> <version>". Returns its id for the
/// history's Install entry; `None` when nothing could be recorded, which never fails the
/// creation.
#[allow(clippy::too_many_arguments)]
pub async fn record_install(
    state: &crate::AppState,
    uuid: &InstanceUuid,
    root: &Path,
    origin: &ModpackOrigin,
    pack_path: &Path,
    files: &BTreeMap<String, PackFile>,
    game_version: &str,
    flavour: &Flavour,
) -> Option<String> {
    if !ImageService::accepts(uuid) {
        return None;
    }
    let info = {
        let (origin, pack_path, files) = (origin.clone(), pack_path.to_path_buf(), files.clone());
        tokio::task::spawn_blocking(move || base_info(&origin, Some(&pack_path), &files, None))
            .await
            .ok()?
    };
    // pruning finds the server's history in its folder until the core lists it
    let _creating = creating(uuid, root);
    let exclude = state
        .backups
        .policy(uuid)
        .await
        .map(|policy| policy.exclude)
        .unwrap_or_default();
    let ctx = new_server_context(
        uuid.clone(),
        root.to_path_buf(),
        game_version.to_string(),
        flavour.clone(),
        exclude,
    )
    .await;
    record_new(
        state.images(),
        ctx,
        info,
        label(&HistoryKind::Install, origin),
    )
    .await
}

/// Writes `pack.json`; a failure only loses the base and `env` of the images to come.
async fn save_pack(images: &Arc<ImageService>, uuid: &InstanceUuid, info: BaseInfo) {
    if !ImageService::accepts(uuid) {
        return;
    }
    let root = images.root().to_path_buf();
    let id = uuid.clone();
    let saved =
        tokio::task::spawn_blocking(move || crate::images::base::save_base(&root, &id, &info))
            .await;
    match saved {
        Ok(Ok(())) => {}
        Ok(Err(e)) => warn!("Could not write the modpack files of {uuid}: {}", e.source),
        Err(e) => warn!("Could not write the modpack files of {uuid}: {e}"),
    }
}

/// The `level-name` of the server in `root`, or the default `world`.
pub async fn level_name(root: &Path) -> String {
    use crate::implementations::minecraft::properties::PropertiesDocument;
    let bytes = tokio::fs::read(root.join("server.properties"))
        .await
        .unwrap_or_default();
    let (doc, _) = PropertiesDocument::from_bytes(&bytes);
    doc.get("level-name")
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| crate::images::layout::DEFAULT_LEVEL_NAME.to_string())
}

/// The context of a server being created (not in the core's list yet), stopped.
pub async fn new_server_context(
    uuid: InstanceUuid,
    root: PathBuf,
    game_version: String,
    flavour: Flavour,
    policy_exclude: Vec<String>,
) -> InstanceContext {
    InstanceContext {
        uuid,
        level_name: level_name(&root).await,
        root,
        game_version,
        flavour,
        policy_exclude,
        running: false,
    }
}

/// What an update or restore learned before it changed anything.
#[derive(Debug, Clone, Default)]
pub struct Before {
    /// The current content image before the change: a baseline entry's image.
    pub current: Option<String>,
    /// The images [`backfill`] linked older entries to, by entry id.
    pub backfilled: BTreeMap<u32, String>,
}

impl Before {
    /// Links the entries of `history` that `backfilled` names and that still have no image.
    pub fn link(&self, history: &mut History) {
        for entry in &mut history.entries {
            if entry.image.is_none() {
                entry.image = self.backfilled.get(&entry.id).cloned();
            }
        }
    }
}

/// Before an update or restore of the server `ctx` changes its files: records what the Mods
/// tab or the Files API changed in the last seconds (so the current image is the content
/// as it is), notes the current image, and converts the history's older entries
/// ([`backfill`]). Best effort; never fails.
pub async fn before_change(
    images: &Arc<ImageService>,
    ctx: &InstanceContext,
    packs: Option<&PackStore>,
) -> Before {
    if !ImageService::accepts(&ctx.uuid) {
        return Before::default();
    }
    if let Err(e) = images.flush(ctx.clone()).await {
        warn!(
            "Could not record the content of {} before its modpack changed: {}",
            ctx.uuid, e.source
        );
    }
    Before {
        current: images.current_content(&ctx.uuid).await,
        backfilled: backfill(images, ctx, packs).await,
    }
}

// ---- converting older entries ----

/// Links the entries of the history of the server `ctx` that have no content image to
/// images converted from the history itself (see the module), once: the first time the
/// history is written with such entries. `packs` are the downloaded packs, for their
/// `.mrpack` hashes. Best effort: never blocks or fails an update; what can't be converted
/// stays without an image. Returns the links it made, by entry id.
pub async fn backfill(
    images: &Arc<ImageService>,
    ctx: &InstanceContext,
    packs: Option<&PackStore>,
) -> BTreeMap<u32, String> {
    if !ImageService::accepts(&ctx.uuid) {
        return BTreeMap::new();
    }
    let history = match History::load_checked(&ctx.root).await {
        Ok(history) => history,
        Err(e) => {
            warn!(
                "Not converting the modpack history of {}: {}",
                ctx.uuid, e.source
            );
            return BTreeMap::new();
        }
    };
    if history.entries.iter().all(|e| e.image.is_some()) {
        return BTreeMap::new();
    }
    // The `.mrpack` hashes Lodestone has: the pack the server's images are built on now,
    // and packs downloaded in the last hour.
    let current_base = {
        let root = images.root().to_path_buf();
        let uuid = ctx.uuid.clone();
        tokio::task::spawn_blocking(move || crate::images::base::load_base(&root, &uuid))
            .await
            .ok()
            .flatten()
            .and_then(|info| info.base)
    };
    let mut pack_paths = Vec::new();
    let versions: BTreeSet<(String, String)> = history
        .entries
        .iter()
        .filter(|e| !e.origin.project_id.is_empty() && !e.origin.version_id.is_empty())
        .map(|e| (e.origin.project_id.clone(), e.origin.version_id.clone()))
        .collect();
    let mut known_packs = Vec::new();
    for (project, version) in versions {
        match &current_base {
            Some(Base::ModrinthPack {
                project: p,
                version: v,
                sha512,
            }) if *p == project && *v == version => {
                known_packs.push((project, version, sha512.clone()));
            }
            _ => {
                if let Some(stored) = packs.and_then(|packs| packs.find_version(&version)) {
                    pack_paths.push((project, version, stored));
                }
            }
        }
    }

    let store = images.store();
    let converting = {
        let ctx = ctx.clone();
        let history = history.clone();
        tokio::task::spawn_blocking(move || {
            convert(&store, &ctx, &history, known_packs, pack_paths)
        })
        .await
    };
    let lineage = match converting {
        Ok(Ok(lineage)) => lineage,
        Ok(Err(e)) => {
            warn!(
                "Could not convert the modpack history of {}: {}",
                ctx.uuid, e.source
            );
            return BTreeMap::new();
        }
        Err(e) => {
            warn!("Could not convert the modpack history of {}: {e}", ctx.uuid);
            return BTreeMap::new();
        }
    };

    // Save the images and link them under the instance's lock, so pruning never sees an
    // image saved but not yet named by the history.
    let _lock = images.instance_lock(&ctx.uuid).await;
    let store = images.store();
    let uuid = ctx.uuid.clone();
    let without_image: HashSet<u32> = history
        .entries
        .iter()
        .filter(|e| e.image.is_none())
        .map(|e| e.id)
        .collect();
    let saving = tokio::task::spawn_blocking(move || {
        let by_id: HashMap<&str, &lodestone_image::Image> =
            lineage.images.iter().map(|i| (i.id.as_str(), i)).collect();
        let mut saved: HashMap<String, bool> = HashMap::new();
        let mut links = BTreeMap::new();
        for rest in &lineage.rest.entries {
            if !without_image.contains(&rest.id) {
                continue;
            }
            let ok = *saved.entry(rest.image.clone()).or_insert_with(|| {
                let Some(image) = by_id.get(rest.image.as_str()) else {
                    return false;
                };
                match store.save_manifest(uuid.as_ref(), image) {
                    Ok(_) => true,
                    Err(e) => {
                        warn!(
                            "Could not save the image of modpack history entry {} of {uuid}: {e}",
                            rest.id
                        );
                        false
                    }
                }
            });
            if ok {
                links.insert(rest.id, rest.image.clone());
            }
        }
        links
    })
    .await;
    let links = match saving {
        Ok(links) => links,
        Err(e) => {
            warn!(
                "Could not save the converted modpack history of {}: {e}",
                ctx.uuid
            );
            return BTreeMap::new();
        }
    };
    if links.is_empty() {
        return links;
    }
    let mut linked = history;
    Before {
        current: None,
        backfilled: links.clone(),
    }
    .link(&mut linked);
    if let Err(e) = linked.save(&ctx.root).await {
        // The images stay until pruning finds nothing names them; the next write converts
        // the entries again, to the same ids.
        warn!(
            "Could not link the modpack history of {} to its images: {}",
            ctx.uuid, e.source
        );
        return BTreeMap::new();
    }
    info!(
        "Linked {} older modpack history entries of {} to content images",
        links.len(),
        ctx.uuid
    );
    links
}

/// Hashes and stores what the history's entries had that is still on the server or in the
/// history's store, and converts the history (blocking; see [`backfill`]).
fn convert(
    store: &Store,
    ctx: &InstanceContext,
    history: &History,
    known_packs: Vec<(String, String, String)>,
    pack_paths: Vec<(String, String, super::store::StoredPack)>,
) -> Result<core_modpack::Lineage, Error> {
    let mut known = KnownFiles::new();
    for (project, version, sha512) in known_packs {
        known.add_pack(&project, &version, &sha512);
    }
    for (project, version, stored) in pack_paths {
        if let Some(sha512) = pack_sha512(&stored.path()) {
            known.add_pack(&project, &version, &sha512);
        }
    }
    // Every SHA-1 the history names.
    let mut wanted: BTreeSet<String> = BTreeSet::new();
    let mut paths: BTreeSet<&str> = BTreeSet::new();
    for entry in &history.entries {
        for (path, file) in &entry.files {
            wanted.insert(file.sha1.to_ascii_lowercase());
            paths.insert(path);
        }
        for change in &entry.changes {
            wanted.extend(change.old_sha1.iter().map(|s| s.to_ascii_lowercase()));
            wanted.extend(change.new_sha1.iter().map(|s| s.to_ascii_lowercase()));
        }
    }
    let mut found: HashSet<String> = HashSet::new();
    // The history's stored copies first: they are named by their SHA-1, checked here.
    for sha1 in &wanted {
        if let Some(object) = object_path(&ctx.root, sha1) {
            if object.is_file() {
                store_known(store, &object, Some(sha1), &mut known, &mut found);
            }
        }
    }
    // Then the pack's files on the server (or their disabled names).
    for path in paths {
        if super::update::record_path(path).is_none() {
            continue;
        }
        let Some((target, sha1)) = on_disk(&ctx.root, path) else {
            continue;
        };
        let sha1 = sha1.to_ascii_lowercase();
        if !wanted.contains(&sha1) || found.contains(&sha1) {
            continue;
        }
        store_known(
            store,
            &ctx.root.join(&target),
            Some(&sha1),
            &mut known,
            &mut found,
        );
    }
    let (layout, _) =
        crate::images::layout::server_layout(&ctx.root, &ctx.level_name, &ctx.flavour)?;
    let DiskLayout::Server(layout) = layout else {
        return Err(color_eyre::eyre::eyre!("A server has a server's layout").into());
    };
    let bytes = serde_json::to_vec(history)
        .map_err(|e| color_eyre::eyre::eyre!("Failed to read the modpack history: {e}"))?;
    let lineage = core_modpack::read(None, Some(&bytes), &layout, &known)
        .map_err(|e| color_eyre::eyre::eyre!("The modpack history can't be converted: {e}"))?;
    let withheld: usize = lineage.images.iter().map(|i| i.withheld.len()).sum();
    if withheld > 0 {
        info!(
            "{withheld} files of the older modpack history of {} could not be put in its images (their bytes are gone, or they are world files)",
            ctx.uuid
        );
    }
    Ok(lineage)
}

/// Stores the file at `path` and notes its hashes in `known`, when it is still the file
/// with `sha1`: its SHA-1 is read, then the store reads it, failing if it changes meanwhile.
fn store_known(
    store: &Store,
    path: &Path,
    sha1: Option<&str>,
    known: &mut KnownFiles,
    found: &mut HashSet<String>,
) {
    let Ok(before) = std::fs::symlink_metadata(path) else {
        return;
    };
    if !before.is_file() {
        return;
    }
    let Ok(actual) = lodestone_minecraft::hash::sha1_of(path) else {
        return;
    };
    let actual = actual.to_ascii_lowercase();
    if sha1.is_some_and(|sha1| sha1 != actual) {
        return;
    }
    let stored = match store.put_file(path, NEVER) {
        Ok(stored) => stored,
        Err(e) => {
            warn!(
                "Could not keep {} for the modpack history: {e}",
                path.display()
            );
            return;
        }
    };
    // The file the store read must be the one hashed above.
    let unchanged = std::fs::symlink_metadata(path).is_ok_and(|after| {
        after.len() == before.len()
            && after.len() == stored.size
            && after.modified().ok() == before.modified().ok()
    });
    if unchanged {
        known.add_file(&actual, &stored.sha256, stored.size);
        found.insert(actual);
    }
}

// ---- restoring from images ----

/// Whether `id` can be an image id (64 lowercase hex), before it is used in a path.
fn is_image_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The files of history entries' content images, read from the image store. Remembers what
/// it read, so asking about every entry of a history reads each manifest once and hashes
/// each object at most once. Blocking.
pub struct ImageLookup {
    store: Arc<Store>,
    instance: String,
    /// Whether images are kept for the instance at all.
    kept: bool,
    manifests: RefCell<HashMap<String, Option<Arc<HashMap<String, Entry>>>>>,
    sha1s: RefCell<HashMap<String, Option<String>>>,
}

impl ImageLookup {
    pub fn new(store: Arc<Store>, uuid: &InstanceUuid) -> ImageLookup {
        ImageLookup {
            store,
            instance: uuid.as_ref().to_string(),
            kept: ImageService::accepts(uuid),
            manifests: Default::default(),
            sha1s: Default::default(),
        }
    }

    /// The entries of image `id`, by path; `None` when it can't be read.
    fn entries(&self, id: &str) -> Option<Arc<HashMap<String, Entry>>> {
        if let Some(cached) = self.manifests.borrow().get(id) {
            return cached.clone();
        }
        let entries = if self.kept && is_image_id(id) {
            match self.store.load_manifest(&self.instance, id) {
                Ok(image) => Some(Arc::new(
                    image
                        .entries
                        .into_iter()
                        .map(|e| (e.path.to_string(), e))
                        .collect(),
                )),
                Err(e) => {
                    warn!(
                        "Could not read the content image {id} of {}: {e}",
                        self.instance
                    );
                    None
                }
            }
        } else {
            None
        };
        self.manifests
            .borrow_mut()
            .insert(id.to_string(), entries.clone());
        entries
    }

    /// The SHA-1 of the object for `entry`, read and checked by the store.
    fn sha1_of(&self, entry: &Entry) -> Option<String> {
        if let Some(sha1) = &entry.sha1 {
            return Some(sha1.to_ascii_lowercase());
        }
        if let Some(cached) = self.sha1s.borrow().get(&entry.sha256) {
            return cached.clone();
        }
        struct Hashing(sha1::Sha1);
        impl std::io::Write for Hashing {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.update(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut hashing = Hashing(sha1::Sha1::new());
        let sha1 = self
            .store
            .read_object(&entry.sha256, entry.size, &mut hashing, NEVER)
            .ok()
            .map(|()| {
                hashing
                    .0
                    .finalize()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()
            });
        self.sha1s
            .borrow_mut()
            .insert(entry.sha256.clone(), sha1.clone());
        sha1
    }

    /// The object of the file `entry` had at `path` (a pack path; a mod may be in the image
    /// under its disabled name), when the entry's image has it with this `sha1` and the
    /// store has all of it.
    pub fn file(&self, entry: &HistoryEntry, path: &str, sha1: &str) -> Option<ImageFile> {
        let entries = self.entries(entry.image.as_deref()?)?;
        let mut candidates = vec![path.to_string()];
        if path.to_ascii_lowercase().ends_with(".jar") {
            candidates.push(disabled_name(path));
        }
        candidates.iter().find_map(|candidate| {
            let found = entries.get(candidate)?;
            if !self.store.object_complete(&found.sha256, found.size) {
                return None;
            }
            self.sha1_of(found)
                .filter(|have| have.eq_ignore_ascii_case(sha1))
                .map(|_| ImageFile {
                    sha256: found.sha256.clone(),
                    size: found.size,
                })
        })
    }
}

impl EntryFiles for ImageLookup {
    fn can_get_back(&self, entry: &HistoryEntry, path: &str, sha1: &str) -> bool {
        self.file(entry, path, sha1).is_some()
    }
}

/// Gives each file a restore of `entry` must write (an add or a replace in `plan`) that has
/// neither a stored copy nor a download the copy its content image has, when that is the
/// same file ([`ImageLookup`]). Reads the image store, so call it on a blocking thread.
pub fn fill_from_image(
    lookup: &ImageLookup,
    entry: &HistoryEntry,
    plan: &FilePlan,
    files: &mut BTreeMap<String, PackFile>,
) {
    if entry.image.is_none() {
        return;
    }
    for change in plan
        .changes
        .iter()
        .filter(|c| c.action != FileAction::Remove)
    {
        let Some(file) = files.get_mut(&change.path) else {
            continue;
        };
        if file.stored.is_some() || file.download.is_some() {
            continue;
        }
        file.image = lookup.file(entry, &change.path, &file.sha1);
    }
}

/// The SHA-512 of the file `image` stands for, read and checked by `store`: what a restore
/// asks Modrinth about before it puts a jar back for a user who can't write global files.
/// Blocking.
pub fn image_sha512(store: &Store, image: &ImageFile) -> Option<String> {
    struct Hashing(sha2::Sha512);
    impl std::io::Write for Hashing {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            sha2::Digest::update(&mut self.0, bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut hashing = Hashing(<sha2::Sha512 as sha2::Digest>::new());
    store
        .read_object(&image.sha256, image.size, &mut hashing, NEVER)
        .ok()?;
    Some(
        sha2::Digest::finalize(hashing.0)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect(),
    )
}

/// The base a history entry's content image had, for a restore whose pack Lodestone no
/// longer has. Blocking.
pub fn image_base(store: &Store, uuid: &InstanceUuid, entry: &HistoryEntry) -> Option<Base> {
    let id = entry.image.as_deref().filter(|id| is_image_id(id))?;
    if !ImageService::accepts(uuid) {
        return None;
    }
    store.load_manifest(uuid.as_ref(), id).ok()?.base
}

// ---- retention ----

/// Servers being created, which the core doesn't list yet, by uuid: where their history is.
fn creating_roots() -> &'static StdMutex<HashMap<InstanceUuid, PathBuf>> {
    static CREATING: OnceLock<StdMutex<HashMap<InstanceUuid, PathBuf>>> = OnceLock::new();
    CREATING.get_or_init(Default::default)
}

/// While it lives, [`LineageRefs`] finds the history of a server being created at its
/// folder. See [`creating`].
pub struct Creating(InstanceUuid);

impl Drop for Creating {
    fn drop(&mut self) {
        creating_roots()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.0);
    }
}

/// Notes that the server `uuid` is being created in `root`, until the guard is dropped.
pub fn creating(uuid: &InstanceUuid, root: &Path) -> Creating {
    creating_roots()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(uuid.clone(), root.to_path_buf());
    Creating(uuid.clone())
}

/// The images each server's modpack history names, for the image service's pruning
/// ([`ImageRefs`]): every entry's image, read from `history.json`. A history that can't be
/// read, or a server the core doesn't know, is an error, so pruning deletes nothing rather
/// than an image the history may still name.
pub struct LineageRefs {
    instances: Arc<DashMap<InstanceUuid, GameInstance>>,
}

impl LineageRefs {
    pub fn new(instances: Arc<DashMap<InstanceUuid, GameInstance>>) -> LineageRefs {
        LineageRefs { instances }
    }
}

#[async_trait]
impl ImageRefs for LineageRefs {
    async fn refs(&self, uuid: &InstanceUuid) -> Result<HashSet<String>, Error> {
        let instance = self.instances.get(uuid).map(|i| i.value().clone());
        let root = match instance {
            Some(GameInstance::MinecraftInstance(instance)) => instance.path().await,
            None => creating_roots()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(uuid)
                .cloned()
                .ok_or_else(|| color_eyre::eyre::eyre!("There is no server {uuid}"))?,
        };
        let history = History::load_checked(&root).await?;
        Ok(history
            .entries
            .into_iter()
            .filter_map(|entry| entry.image)
            .collect())
    }
}

#[cfg(test)]
mod tests;
