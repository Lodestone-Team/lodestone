//! Asking a server for its image and working out what installing it does, before anything
//! changes ("Server to player", the Launcher's steps 1 and 4): for "Create matching
//! instance" (no instance yet) and for Play (an instance linked to the server).

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::Path,
};

use lodestone_image::{Image, Source, paths::is_config};

use super::{
    SyncError,
    base::{BasePack, base_pack},
    placed::{BaseShare, CurseForgeFile, needs_server_copy, placed},
    plan::{AUTHORITATIVE, ServerPlan, loader_of, plan},
    received::{apply_delta, check_view},
    views::KeptViews,
};
use crate::{
    content::{CurseForge, InstanceDirs, Modrinth, install::InstallProgress},
    content::{disabled_name, is_disabled_name},
    cores::player_image::{ImageAnswer, ServerImage, sha256_of_file},
    game::download::{CancelToken, Downloader},
    instances::Instance,
    modpack::{FileSource, ModpackRecord, PackFile, import::sha1_on_disk, merge::Digest},
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
    /// Files the player lacks with this view that they didn't with the view the instance
    /// applied last (paths as shown): withheld files a player would need, and CurseForge
    /// files to download by hand. Empty for a new instance, which shows them all anyway.
    pub newly_lacking: Vec<String>,
}

impl Ready {
    /// Whether installing changes anything: files, or the instance's loader version.
    pub fn changes(&self, instance: Option<&Instance>) -> bool {
        let loader = self.plan.loader.as_ref().map(|l| l.version.as_str());
        self.plan.update.changes_files()
            || instance.is_some_and(|i| i.loader_version.as_deref() != loader)
    }

