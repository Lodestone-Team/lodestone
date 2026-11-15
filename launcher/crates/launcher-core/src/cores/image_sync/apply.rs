//! Placing a planned view ([`super::plan`]): a new instance, staged whole beside the
//! instances and renamed into place, or an update of an instance through the modpack
//! updater (staging, a rollback copy of what it replaces, the plan checked again against
//! the disk, the swap), with the server's objects fetched from the server and every file
//! checked against its SHA-256.
//!
//! An update runs through [`ServerUpdate`], which holds its order: refused while the
//! instance's files are held (its game runs), the record before it ([`before_server_update`]),
//! the operation "Updated from <server>", the files, `content.json` and `modpack.json`
//! (inside the updater), then, on the window's thread, the instance's loader version and
//! the operation's success, and last, off it, [`mark_applied`]: the kept view and the
//! link's applied view. A crash anywhere before the link leaves the old applied view, so
//! the next Play plans again against the `modpack.json` written so far (the view being
//! placed is named in the kept views, so the files it placed count as the server's) and
//! converges; a new instance only appears once whole.

use std::{fs, sync::Arc};

use lodestone_image::Image;

use super::{SyncError, plan::ServerPlan, prepare::Ready, views::KeptViews};
use crate::{
    content::{InstanceDirs, install::InstallProgress, provenance::Provenance},
    cores::link::{self, ServerLink},
    game::download::CancelToken,
    images::{Images, Operation, labels},
    instances::{Instance, InstanceStore, Loader},
    modpack::ModpackRecord,
    modpack::{
        ModpackError,
        fetch::{Sources, stage},
        import::content_entries,
        update::{self, UpdateOutcome},
    },
};

/// Records the instance's content before a server's update changes its files ("Before a
/// server update"), so every content file the update replaces is in an image. The one
/// place this happens: the snapshot before a server update joins it here. A failure is
/// logged and the update goes on (it compares the files themselves, and keeps its own
/// rollback copy); whether the record was made is returned (not for an instance whose
/// images aren't kept).
pub fn before_server_update(images: &Images, instance: &str, server: &str) -> bool {
    match images.record_before_change(instance, labels::BEFORE_SERVER_UPDATE) {
        Ok(recorded) => recorded.is_some(),
        Err(err) => {
            crate::images::warn(
                "images.before_server_update",
                instance,
                &format!("Couldn't record the content before updating from {server}: {err}"),
            );
            false
        }
    }
}

/// Refuses `plan` when the data folder's disk lacks room for the files it writes
/// (staged first, then renamed in): a view may place up to 64 GB, and a hostile server
/// could otherwise fill the player's disk once they accept. Unknown free space (the
/// system wouldn't say) doesn't refuse.
pub fn check_free_space(images: &Images, plan: &ServerPlan) -> Result<(), SyncError> {
    let needed: u64 = plan.update.writes().map(|(fetch, _)| fetch.size()).sum();
    match images.free_space() {
        Some(free) if free < needed => Err(SyncError::NotEnoughSpace { needed, free }),
        _ => Ok(()),
    }
}

