//! Snapshots: data and full images of an instance, kept in the image store with its content
//! images (`docs/instance-images.md`, "M5 decisions").
//!
//! A snapshot is usually a **data image**: everything in the game folder that isn't content
//! ([`super::scope::ContentScope::data_accepts`]: worlds, `options.txt`, `servers.dat`,
//! screenshots, files no pack lists), linked (`content`) to the instance's current content
//! image, which holds the mods, configs and packs it ran with. Either way it covers the
//! whole game folder. When the content image can't be brought up to date first
//! (recording failed, the store has none, a "Find it for me" search has the mods renamed),
//! it is a **full image** of the whole game folder instead, which needs no link.
//!
//! **Records.** Images are named by their contents, so two snapshots of an instance that
//! didn't change between them are one image, while each keeps its own name, kind, pin and
//! time: `images/<instance>/snapshots.json` lists the records, each naming its image and the
//! content it links to, with the figures the list shows, so listing them (and retention's
//! listing of the store, `Images::listing`) never reads a snapshot's manifest. A data or full
//! image's manifest is deleted only when no record names it any more. Before listing,
//! retention or a delete, the records are brought in line with the manifests: a record
//! whose manifest is gone is dropped (a crash in the middle of a delete), and a data or full
//! manifest no record names (a crash between the manifest and its record) is listed again
//! as a manual snapshot, named by its label, so retention never deletes it. A
//! `snapshots.json` that is there but can't be read as one is renamed aside
//! (`snapshots.damaged-<unix>.json`) and rebuilt the same way: every snapshot comes back as
//! a manual one, so kinds and pins are lost but no snapshot is.
//!
//! **Taking one** ([`Images::take_snapshot`]), under the instance's record lock and inside
//! a counted write, never while its game runs: the content first ([`Images`]'s content
//! step: the record before a change, which takes in what is queued, a change by hand and
//! what the game wrote since the last record), so the link names what is on disk; then a snapshot with the `data`
//! stat cache, the free-space check (what it will read, the records under way, and 1 GiB
//! to spare), the manifest (retried once when a collection took an object meanwhile), the
//! record, the stat cache, and retention. An automatic snapshot whose image is the newest
//! record's writes nothing but the stat cache. A crash leaves at worst a manifest no record
//! names, which comes back as a manual snapshot.

use std::path::PathBuf;
use std::sync::Arc;

use lodestone_image::layout::CLIENT_GAME_FOLDER;
use lodestone_image::manifest::App;
use lodestone_image::store::snapshot::{self, Candidate, Snapshot, SnapshotOptions};
use lodestone_image::store::{Cancel, NEVER, StoreError};
use lodestone_image::{DiskLayout, Image, Kind, Producer};
use serde::{Deserialize, Serialize};

use super::context::InstanceContext;
use super::labels;
use super::retention::{SnapshotPolicy, data_to_delete};
use super::scope::full_filter;
use super::state;
use super::{ImageError, Images, Slot};
use crate::instances::Loader;

/// The records file in an instance's image folder.
pub const RECORDS_FILE: &str = "snapshots.json";
/// A records file is small; one larger than this is damaged.
const MAX_RECORDS_BYTES: u64 = 16 << 20;
/// The most records one instance keeps: far more than retention leaves, and a bound on
/// what a damaged or hostile file can make listing hold.
pub const MAX_RECORDS: usize = 10_000;
/// The records file's format.
const RECORDS_VERSION: u32 = 1;
/// The longest a snapshot's name may be, in characters.
pub const MAX_NAME_CHARS: usize = 200;

/// Why a snapshot was taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotKind {
    /// Taken by the user (the Snapshots list, "Back up" in the Worlds tab, the CLI).
    Manual,
    /// Before a risky change: a content, modpack or server update, a loader change.
    BeforeChange,
    /// After a play session, when the setting is on.
    AfterPlay,
    /// The safety snapshot before going back to another one.
    BeforeRestore,
    /// What an instance made from an image export started as.
    Imported,
}

impl SnapshotKind {
    /// Whether the Launcher took it on its own (before a change or after playing): those
    /// are the ones retention's buckets thin out.
    pub fn automatic(self) -> bool {
        matches!(self, SnapshotKind::BeforeChange | SnapshotKind::AfterPlay)
    }

    /// Whether Settings › Storage's clean-up may delete it: automatic and safety
    /// snapshots, never one the user took or imported, nor a pinned one.
    pub fn removable(self) -> bool {
        self.automatic() || self == SnapshotKind::BeforeRestore
    }
}

