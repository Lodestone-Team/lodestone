//! Taking a backup: a snapshot of the server folder into the image store, sealed as a data
//! image (`docs/instance-images.md`, "When images are recorded").
//!
//! The snapshot walks the server folder with its disk layout (`crate::images::layout`), so
//! worlds are `saves/<world>/…`, and a Paper or Spigot server's `<level>_nether/DIM-1` and
//! `<level>_the_end/DIM1` are its level's `DIM-1` and `DIM1` (their other files go to the
//! level's `.bukkit_nether/` and `.bukkit_the_end/`). It leaves out what images never record
//! (logs, caches, `session.lock`, `.lodestone_config`…) and the policy's exclude patterns as
//! canonical patterns (`crate::images::excludes`, which never leave out more than the
//! policy; a pattern with no exact canonical form is backed up anyway). What it can't carry
//! is listed in the image as withheld, with why: links, files that can't be read or kept
//! changing while read, and names an image can't hold (a name that isn't a valid canonical
//! path, or a file the layout can't place back as itself, like a vanilla `DIM-1` left in a
//! Paper level). A restore leaves those as they are on disk (`super::restore`).
//!
//! The data stat cache (`stat-cache.json`, by disk path) means only files that changed are
//! read; a full scan reads every file. Steps, all blocking:
//!
//! 1. [`Prepared::new`]: the layout, the excludes and the stat cache.
//! 2. [`Prepared::prefetch`], for a live backup: when there is a lot to read, reads it with
//!    saving still on, so saving stays off only while what changed since is read.
//! 3. [`Prepared::take`]: the snapshot.
//! 4. [`build`]: the image, sealed, once the caller knows the content image it links to and
//!    whether the server stayed quiesced.

use std::path::PathBuf;
use std::time::Instant;

use lodestone_image::manifest::{App, MAX_TEXT_LEN};
use lodestone_image::store::snapshot::{self, Snapshot, SnapshotError, SnapshotOptions};
use lodestone_image::store::{StatCache, Store as ImageStore, StoreError};
use lodestone_image::{DiskLayout, Image, Kind, Producer};
use tracing::info;

use crate::error::{Error, ErrorKind};
use crate::images::excludes::canonical_excludes;
use crate::images::layout::{game_of, server_layout, server_settings};
use crate::implementations::minecraft::Flavour;
use crate::types::InstanceUuid;

use super::store::{cancelled, err, Cancel};
use crate::util::disk::require_free_space;

/// A live backup with more than this to read reads it first with saving on
/// ([`Prepared::prefetch`]), so saving stays off only while reading what changed since.
pub const PREFETCH_MIN_BYTES: u64 = 256 << 20;
/// Every this many scheduled backups, every file is read again (a full scan), so a file
/// whose size and time stayed while its bytes changed is caught.
pub const FULL_SCAN_EVERY: usize = 10;

/// Whether a backup reads every file: the first of a server, and every
/// [`FULL_SCAN_EVERY`]th scheduled one since the last full scan.
pub fn needs_full_scan(scheduled: bool, first: bool, scheduled_since_full: usize) -> bool {
    first || (scheduled && scheduled_since_full + 1 >= FULL_SCAN_EVERY)
}

/// What to back up.
#[derive(Debug, Clone)]
pub struct Request {
    pub uuid: InstanceUuid,
    /// The server folder.
    pub root: PathBuf,
    pub game_version: String,
    pub flavour: Flavour,
    /// `level-name`, or `world` when the server has none.
    pub level_name: String,
    /// The policy's exclude patterns (server disk paths).
    pub policy_exclude: Vec<String>,
    /// Read every file rather than trust the stat cache.
    pub full_scan: bool,
    /// Threads hashing and storing; 0 is the store's default. A live backup asks for one,
    /// so the game keeps its processors.
    pub threads: usize,
}

/// A backup about to be taken (see the module).
pub struct Prepared {
    pub request: Request,
    layout: DiskLayout,
    /// The level, when it is a world images can name.
    level: Option<String>,
    /// The image's excludes.
    excludes: Vec<String>,
    /// What the last backup (or [`Prepared::prefetch`]) left; `None` for a full scan.
    cache: Option<StatCache>,
    started: Instant,
    /// More than this to read is read ahead ([`PREFETCH_MIN_BYTES`]; tests lower it).
    pub prefetch_min: u64,
}

/// What [`Prepared::take`] found and stored.
pub struct Taken {
    pub snapshot: Snapshot,
    pub duration_ms: u64,
}

/// What [`build`] needs beyond the snapshot.
#[derive(Debug, Clone)]
pub struct Meta {
    /// The image's label: the backup's name, or what kind of backup it is.
    pub label: String,
    pub created: i64,
    /// For a live backup, whether the server stayed quiesced; true when stopped.
    pub quiesced: bool,
    /// The content image the server ran with.
    pub content: Option<String>,
    /// The previous backup's image, as the new image's (informational) parent.
    pub parent: Option<String>,
}

fn snapshot_error(e: SnapshotError) -> Error {
    match e {
        SnapshotError::Cancelled | SnapshotError::Store(StoreError::Cancelled) => cancelled(),
        SnapshotError::RootMissing(_) => err(ErrorKind::NotFound, "The server folder is missing"),
        SnapshotError::Store(e) => err(
            ErrorKind::Internal,
            format!("Failed to write to the snapshot store: {e}"),
        ),
    }
}

