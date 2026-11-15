//! Placing a planned view ([`super::plan`]): a new instance, staged whole beside the
//! instances and renamed into place, or an update of an instance through the modpack
//! updater (staging, a rollback copy of what it replaces, the swap), with the server's
//! objects fetched from the server and every file checked against its SHA-256.
//!
//! An update runs through [`ServerUpdate`], which holds its order: refused while the
//! instance's files are held (its game runs), the record before it ([`before_server_update`]),
//! the operation "Updated from <server>", the files, `content.json` and `modpack.json`
//! (inside the updater), then, on the window's thread, the instance's loader version, the
//! operation's success, and last [`mark_applied`]: the kept view and the link's applied
//! view. A crash anywhere before the link leaves the old applied view, so the next Play
//! plans again against the `modpack.json` written so far and converges; a new instance
//! only appears once whole.

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
/// server update"), so every file the update replaces is in an image. The one place this
/// happens: the snapshot before a server update joins it here. A failure is logged and
/// the update goes on (it compares the files themselves).
pub fn before_server_update(images: &Images, instance: &str, server: &str) {
    if let Err(err) = images.record_before_change(instance, labels::BEFORE_SERVER_UPDATE) {
        crate::images::warn(
            "images.before_server_update",
            instance,
            &format!("Couldn't record the content before updating from {server}: {err}"),
        );
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
/// window's thread, where the instance is saved) saves the loader version, lets the
/// operation succeed and names the view in the link. A view that changes nothing on disk
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
    /// then the files inside the operation, which succeeds part way when placing fails.
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
        before_server_update(&self.images, &id, &self.server);
        let operation = self.images.begin(&id, labels::server_updated(&self.server));
        match update_instance(sources, &self.dirs, &self.ready.plan, progress, cancel).await {
            Ok(_) => Ok(PlacedUpdate {
                operation: Some(operation),
                dirs: self.dirs,
                ready: self.ready,
            }),
            Err(err) => {
                // the files changed part way, or not at all
                operation.succeed_part_way();
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

    /// Ends the update: `save_loader` saves the instance's loader version (only the loader
    /// version follows a server, never the Minecraft version or the loader, which the plan
    /// checked are the instance's), the operation succeeds (once the loader is saved, so
    /// its image has it; whatever the save did, since the files changed), and then, when
    /// the save worked, the view is applied: last of all.
    pub fn finish(
        self,
        save_loader: impl FnOnce(Option<String>) -> Result<(), String>,
    ) -> Result<(), SyncError> {
        let saved = save_loader(self.loader_version());
        if let Some(operation) = self.operation {
            operation.succeed();
        }
        saved.map_err(SyncError::Io)?;
        mark_applied(&self.dirs, &self.ready.plan.view)
    }
}

/// Updates the instance at `dirs` to `plan`'s view: its files, `content.json` and
/// `modpack.json`. [`ServerUpdate::place`] runs it, in its operation; outside that, only
/// for tests of what a crash part way leaves.
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
        .keep(view)
        .map_err(|err| SyncError::io("the server's view", err))?;
    let mut link = link::load_link(&dirs.instance)
        .ok_or_else(|| SyncError::Io("The instance isn't linked to a server any more.".into()))?;
    link.view = Some(view.id.clone());
    link.pack_etag = None;
    link::save_link(&dirs.instance, &link).map_err(|err| SyncError::Modpack(err.into()))?;
    views.prune(&view.id);
    Ok(())
}