/// One snapshot of an instance (see the module).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRecord {
    /// `YYYYMMDD-HHMMSS-xxxx` (UTC, then four random hex digits).
    pub id: String,
    /// The data or full image it names.
    pub image: String,
    pub kind: SnapshotKind,
    /// What the user called it, or the label of an automatic one. Plain text, any
    /// character: show it through [`Self::name_shown`].
    pub name: String,
    /// Unix seconds.
    pub created: i64,
    #[serde(default)]
    pub pinned: bool,
    /// A full image (the whole game folder) rather than a data image linked to content.
    #[serde(default)]
    pub full: bool,
    /// The world folders it holds.
    #[serde(default)]
    pub worlds: Vec<String>,
    /// Each world's own name and size, as the image has them, for lists that show a world
    /// by its name and a snapshot's share of one world.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub world_details: Vec<RecordWorld>,
    /// The content image a data image links to.
    #[serde(default)]
    pub content: Option<String>,
    /// How many files it lists.
    #[serde(default)]
    pub files: usize,
    /// How many files it couldn't carry.
    #[serde(default)]
    pub withheld: usize,
    /// The bytes of its files.
    #[serde(default)]
    pub bytes: u64,
    /// What it added to the store when it was taken.
    #[serde(default)]
    pub new_bytes: u64,
}

/// One world of a snapshot ([`SnapshotRecord::world_details`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordWorld {
    /// Its folder in `saves/`.
    pub name: String,
    /// Its own name (`level.dat`'s), when read: show it through `display_text`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level_name: Option<String>,
    /// The bytes of its files.
    #[serde(default)]
    pub size: u64,
}

/// The [`RecordWorld`]s of `image`'s worlds.
pub(crate) fn record_worlds(image: &Image) -> Vec<RecordWorld> {
    image
        .worlds
        .iter()
        .map(|w| RecordWorld {
            name: w.name.clone(),
            level_name: w.level_name.clone(),
            size: w.size.unwrap_or_default(),
        })
        .collect()
}

impl SnapshotRecord {
    /// The world in the folder `folder`, by its own name when the snapshot read it.
    pub fn world_name(&self, folder: &str) -> String {
        self.world_details
            .iter()
            .find(|w| w.name == folder)
            .and_then(|w| w.level_name.clone())
            .unwrap_or_else(|| folder.to_string())
    }

    /// The bytes of the world in the folder `folder`, when known.
    pub fn world_size(&self, folder: &str) -> Option<u64> {
        self.world_details
            .iter()
            .find(|w| w.name == folder)
            .map(|w| w.size)
    }

    /// The name to show: control and invisible characters written visibly
    /// (`lodestone_image::display_text`), since an imported snapshot's name came from
    /// someone else.
    pub fn name_shown(&self) -> String {
        lodestone_image::display_text(&self.name).into_owned()
    }
}

/// An instance's snapshots.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Snapshots {
    /// Newest first.
    pub records: Vec<SnapshotRecord>,
    /// Manifests of the instance that can't be read, by image id, with why: while there is
    /// one, no content image is pruned, so the list offers to delete them.
    pub unreadable: Vec<(String, String)>,
}

/// The records file.
#[derive(Debug, Default, Serialize, Deserialize)]
struct RecordsFile {
    #[serde(default)]
    version: u32,
    /// The instance's name when the list was last written, for the deleted instance's
    /// marker ([`super::retired`]), when `instance.json` is gone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    instance_name: Option<String>,
    #[serde(default)]
    snapshots: Vec<SnapshotRecord>,
}

/// What to take.
pub struct Take<'a> {
    pub kind: SnapshotKind,
    /// The snapshot's name (cut to [`MAX_NAME_CHARS`]); `None` or blank is
    /// [`labels::SNAPSHOT`].
    pub name: Option<String>,
    /// (bytes read, bytes to read) while files are read.
    pub progress: &'a (dyn Fn(u64, u64) + Sync),
    pub cancel: Cancel<'a>,
}

impl<'a> Take<'a> {
    /// A snapshot of `kind` named `name`, with no progress and no way to cancel.
    pub fn new(kind: SnapshotKind, name: impl Into<String>) -> Take<'a> {
        Take {
            kind,
            name: Some(name.into()),
            progress: &|_, _| {},
            cancel: NEVER,
        }
    }
}