impl Prepared {
    /// Reads what the backup needs before reading any file.
    pub fn new(store: &ImageStore, request: Request) -> Result<Prepared, Error> {
        let started = Instant::now();
        let (layout, level_is_world) =
            server_layout(&request.root, &request.level_name, &request.flavour)?;
        let (level, excludes) = match &layout {
            DiskLayout::Server(server) => {
                let translated = canonical_excludes(&request.policy_exclude, server);
                if !translated.untranslated.is_empty() {
                    info!(
                        "Backups of {} keep what these exclude patterns match, since they have no exact form in an image: {}",
                        request.uuid,
                        translated.untranslated.join(", ")
                    );
                }
                (
                    level_is_world.then(|| server.level_name().to_string()),
                    translated.canonical,
                )
            }
            DiskLayout::Client => (None, Vec::new()),
        };
        let cache = (!request.full_scan).then(|| store.load_stat_cache(request.uuid.as_ref()));
        Ok(Prepared {
            request,
            layout,
            level,
            excludes,
            cache,
            started,
            prefetch_min: PREFETCH_MIN_BYTES,
        })
    }

    fn options<'a>(&'a self, cancel: Cancel<'a>) -> SnapshotOptions<'a> {
        SnapshotOptions {
            excludes: &self.excludes,
            cache: self.cache.as_ref(),
            threads: self.request.threads,
            cancel,
            ..SnapshotOptions::default()
        }
    }

    /// For a live backup, before saving is turned off: when there is more than
    /// [`Self::prefetch_min`] to read, reads and stores it now, and takes what it read as
    /// the stat cache of [`Self::take`]. Returns whether it read ahead. `progress` gets
    /// (bytes read, bytes to read) and is called first with nothing read.
    pub fn prefetch(
        &mut self,
        store: &ImageStore,
        progress: &(dyn Fn(u64, u64) + Sync),
        cancel: Cancel,
    ) -> Result<bool, Error> {
        let options = self.options(cancel);
        let plan = snapshot::plan(store, &self.request.root, &self.layout, &options)
            .map_err(snapshot_error)?;
        let to_read = plan.bytes_to_read();
        if to_read <= self.prefetch_min {
            return Ok(false);
        }
        require_free_space(store.root(), to_read, "for a snapshot")?;
        progress(0, to_read);
        let snapshot = snapshot::run(store, plan, &options, progress).map_err(snapshot_error)?;
        self.cache = Some(snapshot.cache);
        Ok(true)
    }

    /// Walks the folder (taking unchanged files from the stat cache), checks there is room
    /// for what is left to read, then reads and stores it. `walked` gets the bytes to read
    /// once the walk is done; `progress` then gets (bytes read, bytes to read).
    pub fn take(
        &self,
        store: &ImageStore,
        walked: &dyn Fn(u64),
        progress: &(dyn Fn(u64, u64) + Sync),
        cancel: Cancel,
    ) -> Result<Taken, Error> {
        let options = self.options(cancel);
        let plan = snapshot::plan(store, &self.request.root, &self.layout, &options)
            .map_err(snapshot_error)?;
        walked(plan.bytes_to_read());
        require_free_space(store.root(), plan.bytes_to_read(), "for a snapshot")?;
        let snapshot = snapshot::run(store, plan, &options, progress).map_err(snapshot_error)?;
        Ok(Taken {
            snapshot,
            duration_ms: self.started.elapsed().as_millis() as u64,
        })
    }

    /// The level, when the image has it: what [`super::types::BackupRecord::level`] says.
    pub fn level_in(&self, image: &Image) -> Option<String> {
        self.level
            .clone()
            .filter(|level| image.worlds.iter().any(|w| &w.name == level))
    }
}

/// The data image of a backup, sealed and valid.
pub fn build(prepared: &Prepared, taken: &Taken, meta: Meta) -> Result<Image, Error> {
    let request = &prepared.request;
    let mut image = Image::new(Kind::Data, game_of(&request.game_version, &request.flavour));
    taken.snapshot.fill(&mut image);
    for world in &mut image.worlds {
        let prefix = format!("saves/{}/", world.name);
        world.size = Some(
            image
                .entries
                .iter()
                .filter(|e| e.path.as_str().starts_with(&prefix))
                .map(|e| e.size)
                .fold(0, u64::saturating_add),
        );
        world.quiesced = Some(meta.quiesced);
    }
    image.settings = server_settings(&request.flavour);
    image.excludes = prepared.excludes.clone();
    image.content = meta.content;
    image.created = meta.created;
    image.producer = Producer {
        app: App::Core,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    image.label = Some(meta.label.chars().take(MAX_TEXT_LEN).collect());
    image.normalize();
    image.seal();
    image.parent = meta.parent.filter(|parent| *parent != image.id);
    image.validate().map_err(|e| {
        err(
            ErrorKind::Internal,
            format!("The snapshot isn't a valid image: {e}"),
        )
    })?;
    Ok(image)
}

/// A backup's "new bytes": the bytes of its entries that `previous` (the previous backup's
/// image) doesn't have as they are, by path and hash; all of them for a first backup.
pub fn new_bytes(image: &Image, previous: Option<&Image>) -> u64 {
    let Some(previous) = previous else {
        return image
            .entries
            .iter()
            .map(|e| e.size)
            .fold(0, u64::saturating_add);
    };
    image
        .entries
        .iter()
        .filter(|e| {
            previous
                .entry(&e.path)
                .is_none_or(|before| before.sha256 != e.sha256)
        })
        .map(|e| e.size)
        .fold(0, u64::saturating_add)
}

#[cfg(test)]
pub(crate) mod tests;