    /// Whether the player must be asked before playing: installing changes something, or
    /// the view lacks files the applied one didn't ("Files a view can't give": a player
    /// says so before it plays). A view that does neither is only recorded as applied.
    pub fn asks(&self, instance: Option<&Instance>) -> bool {
        self.changes(instance) || !self.newly_lacking.is_empty()
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
    // the server says the instance is in step, but its mods aren't all on disk: every
    // file is read when planning, not taken as unchanged by its record
    let mut mods_changed = false;
    let view = loop {
        let kept = |id: &str| views.as_ref().and_then(|v| v.load(id));
        let found = match answer {
            ImageAnswer::Current { view } => {
                if Some(view.as_str()) == target.applied
                    && let Some((dirs, instance)) = target.existing
                    && let Some(applied) = kept(&view)
                    && game_matches(instance, &applied)
                {
                    if server_mods_on_disk(dirs, &cant_give(&applied)) {
                        return Ok(Prepared::Current);
                    }
                    mods_changed = true;
                }
                // another view it keeps is current: switch to it; or the applied one, whose
                // mods aren't all on disk any more, placed again
                kept(&view)
            }
            ImageAnswer::View { image, view } => {
                check_view(&image, &view)?;
                Some(*image)
            }
            ImageAnswer::Delta { delta, view } => match kept(&delta.from) {
                Some(from) => Some(apply_delta(&from, &delta, &view)?),
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
    let curseforge = resolve_curseforge(lookup.curseforge, &view).await?;
    let server_copies = match &base {
        Some(base) => base.server_copies(&needs_server_copy(&view, &base.pack))?,
        None => Vec::new(),
    };
    let share = base.as_ref().map(|base| BaseShare {
        pack: &base.pack,
        server_overrides: &server_copies,
    });
    let placed = placed(&view, share, &curseforge)?;
    let mut current = target
        .existing
        .and_then(|(dirs, _)| ModpackRecord::load(&dirs.instance))
        .unwrap_or_default();
    if let (Some(views), Some((dirs, _))) = (&views, target.existing)
        && let Some(placing) = views.placing()
        && Some(placing.id.as_str()) != target.applied
    {
        with_interrupted(&mut current, &placing, &dirs.game);
    }
    let plan = plan(
        target.existing,
        &current,
        view,
        placed,
        base.as_ref(),
        target.name,
        !mods_changed,
    )?;
    let newly_lacking = match &views {
        Some(views) => {
            let applied = target.applied.and_then(|id| views.load(id));
            newly_lacking(&plan, applied.as_ref(), &current)
        }
        None => Vec::new(),
    };
    Ok(Prepared::Ready(Box::new(Ready {
        plan,
        base,
        newly_lacking,
    })))
}

/// The files `plan`'s view lacks that `applied` (the view the instance applied last,
/// whose placed files `current` records) didn't: needed withheld files and CurseForge
/// files to download by hand, as the summary shows them. Without an applied view, all of
/// them.
fn newly_lacking(
    plan: &ServerPlan,
    applied: Option<&Image>,
    current: &ModpackRecord,
) -> Vec<String> {
    let mut before: BTreeSet<String> = BTreeSet::new();
    if let Some(applied) = applied {
        before.extend(
            lodestone_image::view::needed_withheld(applied)
                .into_iter()
                .map(|w| w.display_path().into_owned()),
        );
        // a CurseForge file the applied view named that it didn't place was listed
        before.extend(
            applied
                .entries
                .iter()
                .filter(|e| matches!(e.source, Source::Curseforge { .. }))
                .map(|e| e.path.as_str())
                .filter(|path| !current.files.contains_key(*path))
                .map(str::to_string),
        );
    }
    let summary = &plan.summary;
    summary
        .missing
        .iter()
        .map(|(path, _)| path.clone())
        .chain(summary.curseforge_listed.iter().cloned())
        .filter(|path| !before.contains(path))
        .collect()
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

/// The paths `view` lists but can't give (needed withheld files, CurseForge files that
/// may not be downloaded): a copy the player kept stays theirs, so it isn't checked.
fn cant_give(view: &Image) -> BTreeSet<String> {
    lodestone_image::view::needed_withheld(view)
        .into_iter()
        .map(|w| w.path.clone())
        .chain(
            view.entries
                .iter()
                .filter(|e| {
                    matches!(
                        e.source,
                        Source::Curseforge {
                            downloadable: false,
                            ..
                        }
                    )
                })
                .map(|e| e.path.as_str().to_string()),
        )
        .collect()
}

/// The file in the kept views' folder that remembers the size and modification time of
/// each server mod the last check found as placed ([`server_mods_on_disk`]).
const CHECKED_FILE: &str = "mods-checked.json";
/// The most of it that is read.
const MAX_CHECKED_BYTES: u64 = 16 << 20;
/// A file modified this recently isn't remembered: a change within the clock's
/// resolution after the check would leave its time as it was.
const TOO_RECENT: std::time::Duration = std::time::Duration::from_secs(2);

/// A mod as the last check found it: its size, modification time (nanoseconds since
/// 1970) and the hash it was placed with (`sha256:` or `sha1:` and hex).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct CheckedStat {
    size: u64,
    modified: u64,
    digest: String,
}

/// A file's size and modification time, unless it was modified too recently to trust.
fn stat_of(meta: &fs::Metadata) -> Option<(u64, u64)> {
    let modified = meta.modified().ok()?;
    if modified.elapsed().map_or(true, |age| age < TOO_RECENT) {
        return None;
    }
    let nanos = modified
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some((meta.len(), u64::try_from(nanos).ok()?))
}

/// Whether every mod the instance's `modpack.json` says the applied view placed is on
/// disk as placed (a mod the player turned off counts), but those at `skip` (files the
/// view can't give, which the player may keep as they like): when one is missing or
/// changed, the server's answer that the instance is in step isn't taken, and the
/// applied view is placed again, so the server's `mods/` stays authoritative and the
/// player can join. Without a record, the view placed nothing to check.
///
/// A 204 is the common Play, so this mustn't read every mod each time: a mod whose size
/// and modification time are those the last check remembered (`mods-checked.json`, with
/// the hash it was placed with) isn't read again, as the image store's stat cache does
/// for snapshots.
fn server_mods_on_disk(dirs: &InstanceDirs, skip: &BTreeSet<String>) -> bool {
    let Some(record) = ModpackRecord::load(&dirs.instance) else {
        return true;
    };
    let checked_path = dirs
        .instance
        .join(super::views::VIEWS_FOLDER)
        .join(CHECKED_FILE);
    let remembered: BTreeMap<String, CheckedStat> = fs::File::open(&checked_path)
        .ok()
        .and_then(|file| {
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(
                &mut std::io::Read::take(file, MAX_CHECKED_BYTES),
                &mut bytes,
            )
            .ok()?;
            serde_json::from_slice(&bytes).ok()
        })
        .unwrap_or_default();
    let mut found: BTreeMap<String, CheckedStat> = BTreeMap::new();
    let all = record
        .files
        .iter()
        .filter(|(path, _)| {
            AUTHORITATIVE
                .iter()
                .any(|dir| path.starts_with(&format!("{dir}/")))
                && !skip.contains(*path)
        })
        .all(|(path, file)| {
            let digest = file.digest();
            let tag = match &digest {
                Digest::Sha256(hex) => format!("sha256:{}", hex.to_ascii_lowercase()),
                Digest::Sha1(hex) => format!("sha1:{}", hex.to_ascii_lowercase()),
            };
            let mut candidates = vec![path.clone()];
            if !is_disabled_name(path) {
                candidates.push(disabled_name(path));
            }
            candidates.into_iter().any(|candidate| {
                let on_disk = dirs.game.join(&candidate);
                let Ok(meta) = fs::symlink_metadata(&on_disk) else {
                    return false;
                };
                if !meta.is_file() {
                    return false;
                }
                let stat = stat_of(&meta).map(|(size, modified)| CheckedStat {
                    size,
                    modified,
                    digest: tag.clone(),
                });
                let same = stat.is_some() && remembered.get(&candidate) == stat.as_ref();
                let placed = same
                    || match &digest {
                        Digest::Sha256(want) => sha256_of_file(&on_disk)
                            .is_ok_and(|have| have.eq_ignore_ascii_case(want)),
                        Digest::Sha1(want) => sha1_on_disk(&on_disk)
                            .is_some_and(|have| have.eq_ignore_ascii_case(want)),
                    };
                if placed && let Some(stat) = stat {
                    found.insert(candidate, stat);
                }
                placed
            })
        });
    if all && found != remembered {
        // only a cache: a failure to save it only means reading the mods next time
        if let Ok(json) = serde_json::to_vec(&found) {
            let _ = fs::create_dir_all(checked_path.parent().unwrap_or(&dirs.instance));
            let _ = crate::storage::write_atomic(&checked_path, &json);
        }
    }
    all
}

/// Adds to `current` (what `modpack.json` says the applied view placed) the files of
/// `placing`, the view an update was placing when it stopped before its records, that are
/// on disk as that view names them: the server's files, placed by the update, which the
/// next view may drop. Without this, a file the stopped update placed would look changed
/// by the player (it isn't what `modpack.json` records), and stay for ever.
fn with_interrupted(current: &mut ModpackRecord, placing: &Image, game: &Path) {
    for entry in &placing.entries {
        let path = entry.path.as_str();
        let recorded = current
            .files
            .get(path)
            .and_then(|file| file.sha256.as_deref())
            .is_some_and(|sha256| sha256.eq_ignore_ascii_case(&entry.sha256));
        if recorded {
            continue;
        }
        let on_disk = game.join(path);
        let placed = fs::symlink_metadata(&on_disk)
            .is_ok_and(|meta| meta.is_file() && meta.len() == entry.size)
            && sha256_of_file(&on_disk)
                .is_ok_and(|sha256| sha256.eq_ignore_ascii_case(&entry.sha256));
        if !placed {
            continue;
        }
        current.files.insert(
            path.to_string(),
            PackFile {
                sha1: entry.sha1.clone().unwrap_or_default(),
                source: if is_config(&entry.path) {
                    FileSource::Override
                } else {
                    FileSource::Index
                },
                project: match &entry.source {
                    Source::Modrinth { project, .. } => Some(project.clone()),
                    _ => None,
                },
                sha256: Some(entry.sha256.clone()),
            },
        );
    }
}

/// At most this many CurseForge files of one view are looked up (500 a request): the rest
/// are listed for the player to download by hand, so a hostile view can't spend the
/// Launcher's CurseForge quota.
pub const MAX_CURSEFORGE_LOOKUPS: usize = 2_000;

/// Where the CurseForge files `view` names that may be downloaded come from, asked of
/// CurseForge in one request per 500. One CurseForge doesn't have, or has no download
/// for, is left out (the player is told to get it by hand); CurseForge being out of
/// reach (offline, an error, no CurseForge in this build) is an error, since files it
/// would give aren't files their authors keep from other apps.
async fn resolve_curseforge(
    curseforge: Option<&CurseForge>,
    view: &Image,
) -> Result<HashMap<(u64, u64), CurseForgeFile>, SyncError> {
    let wanted: HashSet<(u64, u64)> = view
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
        .take(MAX_CURSEFORGE_LOOKUPS)
        .collect();
    let mut found = HashMap::new();
    if wanted.is_empty() {
        return Ok(found);
    }
    let curseforge = curseforge.ok_or_else(|| {
        SyncError::CurseForgeUnavailable(format!(
            "{} has no CurseForge connection now",
            crate::wording::app_name()
        ))
    })?;
    let ids: Vec<u64> = wanted
        .iter()
        .map(|(_, file)| *file)
        .collect::<BTreeSet<u64>>()
        .into_iter()
        .collect();
    let files = curseforge
        .files_by_id(&ids)
        .await
        .map_err(|err| SyncError::CurseForgeUnavailable(err.to_string()))?;
    for file in files {
        if !wanted.contains(&(file.mod_id, file.id)) || !file.downloadable() {
            continue;
        }
        // the URL CurseForge gave, once its host is one of CurseForge's CDN: the download
        // asks the CurseForge client for it again, as for a CurseForge pack's files
        let (Ok(_), Some(sha1)) = (curseforge.download_url(&file), file.sha1()) else {
            continue;
        };
        if let Some(url) = file
            .download_url
            .as_deref()
            .and_then(|u| url::Url::parse(u).ok())
        {
            found.insert((file.mod_id, file.id), CurseForgeFile { url, sha1 });
        }
    }
    Ok(found)
}