/// A change about to replace an instance's files, which a snapshot comes before
/// ([`Images::before_risky_change`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskyChange<'a> {
    /// The instance's modpack updating to `version`.
    ModpackUpdate { title: &'a str, version: &'a str },
    /// A server's update applied to the instance (milestone 4's second half calls this
    /// before `servers/mod.rs`'s `apply_sync` brings the mods in step).
    ServerUpdate { server: &'a str },
    /// Content updating (Update all, one update): the titles updating.
    ContentUpdate { titles: &'a [String] },
    /// The loader's version changing in place.
    LoaderChange { loader: Loader, version: &'a str },
    /// The instance moving to another Minecraft version or loader in place (the Launcher's
    /// own screens never do; a caller that does takes this first): the next launch
    /// upgrades its worlds, which can't be undone.
    GameChange {
        game_version: &'a str,
        loader: Loader,
        loader_version: Option<&'a str>,
    },
}

impl RiskyChange<'_> {
    /// The snapshot's name.
    pub fn name(&self) -> String {
        match self {
            RiskyChange::ModpackUpdate { title, version } => {
                labels::before_pack_update(title, version)
            }
            RiskyChange::ServerUpdate { server } => labels::before_server_update(server),
            RiskyChange::ContentUpdate { titles } => labels::before_content_update(titles),
            RiskyChange::LoaderChange { loader, version } => {
                labels::before_loader_change(*loader, version)
            }
            RiskyChange::GameChange {
                game_version,
                loader,
                loader_version,
            } => labels::before_game_change(game_version, *loader, *loader_version),
        }
    }

    /// The label of the content record before it when the game's writes are the only
    /// change, for a change that merges configs (a pack's or server's update). `None`: the
    /// record is labelled "Changed while playing" then, as for any snapshot.
    fn content_record(&self) -> Option<&'static str> {
        match self {
            RiskyChange::ModpackUpdate { .. } => Some(labels::BEFORE_MODPACK_UPDATE),
            RiskyChange::ServerUpdate { .. } => Some(labels::BEFORE_SERVER_UPDATE),
            RiskyChange::ContentUpdate { .. }
            | RiskyChange::LoaderChange { .. }
            | RiskyChange::GameChange { .. } => None,
        }
    }
}

impl Images {
    /// The instance's snapshots, newest first, and its manifests that can't be read.
    /// Brings the records in line with the manifests first (see the module) when it can
    /// have the record lock within [`super::history::HISTORY_WAIT`]; otherwise (a long
    /// record runs) lists the file as it is. An instance whose images are an earlier
    /// instance's of the same id has none. Blocking.
    pub fn snapshots(&self, instance: &str) -> Result<Snapshots, ImageError> {
        let ctx = InstanceContext::load(self.instances(), instance)?;
        if !self.accepts(instance) {
            return Ok(Snapshots::default());
        }
        let slot = self.slot(instance);
        let locked = super::check::lock_within(&slot.record, super::history::HISTORY_WAIT);
        if locked.is_some()
            && let Ok(_writing) = self.enter()
        {
            self.readopt_or_warn(&ctx);
        }
        let loaded = state::load_present(self.store(), instance)?;
        let listing = self.listing(instance)?;
        let has_manifests = !listing.content.is_empty() || !listing.links.is_empty();
        if self.retired_marker_in(instance)
            || state::belongs_elsewhere(&loaded, has_manifests, ctx.instance.created)
        {
            return Ok(Snapshots::default());
        }
        let records = match (&locked, self.enter()) {
            (Some(_), Ok(_writing)) => self.reconciled_locked(instance)?,
            _ => self.load_records(instance)?.unwrap_or_default(),
        };
        Ok(Snapshots {
            records,
            unreadable: listing.unreadable,
        })
    }

    /// One snapshot record of the instance.
    pub fn snapshot(&self, instance: &str, id: &str) -> Result<SnapshotRecord, ImageError> {
        self.snapshots(instance)?
            .records
            .into_iter()
            .find(|record| record.id == id)
            .ok_or_else(|| ImageError::NoSnapshot(id.to_string()))
    }

    /// Takes a snapshot of the instance (see the module). `None` when it is automatic and
    /// nothing changed since the newest snapshot. Refused while the game runs, while the
    /// store is suspended, and when the disk lacks room for it and 1 GiB to spare.
    /// Blocking.
    pub fn take_snapshot(
        &self,
        instance: &str,
        take: Take,
    ) -> Result<Option<SnapshotRecord>, ImageError> {
        self.take_snapshot_with(instance, take, None)
    }

    /// Before a change that replaces the instance's files: a snapshot of what it is about
    /// to change ([`SnapshotKind::BeforeChange`], named after the change), whose content
    /// image, recorded first, holds every file the change replaces. The one hook for every
    /// risky change: a content update, a modpack update, a loader change, and a server's
    /// update (milestone 4's second half is to call it in `apply_sync` in place of
    /// `record_before_change`). `None` when nothing changed since the newest snapshot.
    /// What a failure does is the caller's: a modpack update stops, and the user can update
    /// without a snapshot; a content update or a loader change goes on and says so in its
    /// notice. Blocking.
    pub fn before_risky_change(
        &self,
        instance: &str,
        change: RiskyChange,
    ) -> Result<Option<SnapshotRecord>, ImageError> {
        self.before_risky_change_with(instance, change, &|_, _| {}, NEVER)
    }

