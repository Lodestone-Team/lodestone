//! Placing a planned view ([`super::plan`]): a new instance, staged whole beside the
//! instances and renamed into place, or an update of an instance through the modpack
//! updater (staging, a rollback copy of what it replaces, the swap), with the server's
//! objects fetched from the server and every file checked against its SHA-256.
//!
//! The order of writes for an update is: the files, `content.json` and `modpack.json`
//! (inside the updater), then the instance's loader version (the caller, on the window's
//! thread), then [`mark_applied`]: the kept view and the link's applied view. A crash
//! anywhere before the link leaves the old applied view, so the next Play plans again
//! against the `modpack.json` written so far and converges; a new instance only appears
//! once whole.

use std::fs;

use lodestone_image::Image;

use super::{SyncError, plan::ServerPlan, views::KeptViews};
use crate::{
    content::{InstanceDirs, install::InstallProgress, provenance::Provenance},
    cores::link::{self, ServerLink},
    game::download::CancelToken,
    images::{Images, labels},
    instances::{Instance, InstanceStore, Loader},
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

/// Updates the instance at `dirs` to `plan`'s view: its files, `content.json` and
/// `modpack.json`. The caller holds Play (the instance's files are busy), records
/// [`before_server_update`] first and wraps this in its operation ("Updated from
/// <server>"), saves the loader version after, and then calls [`mark_applied`].
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
