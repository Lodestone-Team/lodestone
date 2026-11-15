//! Going back to a snapshot (`docs/instance-images.md`, "Restore").
//!
//! | Choice | Puts back |
//! | --- | --- |
//! | [`RestoreScope::Worlds`] | the chosen worlds' folders |
//! | [`RestoreScope::WorldsAndContent`] | the worlds and the content they ran with (the data image's content link, or a full image's own mods and configs), in one swap |
//! | [`RestoreScope::Everything`] | the whole game folder as the snapshot has it |
//! | [`Images::restore_as_new`] | a new instance beside the original |
//!
//! An in-place restore runs under the instance's record lock, never while its game runs,
//! and first takes a **safety snapshot** ("Before going back to “…”", kept 30 days); when
//! that fails, the restore is refused. A restore that puts content back first records the
//! content as it is ("Before going back to a snapshot"), so the safety snapshot's content
//! link holds every file it replaces, and is refused while a "Find it for me" search is
//! saved, and for a snapshot of another game (the Minecraft version and loader are fixed
//! per instance: only "as a new instance" can have another).
//!
//! It makes the files a snapshot would take in the chosen places match the snapshot, and
//! leaves everything else as it is: what no snapshot takes (`session.lock`, logs,
//! Lodestone's own `.lodestone…` files), links, and files the target snapshot withheld or
//! left out (it has no copy of them). So the safety snapshot has every file the restore
//! replaces or removes; a file it would replace that the safety snapshot couldn't read
//! refuses the restore. The files are staged inside the instance folder, checked byte by
//! byte, then swapped in with the journaled swap ([`super::swap`]).
//!
//! After a restore of content, the instance follows it: its loader version is the
//! snapshot's, `content.json` learns the restored files' Modrinth and CurseForge sources,
//! and a `modpack.json` whose pack version differs from the snapshot's follows the
//! snapshot's base: the version from the base, and as its files the paths it lists now
//! that the snapshot also has, with their restored SHA-1 (the updater never removes a file
//! it doesn't list, so a pack file only the older version had is treated as the user's).
//! Then the content is recorded ("Went back to snapshot “…”").
//!
//! The store keeps every object a manifest names, so the files are there; a store that lost
//! one refuses the restore, naming it, rather than restore part of it.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lodestone_image::layout::CLIENT_GAME_FOLDER;
use lodestone_image::paths::always_excluded;
use lodestone_image::store::Cancel;
use lodestone_image::store::excludes::Excludes;
use lodestone_image::store::materialize::{
    MaterializeError, MaterializeOptions, Selection, materialize,
};
use lodestone_image::store::snapshot::Candidate;
use lodestone_image::{
    Base, CanonicalPath, DiskLayout, Entry, Game, Image, Kind, Source, WithheldReason,
};
use serde::Serialize;

use super::context::InstanceContext;
use super::labels;
use super::recorder::Recorded;
use super::scope::{CONTENT_FOLDERS, ContentScope, is_lodestone_name};
use super::snapshots::{SnapshotKind, SnapshotRecord, Take};
use super::swap::{self, STAGING_PREFIX, Staged, SwapError};
use super::{ImageError, Images, state};
use crate::content::provenance::{self, CurseForgeSource, ModrinthSource, Provenance};
use crate::content::{ContentKind, InstanceDirs, enabled_name};
use crate::instances::{self, Instance};
use crate::modpack::ModpackRecord;

/// What an in-place restore puts back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreScope {
    /// These worlds (folders in `saves/`), nothing else.
    Worlds(Vec<String>),
    /// These worlds and the content they ran with.
    WorldsAndContent(Vec<String>),
    /// The whole game folder.
    Everything,
}

impl RestoreScope {
    fn has_content(&self) -> bool {
        !matches!(self, RestoreScope::Worlds(_))
    }
}

/// What a restore would do, for the dialog that asks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RestorePreview {
    /// The snapshot's worlds.
    pub worlds: Vec<WorldChange>,
    /// How the instance's content now differs from the content the snapshot ran with;
    /// `None` when it doesn't, or the snapshot has no content to compare.
    pub content: Option<ContentDiff>,
    /// Whether the snapshot can put content back (a full image, or a data image whose
    /// content link is there).
    pub can_restore_content: bool,
    /// The snapshot is of another Minecraft version or loader: only "as a new instance"
    /// can have it.
    pub other_game: bool,
    /// Files the store lost that a restore of everything would need.
    pub missing: Vec<String>,
}

/// One of the snapshot's worlds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorldChange {
    /// The folder in `saves/`.
    pub name: String,
    /// The world's own name, when the snapshot read it (show it through `display_text`).
    pub level_name: Option<String>,
    /// Whether the instance has a world in that folder now.
    pub exists_now: bool,
    pub size: u64,
}

/// How two sets of content differ: files by their name when enabled
/// (`mods/sodium.jar`), an update when the same Modrinth project or path has other bytes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ContentDiff {
    /// In the snapshot, not now.
    pub added: Vec<String>,
    /// Now, not in the snapshot.
    pub removed: Vec<String>,
    /// In both, with other bytes or turned on or off.
    pub updated: Vec<String>,
    /// The loader version now and in the snapshot, when they differ.
    pub loader_version: Option<(Option<String>, Option<String>)>,
}

impl ContentDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.updated.is_empty()
            && self.loader_version.is_none()
    }
}