    /// [`Self::before_risky_change`], showing (bytes read, bytes to read) while the files are
    /// read and stopping with [`ImageError::Cancelled`] when `cancel` says so: the first
    /// snapshot of a big world reads all of it, so the change's own progress and Cancel
    /// cover it. A cancelled snapshot records nothing; the caller cancels its change too.
    pub fn before_risky_change_with(
        &self,
        instance: &str,
        change: RiskyChange,
        progress: &(dyn Fn(u64, u64) + Sync),
        cancel: Cancel,
    ) -> Result<Option<SnapshotRecord>, ImageError> {
        let take = Take {
            progress,
            cancel,
            ..Take::new(SnapshotKind::BeforeChange, change.name())
        };
        self.take_snapshot_with(instance, take, change.content_record())
    }

    /// [`Self::take_snapshot`], with the label of the content record before it when the
    /// change about to be made replaces files the game may have written.
    pub(crate) fn take_snapshot_with(
        &self,
        instance: &str,
        take: Take,
        content_record: Option<&str>,
    ) -> Result<Option<SnapshotRecord>, ImageError> {
        if !self.accepts(instance) {
            return Ok(None);
        }
        let slot = self.slot(instance);
        let _record = slot.lock_record();
        let _writing = self.enter()?;
        self.take_snapshot_locked(&slot, instance, take, content_record)
    }

    /// [`Self::take_snapshot_with`], for a caller holding the record lock inside a counted
    /// write (a restore's safety snapshot).
    pub(crate) fn take_snapshot_locked(
        &self,
        slot: &Slot,
        instance: &str,
        take: Take,
        content_record: Option<&str>,
    ) -> Result<Option<SnapshotRecord>, ImageError> {
        if slot.in_session() {
            return Err(ImageError::Running);
        }
        let ctx = InstanceContext::load(self.instances(), instance)?;
        // A game this process didn't start (the Launcher was restarted while it ran) still
        // has its worlds open: a snapshot would read them mid-write.
        if crate::worlds::open_in_game(&ctx.dir.join(CLIENT_GAME_FOLDER).join("saves"), None)
            .is_some()
        {
            return Err(ImageError::Running);
        }
        let loaded = self.guarded(&ctx)?;
        if !loaded.present {
            // The state names the instance before any manifest is written, so the
            // id-reuse guard never takes this snapshot for an earlier instance's.
            let mut first = loaded.state;
            first.instance_created = Some(ctx.instance.created);
            state::save(self.store(), instance, &first)?;
        }
        let content_ok = match self.content_step_locked(slot, instance, content_record) {
            Ok(ok) => ok,
            Err(e) => {
                super::warn(
                    "images.snapshot",
                    instance,
                    &format!(
                        "the content couldn't be recorded first, so the snapshot holds everything: {e}"
                    ),
                );
                false
            }
        };
        let pointer = state::load_present(self.store(), instance)?
            .state
            .current
            .filter(|id| {
                self.store()
                    .manifest_path(instance, id)
                    .is_ok_and(|path| path.is_file())
            });
        let content = pointer.filter(|_| content_ok);
        let mut records = self.reconciled_locked(instance)?;
        let name = snapshot_name(take.name.as_deref());
        // A session that begins meanwhile (Play pressed again while the snapshot after the
        // last one reads a big world: Play waits only so long for the record lock) stops
        // the snapshot at the next file, so the game never writes what it is reading. A
        // session can't end while this holds the record lock, so one seen at the end began
        // during it.
        let caller_cancel = take.cancel;
        let stop = || caller_cancel() || slot.in_session();
        let take = Take {
            cancel: &stop,
            ..take
        };
        let mut retried = false;
        let (image, snapshot) = loop {
            let built = self.build_snapshot(&ctx, &take, content.clone(), &records, &name, None);
            if slot.in_session() {
                return Err(ImageError::Running);
            }
            let (image, snapshot) = built?;
            let automatic = take.kind.automatic();
            if automatic
                && records
                    .first()
                    .is_some_and(|newest| newest.image == image.id)
            {
                // Nothing changed since the newest snapshot: only the stat cache.
                self.save_data_cache(instance, &snapshot);
                return Ok(None);
            }
            let have = self
                .store()
                .manifest_path(instance, &image.id)
                .is_ok_and(|path| path.is_file());
            if have {
                // Another record names the same contents: they share the image.
                break (image, snapshot);
            }
            match self.store().save_manifest(instance, &image) {
                Ok(_) => break (image, snapshot),
                Err(StoreError::Missing(sha256)) if !retried => {
                    super::warn(
                        "images.snapshot",
                        instance,
                        &format!(
                            "an object was collected while taking the snapshot ({sha256}); taking it again"
                        ),
                    );
                    retried = true;
                }
                Err(e) => return Err(e.into()),
            }
        };
        let record = SnapshotRecord {
            id: new_record_id(image.created, &records),
            image: image.id.clone(),
            kind: take.kind,
            name,
            created: image.created,
            pinned: false,
            full: image.kind == Kind::Full,
            worlds: image.worlds.iter().map(|w| w.name.clone()).collect(),
            world_details: record_worlds(&image),
            content: image.content.clone(),
            files: image.entries.len(),
            withheld: image.withheld.len(),
            bytes: lodestone_image::container::files_size(&image),
            new_bytes: snapshot.bytes_added,
        };
        records.insert(0, record.clone());
        self.save_records(instance, &records)?;
        self.save_data_cache(instance, &snapshot);
        if let Err(e) = self.apply_retention_locked(&ctx, &mut records, Some(&record.id)) {
            super::warn("images.retention", instance, &e.to_string());
        }
        Ok(Some(record))
    }

