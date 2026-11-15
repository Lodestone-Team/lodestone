//! Asking a server for its image and working out what installing it does, before anything
//! changes ("Server to player", the Launcher's steps 1 and 4): for "Create matching
//! instance" (no instance yet) and for Play (an instance linked to the server).

use std::{collections::HashMap, path::Path};

use lodestone_image::{Image, Source};

use super::{
    SyncError,
    base::{BasePack, base_pack},
    placed::{CurseForgeFile, placed},
    plan::{ServerPlan, loader_of, plan},
    received::{apply_delta, check_view},
    views::KeptViews,
};
use crate::{
    content::{CurseForge, InstanceDirs, Modrinth, install::InstallProgress},
    cores::player_image::{ImageAnswer, ServerImage},
    game::download::{CancelToken, Downloader},
    instances::Instance,
    modpack::ModpackRecord,
};

/// The instance a view is for: an existing one linked to the server, or a new one.
pub struct Target<'a> {
    /// The instance's folders and `instance.json`; `None` for a new instance.
    pub existing: Option<(&'a InstanceDirs, &'a Instance)>,
    /// The view the instance applied last (its link's).
    pub applied: Option<&'a str>,
    /// The server's name, for a server without a base pack.
    pub name: &'a str,
}

/// Where a view's files are looked up.
#[derive(Clone, Copy)]
pub struct Lookup<'a> {
    pub modrinth: Option<&'a Modrinth>,
    pub curseforge: Option<&'a CurseForge>,
    pub downloader: &'a Downloader,
    /// The base packs' cache ([`crate::paths::DataDir::server_pack_cache_dir`]).
    pub cache: &'a Path,
}

/// A view ready to install.
#[derive(Clone, Debug)]
pub struct Ready {
    pub plan: ServerPlan,
    /// The base pack, whose overrides the install copies.
    pub base: Option<BasePack>,
}

impl Ready {
    /// Whether installing changes anything: files, or the instance's loader version.
    pub fn changes(&self, instance: Option<&Instance>) -> bool {
        let loader = self.plan.loader.as_ref().map(|l| l.version.as_str());
        self.plan.update.changes_files()
            || instance.is_some_and(|i| i.loader_version.as_deref() != loader)
    }
}

/// What the server's image means for the instance.
#[derive(Clone, Debug)]
pub enum Prepared {
    /// The instance's applied view is current: join at once.
    Current,
    /// Another view to place (or the same, worked out again): see [`Ready::changes`].
    Ready(Box<Ready>),
}

/// Asks `server` for its image, naming the views `target` keeps, checks what comes, and
/// plans placing it. `on_preparing` is called each time the server says it is still
/// preparing its files.
pub async fn prepare(
    server: &ServerImage,
    target: Target<'_>,
    lookup: Lookup<'_>,
    on_preparing: &(dyn Fn() + Send + Sync),
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<Prepared, SyncError> {
    let views = target
        .existing
        .map(|(dirs, _)| KeptViews::open(&dirs.instance));
    let have = views
        .as_ref()
        .map(|v| v.have(target.applied))
        .unwrap_or_default();
    progress.set_label("Asking the server for its files");
    let mut answer = server.image(&have, on_preparing, cancel).await?;
    let mut asked_again = false;
    let view = loop {
        let kept = |id: &str| views.as_ref().and_then(|v| v.load(id));
        let found = match &answer {
            ImageAnswer::Current { view } => {
                if Some(view.as_str()) == target.applied
                    && let Some((_, instance)) = target.existing
                    && let Some(applied) = kept(view)
                    && game_matches(instance, &applied)
                {
                    return Ok(Prepared::Current);
                }
                // another view it keeps is current: switch to it
                kept(view)
            }
            ImageAnswer::View { image, view } => {
                check_view(image, view)?;
                Some((**image).clone())
            }
            ImageAnswer::Delta { delta, view } => match kept(&delta.from) {
                Some(from) => Some(apply_delta(&from, delta, view)?),
                None => None,
            },
        };
        match found {
            Some(view) => break view,
            // it named a view this instance doesn't keep after all: the whole view
            None if !asked_again => {
                asked_again = true;
                answer = server.image(&[], on_preparing, cancel).await?;
            }
            None => {
                return Err(SyncError::Refused(
                    "it answered with a view this Launcher didn't ask about".into(),
                ));
            }
        }
    };
    let base = base_pack(
        lookup.modrinth,
        lookup.downloader,
        lookup.cache,
        view.base.as_ref(),
        progress,
        cancel,
    )
    .await?;
    let curseforge = resolve_curseforge(lookup.curseforge, &view).await;
    let placed = placed(&view, base.as_ref().map(|b| &b.pack), &curseforge)?;
    let current = target
        .existing
        .and_then(|(dirs, _)| ModpackRecord::load(&dirs.instance))
        .unwrap_or_default();
    let plan = plan(
        target.existing,
        &current,
        view,
        placed,
        base.as_ref(),
        target.name,
    )?;
    Ok(Prepared::Ready(Box::new(Ready { plan, base })))
}

/// Whether the instance runs the game `view` names, loader version included (an update
/// whose loader version wasn't saved is placed again).
fn game_matches(instance: &Instance, view: &Image) -> bool {
    let Ok(loader) = loader_of(view) else {
        return false;
    };
    instance.game_version == view.game.minecraft
        && instance.loader == crate::instances::Loader::from_kind(loader.as_ref().map(|l| l.kind))
        && instance.loader_version.as_deref() == loader.as_ref().map(|l| l.version.as_str())
}

/// Where the CurseForge files `view` names that may be downloaded come from, asked of
/// CurseForge in one request; one it doesn't know or that can't be downloaded is left out
/// (the player is told to get it by hand).
async fn resolve_curseforge(
    curseforge: Option<&CurseForge>,
    view: &Image,
) -> HashMap<(u64, u64), CurseForgeFile> {
    let wanted: Vec<(u64, u64)> = view
        .entries
        .iter()
        .filter_map(|entry| match entry.source {
            Source::Curseforge {
                project,
                file,
                downloadable: true,
            } => Some((project, file)),
            _ => None,
        })
        .collect();
    let mut found = HashMap::new();
    let Some(curseforge) = curseforge.filter(|_| !wanted.is_empty()) else {
        return found;
    };
    let ids: Vec<u64> = wanted.iter().map(|(_, file)| *file).collect();
    let Ok(files) = curseforge.files_by_id(&ids).await else {
        return found;
    };
    for file in files {
        if !wanted.contains(&(file.mod_id, file.id)) || !file.downloadable() {
            continue;
        }
        let (Ok(url), Some(sha1)) = (curseforge.download_url(&file), file.sha1()) else {
            continue;
        };
        if let Ok(url) = url::Url::parse(&url) {
            found.insert((file.mod_id, file.id), CurseForgeFile { url, sha1 });
        }
    }
    found
}