/// What an in-place restore did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    /// The safety snapshot taken first.
    pub safety: Option<SnapshotRecord>,
    /// The instance as saved, when the restore changed its loader version.
    pub instance: Option<Instance>,
    /// The content record after it, when one was written. A record that fails after the
    /// files were put back is logged, not the restore's failure: the next check records the
    /// change.
    pub recorded: Option<Recorded>,
    /// Where files another job wrote into what the restore replaced meanwhile were kept
    /// (`<instance>/restore-leftovers-<16>/`), when there were any.
    pub kept_aside: Option<PathBuf>,
}

/// A snapshot loaded whole: its image and, for a data image, the content it links to.
struct Target {
    image: Image,
    content: Option<Image>,
    /// It came from an import ([`SnapshotKind::Imported`]): files the store lacks were never
    /// downloaded, rather than lost.
    imported: bool,
}

impl Target {
    /// Its content files: the linked image's, or a full image's own (its files that are
    /// content by `scope`).
    fn content_entries(&self, scope: &ContentScope) -> Option<Vec<Entry>> {
        match (&self.image.kind, &self.content) {
            (_, Some(content)) => Some(content.entries.clone()),
            (Kind::Full, None) => Some(
                self.image
                    .entries
                    .iter()
                    .filter(|e| e.world.is_none() && is_content_entry(scope, &e.path))
                    .cloned()
                    .collect(),
            ),
            _ => None,
        }
    }

    /// Its files in `worlds`.
    fn world_entries(&self, worlds: &[String]) -> Vec<Entry> {
        self.image
            .entries
            .iter()
            .filter(|e| e.world.as_ref().is_some_and(|w| worlds.contains(w)))
            .cloned()
            .collect()
    }

    /// Every file: the image's and its content link's (the image wins at a path both
    /// have).
    fn all_entries(&self) -> Vec<Entry> {
        let mut by_path: BTreeMap<String, Entry> = BTreeMap::new();
        for entry in self.content.iter().flat_map(|c| c.entries.iter()) {
            by_path.insert(entry.path.to_string(), entry.clone());
        }
        for entry in &self.image.entries {
            by_path.insert(entry.path.to_string(), entry.clone());
        }
        by_path.into_values().collect()
    }

    /// Whether the snapshot carries no copy of `path`: it withheld it (or a folder it is
    /// in), or left it out by its excludes.
    fn left_out(&self, path: &CanonicalPath, excludes: &Excludes) -> bool {
        let text = path.as_str();
        let withheld = self
            .image
            .withheld
            .iter()
            .chain(self.content.iter().flat_map(|c| c.withheld.iter()))
            .any(|w| {
                text == w.path
                    || text
                        .strip_prefix(w.path.as_str())
                        .is_some_and(|rest| rest.starts_with('/'))
            });
        withheld || excludes.excludes(text, false)
    }

    fn excludes(&self) -> Excludes {
        let patterns: Vec<String> = self
            .image
            .excludes
            .iter()
            .chain(self.content.iter().flat_map(|c| c.excludes.iter()))
            .cloned()
            .collect();
        Excludes::new(&patterns)
    }
}

/// Whether the file at `path` is content by `scope`.
fn is_content_entry(scope: &ContentScope, path: &CanonicalPath) -> bool {
    let disk = format!("{CLIENT_GAME_FOLDER}/{path}");
    scope.accepts(&Candidate {
        disk: &disk,
        path: Some(path),
        is_dir: false,
    })
}

/// `path` on the instance folder: `minecraft/<path>`.
fn disk_of(path: &CanonicalPath) -> String {
    format!("{CLIENT_GAME_FOLDER}/{path}")
}