    /// Snapshots the instance's game folder (its data with the link `content`, or all of
    /// it when there is none) and builds the image, sealed and valid. With `only_worlds`,
    /// only the content and those worlds are taken (no world for an empty list), never the
    /// player's other files.
    pub(crate) fn build_snapshot(
        &self,
        ctx: &InstanceContext,
        take: &Take,
        content: Option<String>,
        records: &[SnapshotRecord],
        name: &str,
        only_worlds: Option<&[String]>,
    ) -> Result<(Image, Snapshot), ImageError> {
        let store = self.store();
        let cache = store.load_stat_cache(&ctx.id);
        // Only some worlds: the content and those worlds, nothing else of the player's.
        let picked = |candidate: &Candidate| {
            only_worlds.is_none_or(|worlds| {
                ctx.scope.accepts(candidate) || super::scope::in_worlds(candidate, worlds)
            })
        };
        let data_filter =
            |candidate: &Candidate| ctx.scope.data_accepts(candidate) && picked(candidate);
        let all_filter = |candidate: &Candidate| full_filter(candidate) && picked(candidate);
        let filter: &(dyn Fn(&Candidate) -> bool + Sync) = if content.is_some() {
            &data_filter
        } else {
            &all_filter
        };
        let options = SnapshotOptions {
            cache: Some(&cache),
            filter: Some(filter),
            cancel: take.cancel,
            ..SnapshotOptions::default()
        };
        let snapshot_error = |e: snapshot::SnapshotError| match e {
            snapshot::SnapshotError::Cancelled => ImageError::Cancelled,
            other => other.into(),
        };
        let plan = snapshot::plan(store, &ctx.dir, &DiskLayout::Client, &options)
            .map_err(snapshot_error)?;
        let _room = self.reserve_space(plan.bytes_to_read())?;
        let snapshot =
            snapshot::run(store, plan, &options, take.progress).map_err(snapshot_error)?;
        let kind = if content.is_some() {
            Kind::Data
        } else {
            Kind::Full
        };
        let mut image = Image::new(kind, ctx.game());
        snapshot.fill(&mut image);
        image.entries = ctx.describe(std::mem::take(&mut image.entries));
        image.settings = ctx.settings();
        if kind == Kind::Full {
            image.base = ctx.base();
        }
        image.content = content;
        let saves = ctx.dir.join(CLIENT_GAME_FOLDER).join("saves");
        for world in &mut image.worlds {
            let size = image
                .entries
                .iter()
                .filter(|e| e.world.as_deref() == Some(world.name.as_str()))
                .map(|e| e.size)
                .fold(0, u64::saturating_add);
            world.size = Some(size);
            // The game isn't running: snapshots are refused while it is (a session of this
            // Launcher, or a world's `session.lock` held), and one a session begins during
            // is stopped and never recorded.
            world.quiesced = Some(true);
            if let Some(found) = crate::worlds::read_world(&saves.join(&world.name))
                && let Some(summary) = found.summary
            {
                world.level_name = summary.name;
                world.data_version = summary.data_version;
                world.game_version = summary.version_name;
                world.last_played = summary.last_played.filter(|ms| *ms > 0).map(|ms| ms / 1000);
            }
        }
        image.created = chrono::Utc::now().timestamp();
        image.producer = Producer {
            app: App::Launcher,
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        image.label = Some(name.to_string());
        image.seal();
        image.parent = records
            .first()
            .map(|newest| newest.image.clone())
            .filter(|parent| *parent != image.id);
        image
            .validate()
            .map_err(|e| ImageError::Invalid(format!("The snapshot isn't valid: {e}")))?;
        if !snapshot.too_long.is_empty() {
            super::warn(
                "images.snapshot",
                &ctx.id,
                &format!(
                    "{} files have paths too long to record",
                    snapshot.too_long.len()
                ),
            );
        }
        Ok((image, snapshot))
    }

    fn save_data_cache(&self, instance: &str, snapshot: &Snapshot) {
        if let Err(e) = self.store().save_stat_cache(instance, &snapshot.cache) {
            // Only slower next time.
            super::warn("images.snapshot", instance, &e.to_string());
        }
    }

    /// Pins or unpins a snapshot: a pinned one is never deleted by retention or Storage's
    /// clean-up. Blocking.
    pub fn set_pinned(&self, instance: &str, id: &str, pinned: bool) -> Result<(), ImageError> {
        let slot = self.slot(instance);
        let _record = slot.lock_record();
        let _writing = self.enter()?;
        let mut records = self.reconciled_locked(instance)?;
        let record = records
            .iter_mut()
            .find(|record| record.id == id)
            .ok_or_else(|| ImageError::NoSnapshot(id.to_string()))?;
        if record.pinned != pinned {
            record.pinned = pinned;
            self.save_records(instance, &records)?;
        }
        Ok(())
    }

    /// Deletes a snapshot: its record, then its image when no other record names it, then
    /// the content images nothing links to any more; asks for a collection. Blocking.
    pub fn delete_snapshot(&self, instance: &str, id: &str) -> Result<(), ImageError> {
        let slot = self.slot(instance);
        let deleted = {
            let _record = slot.lock_record();
            let _writing = self.enter()?;
            let mut records = self.reconciled_locked(instance)?;
            if !records.iter().any(|record| record.id == id) {
                return Err(ImageError::NoSnapshot(id.to_string()));
            }
            let deleted = self.delete_records_locked(instance, &mut records, &[id.to_string()])?;
            let current = state::load(self.store(), instance).current;
            if let Err(e) = self.prune_locked(instance, current.as_deref()) {
                super::warn("images.prune", instance, &e.to_string());
            }
            deleted
        };
        if deleted > 0 {
            self.request_gc();
        }
        Ok(())
    }

    /// Deletes a manifest of the instance that can't be read (listed in
    /// [`Snapshots::unreadable`]), so pruning can run again. Refuses one that reads.
    /// Blocking.
    pub fn delete_unreadable(&self, instance: &str, image: &str) -> Result<(), ImageError> {
        let slot = self.slot(instance);
        {
            let _record = slot.lock_record();
            let _writing = self.enter()?;
            if self.store().load_header(instance, image).is_ok() {
                return Err(ImageError::Invalid(format!(
                    "The image {image} can be read; delete its snapshot instead"
                )));
            }
            self.store().delete_manifest(instance, image)?;
            let current = state::load(self.store(), instance).current;
            if let Err(e) = self.prune_locked(instance, current.as_deref()) {
                super::warn("images.prune", instance, &e.to_string());
            }
        }
        self.request_gc();
        Ok(())
    }

    /// Deletes the records `ids` of `records` (which are the instance's, reconciled), and
    /// each one's image when no record left names it: the images first, then the records,
    /// so a crash in between leaves records whose image is gone, which the next reconcile
    /// drops. Returns how many manifests went. For a caller holding the record lock.
    pub(crate) fn delete_records_locked(
        &self,
        instance: &str,
        records: &mut Vec<SnapshotRecord>,
        ids: &[String],
    ) -> Result<usize, ImageError> {
        let (gone, kept): (Vec<SnapshotRecord>, Vec<SnapshotRecord>) = std::mem::take(records)
            .into_iter()
            .partition(|record| ids.contains(&record.id));
        *records = kept;
        let mut deleted = 0;
        let mut images: Vec<&String> = gone.iter().map(|record| &record.image).collect();
        images.sort();
        images.dedup();
        for image in images {
            if records.iter().any(|record| record.image == *image) {
                continue;
            }
            match self.store().delete_manifest(instance, image) {
                Ok(true) => deleted += 1,
                Ok(false) => {}
                // The record stays with its image.
                Err(e) => {
                    super::warn("images.snapshot", instance, &e.to_string());
                    records.extend(gone.iter().filter(|r| &r.image == image).cloned());
                }
            }
        }
        newest_first(records);
        self.save_records(instance, records)?;
        Ok(deleted)
    }

    /// Deletes what retention picks ([`data_to_delete`] with the instance's policy), then
    /// prunes the content images nothing links to any more, asking for a collection (at
    /// most every 15 minutes) when anything went. For a caller holding the record lock.
    pub(crate) fn apply_retention_locked(
        &self,
        ctx: &InstanceContext,
        records: &mut Vec<SnapshotRecord>,
        just_created: Option<&str>,
    ) -> Result<(), ImageError> {
        let policy = SnapshotPolicy::of(ctx.instance.snapshots.as_ref());
        let now = chrono::Utc::now().timestamp();
        // A snapshot being restored is read meanwhile ([`Images::keep_while`]).
        let held: Vec<String> = self
            .keep
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&ctx.id)
            .cloned()
            .unwrap_or_default();
        let ids: Vec<String> = data_to_delete(records, now, &policy, just_created)
            .into_iter()
            .filter(|id| {
                records
                    .iter()
                    .find(|r| &r.id == id)
                    .is_none_or(|r| !held.contains(&r.image))
            })
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        let deleted = self.delete_records_locked(&ctx.id, records, &ids)?;
        let current = state::load(self.store(), &ctx.id).current;
        let pruned = self.prune_locked(&ctx.id, current.as_deref())?;
        if deleted + pruned > 0 {
            self.request_gc_after_pruning();
        }
        Ok(())
    }