/// How old a staging folder an update left (`.lodestone-update-*`: a crash part way) is
/// before the next server update deletes it.
const STALE_STAGING: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// Deletes the staging folders older updates of the instance at `dirs` left behind.
fn remove_stale_staging(dirs: &InstanceDirs) {
    let Ok(entries) = fs::read_dir(&dirs.instance) else {
        return;
    };
    for entry in entries.flatten() {
        let stale = entry
            .file_name()
            .to_string_lossy()
            .starts_with(".lodestone-update-")
            && entry.file_type().is_ok_and(|t| t.is_dir())
            && entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > STALE_STAGING);
        if stale {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

fn modpack_error(err: ModpackError) -> SyncError {
    match err {
        ModpackError::Cancelled => SyncError::Cancelled,
        ModpackError::Image(err) => SyncError::Image(err),
        err => SyncError::Modpack(err),
    }
}

/// Makes a new instance named `name` from `plan` (planned for a new instance), linked to
/// its server by `link`. Nothing is left behind when it fails. The caller records its
/// first content image ("Created from <server>").
pub async fn build_instance(
    store: &InstanceStore,
    sources: &Sources<'_>,
    plan: &ServerPlan,
    name: &str,
    link: ServerLink,
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<Instance, SyncError> {
    let staging = store.new_staging().map_err(ModpackError::from)?;
    let result = build(store, sources, plan, name, link, progress, cancel, &staging).await;
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

#[allow(clippy::too_many_arguments)]
async fn build(
    store: &InstanceStore,
    sources: &Sources<'_>,
    plan: &ServerPlan,
    name: &str,
    link: ServerLink,
    progress: &InstallProgress,
    cancel: &CancelToken,
    staging: &std::path::Path,
) -> Result<Instance, SyncError> {
    let dirs = InstanceDirs::new(staging);
    let writes: Vec<_> = plan
        .update
        .writes()
        .map(|(fetch, path)| (fetch.clone(), path.to_string()))
        .collect();
    stage(
        sources,
        &dirs.game,
        &plan.update.record.title,
        &writes,
        progress,
        cancel,
    )
    .await
    .map_err(modpack_error)?;
    progress.set_label("Recording the server's files");
    let downloaded: Vec<(String, &lodestone_minecraft::mrpack::PackDownload)> = plan
        .update
        .downloads()
        .map(|(d, p)| (p.to_string(), d))
        .collect();
    let mut provenance = Provenance::default();
    for (kind, file, entry) in
        content_entries(sources.platforms.modrinth, &dirs.game, &downloaded).await
    {
        provenance.insert(kind, &file, entry);
    }
    let save = |result: Result<(), crate::storage::StorageError>| {
        result.map_err(|err| SyncError::Modpack(err.into()))
    };
    save(provenance.save(staging))?;
    save(plan.update.record.save(staging))?;
    let link = ServerLink {
        view: Some(plan.view.id.clone()),
        pack_etag: None,
        ..link
    };
    save(link::save_link(staging, &link))?;
    KeptViews::open(staging)
        .keep(&plan.view)
        .map_err(|err| SyncError::io("the server's view", err))?;
    let instance = Instance {
        name: name.to_string(),
        game_version: plan.update.game_version.clone(),
        loader: Loader::from_kind(plan.loader.as_ref().map(|l| l.kind)),
        loader_version: plan.loader.as_ref().map(|l| l.version.clone()),
        created: crate::instances::now(),
        ..Instance::default()
    };
    store
        .commit_staged(staging, instance)
        .map_err(|err| SyncError::Modpack(err.into()))
}

/// Bringing a linked instance to a server's view (the Launcher's step 4), in the order
/// its writes must happen. [`Self::start`] refuses while the instance's files are held;
/// [`Self::place`] (on a background thread) records the content first and places the
/// files inside the operation "Updated from <server>"; [`PlacedUpdate::finish`] (on the
/// window's thread, where the instance is saved) saves the loader version and lets the
/// operation succeed; [`AppliedUpdate::record`] names the view in the link. A view that
/// changes nothing on disk
/// (the files are the view's already: a crash before the link, or a view that only lacks
/// files the player was told about) is only recorded: its `modpack.json` if that names
/// other files, then the link.
pub struct ServerUpdate {
    images: Arc<Images>,
    instance: Instance,
    server: String,
    dirs: InstanceDirs,
    ready: Arc<Ready>,
}

impl ServerUpdate {
    /// An update of `instance` (at `dirs`) to `ready`'s view from the server `server`
    /// (its name, for labels), unless `held` says why its files can't change now (its game
    /// runs, say: the caller knows), which is given back.
    pub fn start<W>(
        images: Arc<Images>,
        instance: Instance,
        server: &str,
        dirs: InstanceDirs,
        ready: Arc<Ready>,
        held: Option<W>,
    ) -> Result<ServerUpdate, W> {
        if let Some(why) = held {
            return Err(why);
        }
        Ok(ServerUpdate {
            images,
            instance,
            server: server.to_string(),
            dirs,
            ready,
        })
    }

    /// Places the view's files (from `sources`): first the record before a server update,
    /// then the files inside the operation, which succeeds part way when placing fails
    /// with files changed part way, and records nothing when it failed changing nothing.
    pub async fn place(
        self,
        sources: &Sources<'_>,
        progress: &InstallProgress,
        cancel: &CancelToken,
    ) -> Result<PlacedUpdate, SyncError> {
        let id = self.instance.id.clone();
        if !self.ready.changes(Some(&self.instance)) {
            let record = &self.ready.plan.update.record;
            if ModpackRecord::load(&self.dirs.instance).as_ref() != Some(record) {
                // a change of which files are content, recorded without a label
                let operation = self.images.begin_rescope(&id);
                let saved = record.save(&self.dirs.instance);
                operation.finish(&saved);
                saved.map_err(|err| SyncError::Modpack(err.into()))?;
            }
            return Ok(PlacedUpdate {
                operation: None,
                dirs: self.dirs,
                ready: self.ready,
            });
        }
        remove_stale_staging(&self.dirs);
        check_free_space(&self.images, &self.ready.plan)?;
        let views = KeptViews::open(&self.dirs.instance);
        views
            .begin_placing(&self.ready.plan.view)
            .map_err(|err| SyncError::io("the server's view", err))?;
        // what the record before the update holds: the content folders and the files the
        // instance's packs wrote, as they are before it
        let scope = crate::images::context::scope_of(&self.dirs.instance);
        let recorded = before_server_update(&self.images, &id, &self.server);
        let operation = self.images.begin(&id, labels::server_updated(&self.server));
        match update_instance(sources, &self.dirs, &self.ready.plan, progress, cancel).await {
            Ok(outcome) => {
                // When the record before the update holds every file it replaced or
                // removed, its rollback copy, there only to undo a swap that fails part
                // way, isn't kept: a server that updates weekly would grow it for ever.
                // A file outside the record's scope (the player's own script at a path
                // the server's pack now places) is only in the rollback copy, which stays.
                let held = recorded
                    && self
                        .ready
                        .plan
                        .update
                        .touched()
                        .iter()
                        .all(|path| scope.holds_file(path));
                if held {
                    let _ = fs::remove_dir_all(&outcome.snapshot);
                }
                Ok(PlacedUpdate {
                    operation: Some(operation),
                    dirs: self.dirs,
                    ready: self.ready,
                })
            }
            Err(err @ SyncError::Modpack(ModpackError::NotUndone(_))) => {
                // the files changed part way: the files it placed are the server's
                operation.succeed_part_way();
                Err(err)
            }
            Err(err) => {
                // nothing changed (it stopped before the files, or put back what it had
                // changed): no image, and the view isn't being placed any more, so the
                // next plan doesn't take the player's own copy of one of its files for
                // the server's
                drop(operation);
                views.done_placing();
                Err(err)
            }
        }
    }
}

/// A server's view whose files are placed: what is left of its update.
#[must_use = "the view isn't applied until it is finished"]
pub struct PlacedUpdate {
    operation: Option<Operation>,
    dirs: InstanceDirs,
    ready: Arc<Ready>,
}

impl PlacedUpdate {
    /// The loader version the instance runs now: the view's.
    pub fn loader_version(&self) -> Option<String> {
        self.ready.plan.loader.as_ref().map(|l| l.version.clone())
    }

    /// Ends the update on the window's thread: `save_loader` saves the instance's loader
    /// version (only the loader version follows a server, never the Minecraft version or
    /// the loader, which the plan checked are the instance's), and the operation succeeds
    /// (once the loader is saved, so its image has it; whatever the save did, since the
    /// files changed). When the save worked, what is left is naming the view, last of all
    /// and off the window's thread ([`AppliedUpdate::record`]).
    pub fn finish(
        self,
        save_loader: impl FnOnce(Option<String>) -> Result<(), String>,
    ) -> Result<AppliedUpdate, SyncError> {
        let saved = save_loader(self.loader_version());
        if let Some(operation) = self.operation {
            operation.succeed();
        }
        saved.map_err(SyncError::Io)?;
        Ok(AppliedUpdate {
            dirs: self.dirs,
            ready: self.ready,
        })
    }
}

/// A server's view whose files and loader version are in place: only its record is left.
#[must_use = "the view isn't applied until it is recorded"]
pub struct AppliedUpdate {
    dirs: InstanceDirs,
    ready: Arc<Ready>,
}

impl AppliedUpdate {
    /// Keeps the view and names it in the link ([`mark_applied`]): the update's last
    /// write. It writes (and syncs) the whole view, so it runs off the window's thread.
    pub fn record(self) -> Result<(), SyncError> {
        mark_applied(&self.dirs, &self.ready.plan.view)
    }
}

/// Updates the instance at `dirs` to `plan`'s view: its files, `content.json` and
/// `modpack.json`, without the record before it or the operation. [`ServerUpdate::place`]
/// runs it, in its operation; outside that, only tests of what a crash part way leaves
/// call it (the app always goes through [`ServerUpdate`]).
#[doc(hidden)]
pub async fn update_instance(
    sources: &Sources<'_>,
    dirs: &InstanceDirs,
    plan: &ServerPlan,
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<UpdateOutcome, SyncError> {
    update::apply_from(sources, dirs, &plan.update, false, progress, cancel)
        .await
        .map_err(modpack_error)
}

/// Records that the instance at `dirs` applied `view`: keeps the view, then names it in the
/// server link, then prunes the kept views. Last, after everything else an update writes.
pub fn mark_applied(dirs: &InstanceDirs, view: &Image) -> Result<(), SyncError> {
    let views = KeptViews::open(&dirs.instance);
    views
        .keep_once(view)
        .map_err(|err| SyncError::io("the server's view", err))?;
    let mut link = link::load_link(&dirs.instance)
        .ok_or_else(|| SyncError::Io("The instance isn't linked to a server any more.".into()))?;
    link.view = Some(view.id.clone());
    link.pack_etag = None;
    link::save_link(&dirs.instance, &link).map_err(|err| SyncError::Modpack(err.into()))?;
    views.done_placing();
    views.prune(&view.id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use lodestone_image::{Game, Kind, Loader as ImageLoader};

    use super::*;
    use crate::{
        images::Timing,
        modpack::{
            FileSource, PackFile,
            merge::{self, Digest, Fetch, Wanted},
        },
        paths::DataDir,
    };

    /// A view whose files need more room than the disk has is refused before anything
    /// is downloaded.
    #[test]
    fn a_plan_larger_than_the_free_space_is_refused() {
        let temp = tempfile::tempdir().unwrap();
        let images = Images::with_timing(&DataDir::at(temp.path().join("data")), Timing::default());
        let sha256 = "ab".repeat(32);
        let new: BTreeMap<String, Wanted> = [(
            "mods/big.jar".to_string(),
            Wanted {
                digest: Digest::Sha256(sha256.clone()),
                record: PackFile {
                    sha1: String::new(),
                    source: FileSource::Index,
                    project: None,
                    sha256: Some(sha256.clone()),
                },
                fetch: Fetch::Object {
                    sha256,
                    size: 10_000,
                },
            },
        )]
        .into();
        let update = update::plan_files(
            None,
            &ModpackRecord::default(),
            &new,
            ModpackRecord::default(),
            merge::Rules::default(),
        );
        let plan = ServerPlan {
            view: Image::new(
                Kind::Content,
                Game {
                    minecraft: "1.21.1".into(),
                    loader: ImageLoader::Vanilla,
                    loader_version: None,
                },
            ),
            update,
            summary: Default::default(),
            loader: None,
        };
        *images.fake_free.lock().unwrap() = Some(5_000);
        assert!(matches!(
            check_free_space(&images, &plan),
            Err(SyncError::NotEnoughSpace {
                needed: 10_000,
                free: 5_000
            })
        ));
        *images.fake_free.lock().unwrap() = Some(20_000);
        assert!(check_free_space(&images, &plan).is_ok());
    }
}