/// Whether the relative path `rel` is `owned` or inside it.
fn inside(rel: &str, owned: &str) -> bool {
    rel == owned
        || rel
            .strip_prefix(owned)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The instance's loader as an image names it, back.
pub(crate) fn loader_of(loader: &lodestone_image::Loader) -> Option<instances::Loader> {
    Some(match loader {
        lodestone_image::Loader::Vanilla => instances::Loader::None,
        lodestone_image::Loader::Fabric => instances::Loader::Fabric,
        lodestone_image::Loader::Quilt => instances::Loader::Quilt,
        lodestone_image::Loader::Forge => instances::Loader::Forge,
        lodestone_image::Loader::NeoForge => instances::Loader::NeoForge,
        _ => return None,
    })
}

/// Whether `a` and `b` are another Minecraft version or loader.
fn other_game(a: &Game, b: &Game) -> bool {
    a.minecraft != b.minecraft || a.loader != b.loader
}

impl Images {
    /// Loads the snapshot `record` of `instance` whole.
    fn target(&self, instance: &str, record: &SnapshotRecord) -> Result<Target, ImageError> {
        let image = self.store().load_manifest(instance, &record.image)?;
        let content = match image.content.as_deref() {
            Some(link) if image.kind == Kind::Data => {
                self.store().load_manifest(instance, link).ok()
            }
            _ => None,
        };
        Ok(Target {
            image,
            content,
            imported: record.kind == SnapshotKind::Imported,
        })
    }

    /// What going back to the snapshot `id` would do (see [`RestorePreview`]). Blocking;
    /// changes nothing.
    pub fn preview_restore(&self, instance: &str, id: &str) -> Result<RestorePreview, ImageError> {
        let record = self.snapshot(instance, id)?;
        let ctx = InstanceContext::load(self.instances(), instance)?;
        let target = self.target(instance, &record)?;
        let saves = ctx.dir.join(CLIENT_GAME_FOLDER).join("saves");
        let worlds = target
            .image
            .worlds
            .iter()
            .map(|world| WorldChange {
                name: world.name.clone(),
                level_name: world.level_name.clone(),
                exists_now: saves.join(&world.name).is_dir(),
                size: world.size.unwrap_or_default(),
            })
            .collect();
        let target_content = target.content_entries(&ctx.scope);
        let current = state::load(self.store(), instance)
            .current
            .and_then(|id| self.store().load_manifest(instance, &id).ok());
        let content = match (&target_content, &current) {
            (Some(target_content), Some(current)) => {
                let mut diff = diff_content(&current.entries, target_content);
                let target_game = &target.image.game;
                if current.game.loader_version != target_game.loader_version {
                    diff.loader_version = Some((
                        current.game.loader_version.clone(),
                        target_game.loader_version.clone(),
                    ));
                }
                (!diff.is_empty()).then_some(diff)
            }
            _ => None,
        };
        let all = target.all_entries();
        let missing = all
            .iter()
            .filter(|e| !self.store().object_complete(&e.sha256, e.size))
            .map(|e| e.path.to_string())
            .collect();
        Ok(RestorePreview {
            worlds,
            content,
            can_restore_content: target_content.is_some(),
            other_game: other_game(&ctx.game(), &target.image.game),
            missing,
        })
    }

    /// Goes back to the snapshot `id` in place (see the module). Blocking.
    pub fn restore(
        self: &Arc<Self>,
        instance: &str,
        id: &str,
        scope: &RestoreScope,
        cancel: Cancel,
    ) -> Result<Restored, ImageError> {
        if !self.accepts(instance) {
            return Err(ImageError::Invalid(
                "The image store can't name this instance".into(),
            ));
        }
        let slot = self.slot(instance);
        let _record = slot.lock_record();
        let _writing = self.enter()?;
        if slot.in_session() {
            return Err(ImageError::Running);
        }
        self.recover_unfinished_locked(instance)?;
        let record = self
            .reconciled_locked(instance)?
            .into_iter()
            .find(|record| record.id == id)
            .ok_or_else(|| ImageError::NoSnapshot(id.to_string()))?;
        let ctx = InstanceContext::load(self.instances(), instance)?;
        // A game this process didn't start (the Launcher was restarted while it ran) has
        // the worlds open: swapping them would mix its next save into the restored world.
        let owned_worlds = match scope {
            RestoreScope::Worlds(worlds) | RestoreScope::WorldsAndContent(worlds) => {
                Some(worlds.as_slice())
            }
            RestoreScope::Everything => None,
        };
        if crate::worlds::open_in_game(
            &ctx.dir.join(CLIENT_GAME_FOLDER).join("saves"),
            owned_worlds,
        )
        .is_some()
        {
            return Err(ImageError::Running);
        }
        let target = self.target(instance, &record)?;
        // Kept from pruning and retention while it is read (the safety snapshot's
        // retention runs in between).
        let _keep_image = self.keep_while(instance, &target.image.id);
        let _keep_content = target
            .content
            .as_ref()
            .map(|content| self.keep_while(instance, &content.id));
        let worlds: &[String] = match scope {
            RestoreScope::Worlds(worlds) | RestoreScope::WorldsAndContent(worlds) => worlds,
            RestoreScope::Everything => &[],
        };
        if let Some(missing) = worlds
            .iter()
            .find(|w| !target.image.worlds.iter().any(|t| t.name == **w))
        {
            return Err(ImageError::Invalid(format!(
                "The snapshot has no world {missing:?}"
            )));
        }
        if matches!(scope, RestoreScope::Worlds(w) if w.is_empty()) {
            return Err(ImageError::Invalid("Choose a world to put back".into()));
        }
        let target_content = target.content_entries(&ctx.scope);
        if scope.has_content() {
            if self.finder_running(instance) {
                return Err(ImageError::Invalid(
                    "Finish the Find it for me search first".into(),
                ));
            }
            if other_game(&ctx.game(), &target.image.game) {
                return Err(ImageError::Invalid(
                    "The snapshot is of another Minecraft version or loader; restore it as a new instance".into(),
                ));
            }
            if target_content.is_none() {
                return Err(ImageError::Invalid(
                    "The content this snapshot ran with is gone, so only its worlds can be put back".into(),
                ));
            }
        }

        // The safety snapshot: everything the restore replaces or removes.
        let safety = self.take_snapshot_locked(
            &slot,
            instance,
            Take::new(
                SnapshotKind::BeforeRestore,
                labels::before_going_back(&record.name),
            ),
            scope.has_content().then_some(labels::BEFORE_RESTORE),
        )?;
        let safety_target = match &safety {
            Some(safety) => self.target(instance, safety)?,
            None => {
                return Err(ImageError::Invalid(
                    "The safety snapshot wasn't taken, so nothing was put back".into(),
                ));
            }
        };

        let (owned, entries) = plan_scope(&ctx, &target, &safety_target, scope, target_content)?;
        let excludes = target.excludes();
        let safety_all = safety_target.all_entries();
        let replaced: HashSet<String> = safety_all
            .iter()
            .filter(|e| !target.left_out(&e.path, &excludes))
            .map(|e| disk_of(&e.path))
            .filter(|disk| owned.iter().any(|o| inside(disk, o)))
            .collect();
        let placed: Vec<String> = entries.iter().map(|e| disk_of(&e.path)).collect();
        let placed_set: HashSet<&str> = placed.iter().map(String::as_str).collect();
        // A file the restore would replace that the safety snapshot couldn't read.
        let unsafe_file = safety_target
            .image
            .withheld
            .iter()
            .chain(safety_target.content.iter().flat_map(|c| c.withheld.iter()))
            .filter(|w| {
                matches!(
                    w.reason,
                    WithheldReason::Unreadable | WithheldReason::KeptChanging
                )
            })
            .find(|w| placed_set.contains(format!("{CLIENT_GAME_FOLDER}/{}", w.path).as_str()));
        if let Some(file) = unsafe_file {
            return Err(ImageError::Invalid(format!(
                "{} couldn't be read for the safety snapshot, so the restore won't replace it; close what holds it and try again",
                file.path
            )));
        }

        // Stage, checking every byte, while collections are held off.
        let _hold = self.store().hold_off_collection()?;
        let bytes = entries.iter().map(|e| e.size).fold(0, u64::saturating_add);
        let _room = self.reserve_space(bytes)?;
        let suffix = swap::random_suffix();
        let staging = ctx.dir.join(format!("{STAGING_PREFIX}{suffix}"));
        std::fs::create_dir(&staging).map_err(ImageError::io(
            "Failed to create the restore's staging folder",
        ))?;
        let staged = (|| -> Result<Staged, ImageError> {
            stage(self, &target, &entries, &staging, cancel)?;
            Staged::new(&ctx.dir, suffix, &owned, &replaced, &placed)
        })();
        let staged = match staged {
            Ok(staged) => staged,
            Err(e) => {
                let _ = super::remove_tree(&staging);
                return Err(e);
            }
        };
        let journal = self
            .store()
            .instance_dir(instance)?
            .join(swap::JOURNAL_FILE);
        let kept_aside = match staged.swap(&journal) {
            Ok(kept) => kept,
            Err(SwapError::RolledBack(e)) | Err(SwapError::Broken(e)) => return Err(e),
        };
        if let Some(kept) = &kept_aside {
            super::warn(
                "images.restore",
                instance,
                &format!(
                    "files written meanwhile into what the restore replaced are kept in {}",
                    kept.display()
                ),
            );
        }

        let saved = if scope.has_content() {
            self.follow_restored_content(&ctx, &target, &entries)
        } else {
            None
        };
        // The files are back: a record that fails now (the disk filled up) is no failure
        // of the restore, whose caller would otherwise reload nothing and keep showing the
        // old loader version. The next check records the change as "Changed by hand".
        let recorded =
            match self.record_held(&slot, instance, Some(&labels::reverted(&record.name))) {
                Ok(recorded) => recorded,
                Err(e) => {
                    super::warn("images.record", instance, &e.to_string());
                    None
                }
            };
        Ok(Restored {
            safety,
            instance: saved,
            recorded,
            kept_aside,
        })
    }

    /// After a restore of content: the instance follows it (see the module). Each step's
    /// failure is logged; the files are restored either way. Returns the instance when its
    /// `instance.json` was saved with another loader version.
    fn follow_restored_content(
        &self,
        ctx: &InstanceContext,
        target: &Target,
        entries: &[Entry],
    ) -> Option<Instance> {
        let dirs = InstanceDirs::new(&ctx.dir);
        let mut saved = None;
        let game = &target.image.game;
        if ctx.instance.loader != instances::Loader::None
            && ctx.instance.loader_version != game.loader_version
            && game.loader_version.is_some()
        {
            let mut instance = ctx.instance.clone();
            instance.loader_version = game.loader_version.clone();
            match self.instances().save(&instance) {
                Ok(()) => saved = Some(instance),
                Err(e) => super::warn("images.restore", &ctx.id, &e.to_string()),
            }
        }
        if let Err(e) = learn_sources(&dirs, entries) {
            super::warn("images.restore", &ctx.id, &e);
        }
        let base = target
            .content
            .as_ref()
            .map_or(target.image.base.clone(), |content| content.base.clone());
        if let Err(e) = follow_base(&dirs, base.as_ref(), entries) {
            super::warn("images.restore", &ctx.id, &e);
        }
        if let Err(e) = crate::modpack::addon::forget_missing(&dirs) {
            super::warn("images.restore", &ctx.id, &e.to_string());
        }
        saved
    }

    /// Makes a new instance from the snapshot `id` of `instance`, named `name` (made
    /// unique): its files, the source's own settings (as Duplicate copies them) with the
    /// snapshot's game, and its content sources. Recorded as "Made from a snapshot".
    /// Blocking.
    pub fn restore_as_new(
        self: &Arc<Self>,
        instance: &str,
        id: &str,
        name: &str,
        cancel: Cancel,
    ) -> Result<Instance, ImageError> {
        let made = {
            let slot = self.slot(instance);
            let _record = slot.lock_record();
            let _writing = self.enter()?;
            let record = self
                .reconciled_locked(instance)?
                .into_iter()
                .find(|record| record.id == id)
                .ok_or_else(|| ImageError::NoSnapshot(id.to_string()))?;
            let source = InstanceContext::load(self.instances(), instance)?;
            let target = self.target(instance, &record)?;
            if target.image.kind == Kind::Data && target.content.is_none() {
                return Err(ImageError::Invalid(
                    "The content this snapshot ran with is gone, so it can't make a new instance"
                        .into(),
                ));
            }
            let entries = target.all_entries();
            let _hold = self.store().hold_off_collection()?;
            let bytes = entries.iter().map(|e| e.size).fold(0, u64::saturating_add);
            let _room = self.reserve_space(bytes)?;
            let staging = self
                .instances()
                .new_staging()
                .map_err(|e| ImageError::Instance(e.to_string()))?;
            let made = (|| -> Result<Instance, ImageError> {
                stage(self, &target, &entries, &staging, cancel)?;
                if let Some(record) = &source.record {
                    let _ = crate::storage::save(&ModpackRecord::path(&staging), record);
                }
                let dirs = InstanceDirs::new(&staging);
                learn_sources(&dirs, &entries).map_err(ImageError::Invalid)?;
                let base = target
                    .content
                    .as_ref()
                    .map_or(target.image.base.clone(), |content| content.base.clone());
                follow_base(&dirs, base.as_ref(), &entries).map_err(ImageError::Invalid)?;
                let game = &target.image.game;
                let loader = loader_of(&game.loader).unwrap_or(instances::Loader::None);
                let new = Instance {
                    name: name.trim().to_string(),
                    game_version: game.minecraft.clone(),
                    loader,
                    loader_version: game.loader_version.clone(),
                    recommended_memory: target
                        .image
                        .settings
                        .client
                        .as_ref()
                        .and_then(|c| c.recommended_memory_mb)
                        .or(source.instance.recommended_memory),
                    created: instances::now(),
                    last_played: None,
                    play_time: 0,
                    snapshots: None,
                    ..source.instance.clone()
                };
                self.instances()
                    .commit_staged(&staging, new)
                    .map_err(|e| ImageError::Instance(e.to_string()))
            })();
            if made.is_err() {
                let _ = super::remove_tree(&staging);
            }
            made?
        };
        if let Err(e) = self.record_now(&made.id, labels::FROM_SNAPSHOT) {
            super::warn("images.record", &made.id, &e.to_string());
        }
        Ok(made)
    }

    /// Puts the world `world` of the snapshot `id` back beside the instance's worlds, as a
    /// new world "`<world>` (restored `<date>`)", replacing nothing. Returns its folder.
    /// Blocking.
    pub fn restore_world_copy(
        &self,
        instance: &str,
        id: &str,
        world: &str,
        cancel: Cancel,
    ) -> Result<String, ImageError> {
        let slot = self.slot(instance);
        let _record = slot.lock_record();
        let _writing = self.enter()?;
        let record = self
            .reconciled_locked(instance)?
            .into_iter()
            .find(|record| record.id == id)
            .ok_or_else(|| ImageError::NoSnapshot(id.to_string()))?;
        let ctx = InstanceContext::load(self.instances(), instance)?;
        let target = self.target(instance, &record)?;
        let found = target
            .image
            .worlds
            .iter()
            .find(|w| w.name == world)
            .ok_or_else(|| ImageError::Invalid(format!("The snapshot has no world {world:?}")))?
            .clone();
        let entries = target.world_entries(&[world.to_string()]);
        let _hold = self.store().hold_off_collection()?;
        let bytes = entries.iter().map(|e| e.size).fold(0, u64::saturating_add);
        let _room = self.reserve_space(bytes)?;
        let suffix = swap::random_suffix();
        let staging = ctx.dir.join(format!("{STAGING_PREFIX}{suffix}"));
        std::fs::create_dir(&staging).map_err(ImageError::io(
            "Failed to create the restore's staging folder",
        ))?;
        let placed = (|| -> Result<String, ImageError> {
            stage(self, &target, &entries, &staging, cancel)?;
            let saves = ctx.dir.join(CLIENT_GAME_FOLDER).join("saves");
            std::fs::create_dir_all(&saves)
                .map_err(ImageError::io("Failed to create the saves folder"))?;
            let today = crate::worlds::date(std::time::SystemTime::now());
            let folder = crate::worlds::unused(
                &saves,
                &crate::worlds::folder_name(&format!("{world} (restored {today})")),
            );
            let staged = staging.join(CLIENT_GAME_FOLDER).join("saves").join(world);
            crate::fsx::rename(&staged, saves.join(&folder))
                .map_err(ImageError::io("Failed to place the restored world"))?;
            let name = found
                .level_name
                .clone()
                .unwrap_or_else(|| world.to_string());
            crate::worlds::set_level_name(
                &saves.join(&folder),
                &format!("{name} (restored {today})"),
            );
            Ok(folder)
        })();
        let _ = super::remove_tree(&staging);
        placed
    }

    /// Whether a restore of `instance` didn't finish and couldn't be undone: its journal is
    /// still there (a rollback that failed, at startup or during the restore itself). The
    /// instance mustn't be played until [`Self::retry_unfinished_restore`] undoes it: the
    /// game would start with worlds or mods renamed aside and write fresh ones. One look at
    /// a file, for Play; a restore under way has its journal too, while the instance is
    /// busy.
    pub fn restore_unfinished(&self, instance: &str) -> bool {
        self.accepts(instance)
            && self
                .store()
                .instance_dir(instance)
                .is_ok_and(|dir| dir.join(swap::JOURNAL_FILE).exists())
    }

    /// Tries again to undo a restore of `instance` that couldn't be undone (Play does,
    /// before refusing). `Ok` when none is left. Waits at most `wait` for another record
    /// of the instance, then says to try again. Blocking, but short: renames back.
    pub fn retry_unfinished_restore(
        &self,
        instance: &str,
        wait: std::time::Duration,
    ) -> Result<(), ImageError> {
        if !self.restore_unfinished(instance) {
            return Ok(());
        }
        let slot = self.slot(instance);
        let Some(_record) = super::check::lock_within(&slot.record, wait) else {
            return Err(ImageError::Invalid(
                "The instance is busy; try again in a moment".into(),
            ));
        };
        self.recover_unfinished_locked(instance)
    }

    /// Undoes an unfinished restore of `instance` (see [`Self::restore_unfinished`]), for a
    /// caller holding its record lock: Play's retry, and every restore before it starts,
    /// which would otherwise write its journal over the one that knows what is aside.
    fn recover_unfinished_locked(&self, instance: &str) -> Result<(), ImageError> {
        let journal = self
            .store()
            .instance_dir(instance)?
            .join(swap::JOURNAL_FILE);
        if !journal.exists() {
            return Ok(());
        }
        let root = self.instances().dir(instance);
        match swap::recover_one(&root, &journal) {
            Ok(_) => {
                swap::remove_leftovers(&root);
                super::warn("images.restore", instance, "undid an unfinished restore");
                Ok(())
            }
            Err(problem) => Err(ImageError::Invalid(format!(
                "A restore of this instance didn't finish and couldn't be undone ({problem}). Close any program using its files and try again, or restart the Launcher"
            ))),
        }
    }

    /// Finishes or undoes every restore a crash interrupted (see [`super::swap`]), and
    /// deletes restore work folders no journal names. Run once when the Launcher starts,
    /// right after opening the images and before anything can be played. A journal whose
    /// instance folder is gone stays, with a note. Blocking.
    pub fn recover_restores(&self) -> swap::Recovery {
        let mut recovery = swap::Recovery::default();
        let mut stuck: HashSet<String> = HashSet::new();
        let Ok(ids) = self.store().list_instances() else {
            return recovery;
        };
        for id in ids {
            if id.starts_with(super::retired::RETIRED_PREFIX) {
                continue;
            }
            let Ok(journal) = self
                .store()
                .instance_dir(&id)
                .map(|dir| dir.join(swap::JOURNAL_FILE))
            else {
                continue;
            };
            if !journal.exists() {
                continue;
            }
            let root = self.instances().dir(&id);
            if !root.is_dir() {
                super::warn(
                    "images.restore",
                    &id,
                    "an interrupted restore's journal stays: its instance folder isn't there",
                );
                continue;
            }
            match swap::recover_one(&root, &journal) {
                Ok(Some(phase)) => {
                    super::warn(
                        "images.restore",
                        &id,
                        if phase == swap::Phase::Done {
                            "finished an interrupted restore"
                        } else {
                            "undid an interrupted restore"
                        },
                    );
                    recovery.repaired.push(id);
                }
                Ok(None) => {}
                Err(problem) => {
                    super::warn(
                        "images.restore",
                        &id,
                        &format!("an interrupted restore couldn't be undone: {problem}"),
                    );
                    stuck.insert(id.clone());
                    recovery.broken.push(id);
                }
            }
        }
        let (instances, _) = self.instances().list();
        for instance in instances {
            if !stuck.contains(&instance.id) {
                swap::remove_leftovers(&self.instances().dir(&instance.id));
            }
        }
        recovery
    }
}

/// What an in-place restore of `scope` owns (paths relative to the instance folder it
/// makes match the snapshot) and the snapshot's files it places there.
fn plan_scope(
    ctx: &InstanceContext,
    target: &Target,
    safety: &Target,
    scope: &RestoreScope,
    target_content: Option<Vec<Entry>>,
) -> Result<(Vec<String>, Vec<Entry>), ImageError> {
    let game = |path: &str| format!("{CLIENT_GAME_FOLDER}/{path}");
    let mut owned: BTreeSet<String> = BTreeSet::new();
    let mut entries: Vec<Entry> = Vec::new();
    match scope {
        RestoreScope::Worlds(worlds) | RestoreScope::WorldsAndContent(worlds) => {
            for world in worlds {
                owned.insert(game(&format!("saves/{world}")));
            }
            entries.extend(target.world_entries(worlds));
            if matches!(scope, RestoreScope::WorldsAndContent(_)) {
                let content = target_content.unwrap_or_default();
                for folder in CONTENT_FOLDERS {
                    owned.insert(game(folder));
                }
                // The pack files outside the content folders, the snapshot's and those on
                // disk now (the safety snapshot's content holds them).
                let safety_content = safety.content_entries(&ctx.scope).unwrap_or_default();
                for entry in content.iter().chain(safety_content.iter()) {
                    let first = entry.path.components().next().unwrap_or_default();
                    if !CONTENT_FOLDERS
                        .iter()
                        .any(|folder| first.eq_ignore_ascii_case(folder))
                    {
                        owned.insert(disk_of(&entry.path));
                    }
                }
                entries.extend(content);
            }
        }
        RestoreScope::Everything => {
            entries = target.all_entries();
            let game_dir = ctx.dir.join(CLIENT_GAME_FOLDER);
            let mut tops: BTreeSet<String> = entries
                .iter()
                .filter_map(|e| e.path.components().next().map(str::to_string))
                .collect();
            if let Ok(found) = std::fs::read_dir(&game_dir) {
                tops.extend(
                    found
                        .flatten()
                        .filter_map(|e| e.file_name().to_str().map(str::to_string)),
                );
            }
            for top in tops {
                let keep_out = is_lodestone_name(&top)
                    || CanonicalPath::new(format!("{top}/x")).is_ok_and(|p| always_excluded(&p))
                    || CanonicalPath::new(&top).is_ok_and(|p| always_excluded(&p));
                if !keep_out {
                    owned.insert(game(&top));
                }
            }
        }
    }
    // Two entries of one path (a pack file both images list) are placed once.
    let mut seen = HashSet::new();
    entries.retain(|e| seen.insert(e.path.to_string()));
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let owned: Vec<String> = owned.into_iter().collect();
    if let Some(stray) = entries
        .iter()
        .find(|e| !owned.iter().any(|o| inside(&disk_of(&e.path), o)))
    {
        return Err(ImageError::Invalid(format!(
            "the restore would place {} outside what it owns",
            stray.path
        )));
    }
    Ok((owned, entries))
}

/// Places `entries` into `staging` (inside the folder they will replace), as the client
/// lays them out, checking every byte; durable, since the swap comes next.
fn stage(
    images: &Images,
    target: &Target,
    entries: &[Entry],
    staging: &Path,
    cancel: Cancel,
) -> Result<(), ImageError> {
    let mut image = Image::new(Kind::Full, target.image.game.clone());
    image.entries = entries.to_vec();
    let options = MaterializeOptions {
        selection: Selection::All,
        durable: true,
        cancel,
    };
    match materialize(
        images.store(),
        &image,
        &DiskLayout::Client,
        staging,
        &options,
    ) {
        Ok(_) => Ok(()),
        Err(MaterializeError::Cancelled) => Err(ImageError::Cancelled),
        Err(MaterializeError::Missing(paths)) if target.imported => {
            Err(ImageError::Invalid(format!(
                "{} of the snapshot's files, such as {}, couldn't be downloaded when it was \
                 imported, so nothing was put back",
                paths.len(),
                lodestone_image::display_text(
                    paths.first().map(CanonicalPath::as_str).unwrap_or_default()
                )
            )))
        }
        Err(MaterializeError::Missing(paths)) => Err(ImageError::Invalid(format!(
            "The image store lost {} of the snapshot's files, such as {}, so nothing was put back",
            paths.len(),
            paths.first().map(CanonicalPath::as_str).unwrap_or_default()
        ))),
        Err(e) => Err(ImageError::Invalid(format!(
            "The snapshot's files couldn't be put back: {e}"
        ))),
    }
}

/// `content.json` learns the Modrinth and CurseForge sources of the restored files directly
/// in `mods/`, `resourcepacks/` and `shaderpacks/` (now on disk under `dirs`): an entry with
/// the same SHA-512 keeps what it knew with the file's new size and time; otherwise one is
/// made from the image's source. Files without a source are looked up again by the next
/// listing, as `content.json` is only a cache.
pub(crate) fn learn_sources(dirs: &InstanceDirs, entries: &[Entry]) -> Result<(), String> {
    let mut learned: Vec<(ContentKind, String, provenance::Entry)> = Vec::new();
    for entry in entries {
        let mut parts = entry.path.components();
        let (Some(folder), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let Some(kind) = ContentKind::ALL.into_iter().find(|k| k.dir() == folder) else {
            continue;
        };
        let Some(sha512) = entry.sha512.clone() else {
            continue;
        };
        let (modrinth, curseforge) = match &entry.source {
            Source::Modrinth {
                project,
                version,
                url,
            } => (
                Some(ModrinthSource {
                    project_id: project.clone(),
                    version_id: version.clone(),
                    url: Some(url.clone()),
                    ..ModrinthSource::default()
                }),
                None,
            ),
            Source::Curseforge {
                project,
                file,
                downloadable,
            } => (
                None,
                Some(CurseForgeSource {
                    project_id: *project,
                    file_id: *file,
                    downloadable: *downloadable,
                    ..CurseForgeSource::default()
                }),
            ),
            _ => continue,
        };
        let Ok(meta) = std::fs::metadata(dirs.game.join(folder).join(name)) else {
            continue;
        };
        learned.push((
            kind,
            name.to_string(),
            provenance::Entry {
                size: meta.len(),
                modified: meta.modified().ok().and_then(provenance::nanos),
                sha1: entry.sha1.clone().unwrap_or_default(),
                sha512,
                modrinth,
                curseforge,
                ..provenance::Entry::default()
            },
        ));
    }
    if learned.is_empty() {
        return Ok(());
    }
    Provenance::update(&dirs.instance, |known| {
        for (kind, name, entry) in learned {
            let key = provenance::key(kind, &enabled_name(&name));
            let kept = known
                .files
                .get(&key)
                .filter(|old| old.sha512.eq_ignore_ascii_case(&entry.sha512))
                .cloned();
            let merged = match kept {
                Some(old) => provenance::Entry {
                    size: entry.size,
                    modified: entry.modified,
                    ..old
                },
                None => entry,
            };
            known.files.insert(key, merged);
        }
    })
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// A `modpack.json` follows the restored content's base when its pack version differs (see
/// the module). Nothing changes for an instance without one, a base of another pack, or the
/// same version.
fn follow_base(dirs: &InstanceDirs, base: Option<&Base>, entries: &[Entry]) -> Result<(), String> {
    let Some(mut record) = ModpackRecord::load(&dirs.instance) else {
        return Ok(());
    };
    let changed = match base {
        Some(Base::ModrinthPack {
            project,
            version,
            sha512,
        }) if *project == record.project_id && *version != record.version_id => {
            record.version_id = version.clone();
            record.pack_sha512 = Some(sha512.clone());
            record.version_number.clear();
            record.date_published.clear();
            true
        }
        Some(Base::CurseforgePack { project, file }) => match &mut record.curseforge {
            Some(pack) if pack.project_id == *project && pack.file_id != *file => {
                pack.file_id = *file;
                pack.file_date.clear();
                record.version_number.clear();
                true
            }
            _ => false,
        },
        _ => false,
    };
    if !changed {
        return Ok(());
    }
    let restored: HashSet<String> = entries
        .iter()
        .map(|e| e.path.as_str().to_ascii_lowercase())
        .collect();
    let game: PathBuf = dirs.game.clone();
    record
        .files
        .retain(|path, _| restored.contains(&path.to_ascii_lowercase()));
    for (path, file) in record.files.iter_mut() {
        let on_disk = swap::join_rel(&game, path);
        if let Ok(sha1) = crate::game::download::sha1_of(&on_disk) {
            file.sha1 = sha1;
        }
    }
    crate::storage::save(&ModpackRecord::path(&dirs.instance), &record).map_err(|e| e.to_string())
}

#[cfg(test)]
pub(crate) fn follow_base_for_tests(
    instance_dir: &Path,
    base: Option<&Base>,
    entries: &[Entry],
) -> Result<(), String> {
    follow_base(&InstanceDirs::new(instance_dir), base, entries)
}

/// How the content `now` differs from the content `then` (see [`ContentDiff`]).
pub fn diff_content(now: &[Entry], then: &[Entry]) -> ContentDiff {
    struct Seen<'a> {
        sha256: &'a str,
        disabled: bool,
        project: Option<&'a str>,
    }
    fn index(entries: &[Entry]) -> BTreeMap<String, Seen<'_>> {
        entries
            .iter()
            .map(|e| {
                let path = e.path.as_str();
                let enabled = enabled_name(path);
                let project = match &e.source {
                    Source::Modrinth { project, .. } => Some(project.as_str()),
                    _ => None,
                };
                (
                    enabled.clone(),
                    Seen {
                        sha256: &e.sha256,
                        disabled: enabled != path,
                        project,
                    },
                )
            })
            .collect()
    }
    let (now, then) = (index(now), index(then));
    let mut diff = ContentDiff::default();
    for (path, old) in &now {
        match then.get(path) {
            None => diff.removed.push(path.clone()),
            Some(new) if new.sha256 != old.sha256 || new.disabled != old.disabled => {
                diff.updated.push(path.clone())
            }
            Some(_) => {}
        }
    }
    for path in then.keys() {
        if !now.contains_key(path) {
            diff.added.push(path.clone());
        }
    }
    // The same Modrinth project removed and added is one update.
    let mut paired = Vec::new();
    diff.added.retain(|added| {
        let project = then.get(added).and_then(|s| s.project);
        let Some(project) = project else {
            return true;
        };
        match diff
            .removed
            .iter()
            .position(|removed| now.get(removed).and_then(|s| s.project) == Some(project))
        {
            Some(i) => {
                diff.removed.remove(i);
                paired.push(added.clone());
                false
            }
            None => true,
        }
    });
    diff.updated.extend(paired);
    diff.updated.sort();
    diff
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, sha: char, project: Option<&str>) -> Entry {
        let source = match project {
            Some(project) => Source::Modrinth {
                project: project.into(),
                version: "v".into(),
                url: format!("https://cdn.modrinth.com/data/{project}/versions/v/x.jar"),
            },
            None => Source::Object {},
        };
        Entry::new(
            CanonicalPath::new(path).unwrap(),
            1,
            sha.to_string().repeat(64),
            source,
        )
    }

    #[test]
    fn content_diffs_pair_projects_and_see_toggles() {
        let now = [
            entry("mods/sodium-0.6.jar", 'a', Some("AANobbMI")),
            entry("mods/jei.jar", 'b', None),
            entry("mods/extra.jar", 'c', None),
            entry("config/a.toml", 'd', None),
            entry("mods/off.jar.disabled", 'e', None),
        ];
        let then = [
            entry("mods/sodium-0.5.jar", '1', Some("AANobbMI")),
            entry("mods/jei.jar", 'b', None),
            entry("mods/old.jar", '2', None),
            entry("config/a.toml", '3', None),
            entry("mods/off.jar", 'e', None),
        ];
        let diff = diff_content(&now, &then);
        assert_eq!(diff.added, ["mods/old.jar"]);
        assert_eq!(diff.removed, ["mods/extra.jar"]);
        assert_eq!(
            diff.updated,
            ["config/a.toml", "mods/off.jar", "mods/sodium-0.5.jar"]
        );
        assert!(diff_content(&now, &now).is_empty());
    }

    #[test]
    fn journal_paths_stay_in_the_game_folder() {
        assert!(swap::valid_rel_path("minecraft/saves/World"));
        assert!(swap::valid_rel_path("minecraft/mods"));
        for bad in [
            "minecraft",
            "saves/World",
            "minecraft/../x",
            "minecraft//x",
            "minecraft/./x",
            "minecraft/a\\b",
            "/minecraft/x",
            "minecraft/a\0b",
        ] {
            assert!(!swap::valid_rel_path(bad), "{bad}");
        }
    }
}