    /// The instance's records, brought in line with its manifests and saved when that
    /// changed them (see the module). For a caller holding the record lock inside a
    /// counted write.
    pub(crate) fn reconciled_locked(
        &self,
        instance: &str,
    ) -> Result<Vec<SnapshotRecord>, ImageError> {
        let (mut records, mut changed) = match self.load_records(instance)? {
            Some(records) => (records, false),
            None => {
                // Damaged: renamed aside, and every image comes back below.
                self.set_damaged_aside(instance)?;
                (Vec::new(), true)
            }
        };
        let ids = self.store().manifest_ids(instance)?;
        let before = records.len();
        records.retain(|record| ids.contains(&record.image));
        changed |= records.len() != before;
        for id in &ids {
            if records.iter().any(|record| &record.image == id) {
                continue;
            }
            let Ok(header) = self.store().load_header(instance, id) else {
                continue;
            };
            if header.kind == Kind::Content {
                continue;
            }
            let mut record = SnapshotRecord {
                id: new_record_id(header.created, &records),
                image: id.clone(),
                kind: SnapshotKind::Manual,
                name: snapshot_name(header.label.as_deref()),
                created: header.created,
                pinned: false,
                full: header.kind == Kind::Full,
                worlds: Vec::new(),
                world_details: Vec::new(),
                content: header.content.clone(),
                files: 0,
                withheld: 0,
                bytes: 0,
                new_bytes: 0,
            };
            if let Ok(image) = self.store().load_manifest(instance, id) {
                record.worlds = image.worlds.iter().map(|w| w.name.clone()).collect();
                record.world_details = record_worlds(&image);
                record.files = image.entries.len();
                record.withheld = image.withheld.len();
                record.bytes = lodestone_image::container::files_size(&image);
            }
            super::warn(
                "images.snapshot",
                instance,
                &format!("the snapshot image {id} had no record; it is listed again"),
            );
            records.push(record);
            changed = true;
        }
        newest_first(&mut records);
        records.truncate(MAX_RECORDS);
        if changed {
            self.save_records(instance, &records)?;
        }
        Ok(records)
    }

    /// The records file of `instance`: `Some(records)` (empty when there is none), or
    /// `None` when it is there but isn't one (too big, not JSON, a bad image id, too many
    /// records). Failing to read it is an error, never "no snapshots".
    pub(crate) fn load_records(
        &self,
        instance: &str,
    ) -> Result<Option<Vec<SnapshotRecord>>, ImageError> {
        let path = self.records_path(instance)?;
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Some(Vec::new())),
            Err(e) => return Err(ImageError::io("Couldn't look at the snapshot list")(e)),
        };
        if !meta.is_file() || meta.len() > MAX_RECORDS_BYTES {
            return Ok(None);
        }
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Some(Vec::new())),
            Err(e) => return Err(ImageError::io("Couldn't read the snapshot list")(e)),
        };
        let Ok(file) = serde_json::from_slice::<RecordsFile>(&bytes) else {
            return Ok(None);
        };
        let valid = file.snapshots.len() <= MAX_RECORDS
            && file.snapshots.iter().all(|record| {
                lodestone_image::hex::is_sha256(&record.image)
                    && record
                        .content
                        .as_deref()
                        .is_none_or(lodestone_image::hex::is_sha256)
                    && valid_record_id(&record.id)
            });
        if !valid {
            return Ok(None);
        }
        let mut records = file.snapshots;
        newest_first(&mut records);
        Ok(Some(records))
    }

    /// Renames a damaged records file aside, so the rebuilt one never replaces what may
    /// still be read by hand.
    fn set_damaged_aside(&self, instance: &str) -> Result<(), ImageError> {
        let path = self.records_path(instance)?;
        let aside = path.with_file_name(format!(
            "snapshots.damaged-{}.json",
            chrono::Utc::now().timestamp()
        ));
        super::warn(
            "images.snapshot",
            instance,
            "the snapshot list is damaged; it is set aside and every snapshot is listed again",
        );
        match std::fs::rename(&path, &aside) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(ImageError::io(
                "Couldn't set the damaged snapshot list aside",
            )(e)),
        }
    }

    pub(crate) fn save_records(
        &self,
        instance: &str,
        records: &[SnapshotRecord],
    ) -> Result<(), ImageError> {
        let instance_name = self
            .instances()
            .get(instance)
            .ok()
            .map(|found| found.name)
            .or_else(|| {
                let bytes = std::fs::read(self.records_path(instance).ok()?).ok()?;
                serde_json::from_slice::<RecordsFile>(&bytes)
                    .ok()?
                    .instance_name
            });
        let file = RecordsFile {
            version: RECORDS_VERSION,
            instance_name,
            snapshots: records.to_vec(),
        };
        let bytes = serde_json::to_vec_pretty(&file)
            .map_err(|e| ImageError::Invalid(format!("Failed to write the snapshot list: {e}")))?;
        super::write_small_atomic(&self.records_path(instance)?, &bytes)?;
        self.snapshots_changed(instance);
        Ok(())
    }

    fn records_path(&self, instance: &str) -> Result<PathBuf, ImageError> {
        Ok(self.store().instance_dir(instance)?.join(RECORDS_FILE))
    }

    /// [`Self::take_snapshot`] after a play session, when `after_play` is on and no other
    /// session of the instance began meanwhile. Logged when it fails.
    pub(crate) fn after_play(self: &Arc<Self>, instance: &str) {
        if self.slot(instance).in_session() {
            return;
        }
        let take = Take::new(SnapshotKind::AfterPlay, labels::AFTER_PLAYING);
        match self.take_snapshot(instance, take) {
            Ok(_) | Err(ImageError::Gone(_)) | Err(ImageError::Running) => {}
            Err(e) => super::warn("images.snapshot", instance, &e.to_string()),
        }
    }
}

/// Sorts records newest first by time, keeping their order within a second: the file
/// lists them newest first as they were taken, and an id's random part says nothing about
/// which came first.
pub(crate) fn newest_first(records: &mut [SnapshotRecord]) {
    records.sort_by_key(|r| std::cmp::Reverse(r.created));
}

/// A snapshot's name: `name` trimmed and cut to [`MAX_NAME_CHARS`], or
/// [`labels::SNAPSHOT`] when blank.
pub fn snapshot_name(name: Option<&str>) -> String {
    let name = name.map(str::trim).unwrap_or_default();
    if name.is_empty() {
        return labels::SNAPSHOT.to_string();
    }
    name.chars().take(MAX_NAME_CHARS).collect()
}

/// A new record id for a snapshot taken at `created` (unix seconds): its UTC time and four
/// random hex digits, unused among `records`.
pub(crate) fn new_record_id(created: i64, records: &[SnapshotRecord]) -> String {
    let time = chrono::DateTime::from_timestamp(created, 0)
        .unwrap_or_default()
        .format("%Y%m%d-%H%M%S");
    loop {
        let random = u16::from_le_bytes(crate::random_bytes());
        let id = format!("{time}-{random:04x}");
        if !records.iter().any(|record| record.id == id) {
            return id;
        }
    }
}

/// Whether `id` has a record id's shape (ASCII digits, `-`, lower-case hex; at most 32
/// characters): it is passed on the command line and shown, never used as a path.
pub(crate) fn valid_record_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || b == b'-' || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_ids() {
        assert_eq!(snapshot_name(None), labels::SNAPSHOT);
        assert_eq!(snapshot_name(Some("  ")), labels::SNAPSHOT);
        assert_eq!(snapshot_name(Some(" Before 1.21 ")), "Before 1.21");
        assert_eq!(
            snapshot_name(Some(&"x".repeat(500))).chars().count(),
            MAX_NAME_CHARS
        );
        let id = new_record_id(1_794_645_624, &[]);
        assert!(id.starts_with("20261114-"), "{id}");
        assert!(valid_record_id(&id));
        assert!(!valid_record_id("../x"));
        assert!(!valid_record_id(""));
        assert!(SnapshotKind::AfterPlay.automatic());
        assert!(!SnapshotKind::BeforeRestore.automatic());
        assert!(SnapshotKind::BeforeRestore.removable());
        assert!(!SnapshotKind::Manual.removable());
        assert!(!SnapshotKind::Imported.removable());
    }
}
