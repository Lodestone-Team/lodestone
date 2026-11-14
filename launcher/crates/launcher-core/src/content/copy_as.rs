//! "Copy with another version or loader": a new instance made from an existing one, for
//! another Minecraft version or mod loader. An instance's version and loader are fixed
//! once it is made: mods for one loader or version don't load on another, worlds lose
//! modded blocks, and an older version can't open a newer world. So moving on makes a
//! new instance, and the original stays as it is.
//!
//! What goes across, each part by choice:
//!
//! - worlds (`saves/`), only to the same or a newer Minecraft version: the game upgrades
//!   a world when it opens it, and the upgraded world can't go back
//!   ([`worlds_rule`]);
//! - resource packs, shader packs and `options.txt` (its pack ids named as the new
//!   version names them), with the shader pack in use and whether the instance uses
//!   the shared game settings (and which of its own it keeps), so the next launch
//!   doesn't write the shared ones over the options copied;
//! - the server list (`servers.dat`), always: it isn't tied to a version;
//! - mods Modrinth knows (from `content.json`, after a lookup by SHA-1), as each
//!   project's build for the new version and loader ([`resolve_mods`]), installed fresh
//!   through the usual Modrinth path (hashes checked) with any required mods they need
//!   now. A mod that was off stays off. Mods with no such build, and mods Modrinth
//!   doesn't know (or that couldn't be checked), stay behind, unless another mod
//!   requires them. `config/` stays: the new builds' settings can differ. Vanilla runs
//!   no mods, so none goes to it, and none counts as staying behind.
//!
//! The settings that aren't tied to the version (memory, arguments, window, group,
//! icon, background) come along; play time, the modpack and a linked server don't. The
//! copy is built in a hidden folder and renamed into place, so a failed or cancelled
//! copy leaves nothing behind (a world being copied stops at once when cancelled, and
//! worlds the disk has no room for aren't started). Something deleted from the
//! original since it was read is skipped. The loader installs the next time the copy
//! is played.

use std::{cmp::Ordering, fs, path::Path};

use futures::future;
use lodestone_minecraft::modrinth::RawVersion;

use super::{
    ContentError, ContentKind, InstanceDirs, Modrinth, ModrinthError, Target, files,
    install::{self, InstallProgress, PlannedFile},
    io_error, listing, options,
    provenance::Provenance,
};
use crate::{
    game::{
        download::{CancelToken, DownloadError, Downloader},
        loaders::LoaderChoice,
    },
    instances::{
        self, BACKGROUND_FILE, ICON_FILE, Instance, InstanceError, InstanceStore, Loader,
        SLIDESHOW_DIR,
    },
    shared_options, storage,
};

/// How many projects are looked up on Modrinth at once.
const PARALLEL_LOOKUPS: usize = 6;

/// The game's list of multiplayer servers, which goes into every copy.
const SERVERS_FILE: &str = "servers.dat";

/// What goes into the copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CopyAsOptions {
    /// The worlds in `saves/` (only to the same or a newer version).
    pub worlds: bool,
    /// Resource packs, shader packs and `options.txt`.
    pub packs: bool,
    /// The mods Modrinth has a build of for the new version and loader.
    pub mods: bool,
}

impl Default for CopyAsOptions {
    fn default() -> Self {
        Self {
            worlds: false,
            packs: true,
            mods: true,
        }
    }
}

/// Whether worlds can go from one Minecraft version to another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldsRule {
    /// The same version: the worlds open as they are.
    Same,
    /// A newer version: the game upgrades each world when it opens it, and the old
    /// version can't open it again.
    Newer,
    /// An older version, which can't open them.
    Older,
    /// Versions that can't be put in order (one isn't known): worlds stay, to be safe.
    Unknown,
}

impl WorldsRule {
    pub fn allowed(self) -> bool {
        matches!(self, WorldsRule::Same | WorldsRule::Newer)
    }
}

/// Whether a version id looks like a release ("1.21.1", "26.1"): numbers and dots.
fn release_like(id: &str) -> bool {
    !id.is_empty()
        && id
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether worlds can go from Minecraft `from` to `to`. `newest_first` is the version
/// manifest's order (the launcher's version list), which places snapshots and old
/// versions too; without it (or for a version it doesn't list) two releases are
/// compared by number.
pub fn worlds_rule(from: &str, to: &str, newest_first: &[&str]) -> WorldsRule {
    if from == to {
        return WorldsRule::Same;
    }
    let place = |id: &str| newest_first.iter().position(|v| *v == id);
    let order = match (place(from), place(to)) {
        // a smaller index is newer
        (Some(from), Some(to)) => Some(from.cmp(&to)),
        _ if release_like(from) && release_like(to) => {
            Some(lodestone_minecraft::versions::compare_versions(to, from))
        }
        _ => None,
    };
    match order {
        Some(Ordering::Greater) => WorldsRule::Newer,
        Some(Ordering::Less) => WorldsRule::Older,
        Some(Ordering::Equal) => WorldsRule::Same,
        None => WorldsRule::Unknown,
    }
}

/// One of the instance's mods that Modrinth knows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceMod {
    pub project_id: String,
    /// The project's title, else the file's name.
    pub title: String,
    pub file_name: String,
    /// Whether it is on (any of its files, for a project with several).
    pub enabled: bool,
}

/// What an instance has that a copy can take.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Contents {
    /// The worlds: folders in `saves/` with a `level.dat`, by name.
    pub worlds: Vec<String>,
    pub resource_packs: Vec<String>,
    pub shader_packs: Vec<String>,
    /// Whether it has an `options.txt`.
    pub options: bool,
    /// The shader pack its shader mod uses, if any.
    pub shader: Option<String>,
    /// Its mods Modrinth knows, one per project.
    pub mods: Vec<SourceMod>,
    /// The names of its other mods, which Modrinth doesn't know: added by hand, or from
    /// CurseForge.
    pub other_mods: Vec<String>,
    /// The names of the mods that couldn't be looked up on Modrinth (it failed, or it
    /// can't be reached): they may be on it.
    pub unchecked: Vec<String>,
    /// Why the lookup failed, if it did.
    pub lookup_error: Option<String>,
}

/// The worlds in the game folder: folders in `saves/` with a `level.dat`, sorted.
pub fn world_folders(dirs: &InstanceDirs) -> Vec<String> {
    let mut worlds: Vec<String> = fs::read_dir(dirs.game.join("saves"))
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .filter(|e| e.path().join("level.dat").is_file())
                .filter_map(|e| e.file_name().to_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    worlds.sort_by_key(|name| name.to_lowercase());
    worlds
}

/// Reads what the instance in `dirs` has. With `api`, mods `content.json` doesn't know
/// yet are looked up on Modrinth by SHA-1 first, so they aren't left behind by mistake.
pub async fn contents(
    api: Option<&Modrinth>,
    dirs: &InstanceDirs,
) -> Result<Contents, ContentError> {
    let (files, provenance, lookup_error) = listing::refresh(api, dirs, ContentKind::Mod).await?;
    let mut mods: Vec<SourceMod> = Vec::new();
    let mut other_mods = Vec::new();
    let mut unchecked = Vec::new();
    for file in files.iter().filter(|f| !f.is_dir) {
        let entry = provenance.current(file);
        let source = entry.and_then(|e| e.modrinth.as_ref());
        match source {
            Some(source) => match mods.iter_mut().find(|m| m.project_id == source.project_id) {
                Some(known) => known.enabled |= file.enabled_by_name(),
                None => mods.push(SourceMod {
                    project_id: source.project_id.clone(),
                    title: if source.title.is_empty() {
                        file.display_name()
                    } else {
                        source.title.clone()
                    },
                    file_name: file.file_name.clone(),
                    enabled: file.enabled_by_name(),
                }),
            },
            None if entry.is_some_and(|e| e.looked_up) => other_mods.push(file.display_name()),
            None => unchecked.push(file.display_name()),
        }
    }
    let names = |kind: ContentKind| -> Result<Vec<String>, ContentError> {
        Ok(super::scan::scan(dirs, kind)?
            .into_iter()
            .map(|f| f.file_name)
            .collect())
    };
    Ok(Contents {
        worlds: world_folders(dirs),
        resource_packs: names(ContentKind::ResourcePack)?,
        shader_packs: names(ContentKind::ShaderPack)?,
        options: dirs.game.join(options::FILE).is_file(),
        shader: listing::shader_state(dirs)?.selection.pack,
        mods,
        other_mods,
        unchecked,
        lookup_error,
    })
}

/// The instance's mods sorted by whether Modrinth has a build of them for a target.
#[derive(Clone, Debug, Default)]
pub struct ResolvedMods {
    /// Each mod with its build for the target.
    pub found: Vec<(SourceMod, RawVersion)>,
    /// The mods with no build for it.
    pub missing: Vec<SourceMod>,
}

impl ResolvedMods {
    /// Sorts `mods` by `build`, each mod's build for the target if it has one.
    pub fn sort(
        mods: &[SourceMod],
        mut build: impl FnMut(&SourceMod) -> Option<RawVersion>,
    ) -> Self {
        let mut resolved = Self::default();
        for source in mods {
            match build(source) {
                Some(version) => resolved.found.push((source.clone(), version)),
                None => resolved.missing.push(source.clone()),
            }
        }
        resolved
    }
}

/// Finds each mod's build for `target`: the newest release for its Minecraft version and
/// loader, else the newest version (as installing picks one). Vanilla runs no mods, so
/// none has a build for it. A project Modrinth no longer has has none.
pub async fn resolve_mods(
    api: &Modrinth,
    mods: &[SourceMod],
    target: &Target,
) -> Result<ResolvedMods, ContentError> {
    if target.loader == Loader::None {
        return Ok(ResolvedMods::sort(mods, |_| None));
    }
    let loaders = target.accepted_loaders(ContentKind::Mod);
    let game_versions = [target.game_version.clone()];
    let mut builds = Vec::with_capacity(mods.len());
    for chunk in mods.chunks(PARALLEL_LOOKUPS) {
        let found =
            future::join_all(chunk.iter().map(|source| {
                build_for(api, &source.project_id, &loaders, &game_versions, target)
            }))
            .await;
        for build in found {
            builds.push(build?);
        }
    }
    let mut builds = builds.into_iter();
    Ok(ResolvedMods::sort(mods, |_| builds.next().flatten()))
}

/// The build of `project` for `target`, if it has one.
async fn build_for(
    api: &Modrinth,
    project: &str,
    loaders: &[String],
    game_versions: &[String],
    target: &Target,
) -> Result<Option<RawVersion>, ContentError> {
    match api.versions(project, loaders, game_versions).await {
        Ok(versions) => Ok(install::pick_version(target, ContentKind::Mod, versions)),
        Err(ModrinthError::NotFound(_)) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// What a copy takes, once chosen.
#[derive(Clone, Debug)]
pub struct CopyPlan {
    pub game_version: String,
    pub loader: Loader,
    /// Whether worlds can go to the new version at all.
    pub worlds_rule: WorldsRule,
    /// The worlds that go.
    pub worlds: Vec<String>,
    pub resource_packs: Vec<String>,
    pub shader_packs: Vec<String>,
    /// Whether `options.txt` goes.
    pub options: bool,
    /// The shader pack to keep in use, if it goes.
    pub shader: Option<String>,
    /// The mods that go, as their build for the new version and loader.
    pub mods: Vec<(SourceMod, RawVersion)>,
    /// Mods Modrinth has no build of for the new version and loader: they stay behind.
    pub no_build: Vec<SourceMod>,
    /// Mods Modrinth doesn't know: they stay behind.
    pub not_on_modrinth: Vec<String>,
    /// Mods that couldn't be looked up on Modrinth: they stay behind.
    pub unchecked: Vec<String>,
}

impl CopyPlan {
    /// The mods that stay behind (unless another mod requires them), by name.
    pub fn left_behind(&self) -> Vec<String> {
        self.no_build
            .iter()
            .map(|m| m.title.clone())
            .chain(self.not_on_modrinth.iter().cloned())
            .chain(self.unchecked.iter().cloned())
            .collect()
    }

    /// Whether the copy changes something the user should agree to first: worlds that
    /// are upgraded, or mods that stay behind.
    pub fn needs_confirmation(&self) -> bool {
        (!self.worlds.is_empty() && self.worlds_rule == WorldsRule::Newer)
            || !self.left_behind().is_empty()
    }
}

/// Decides what goes: `contents` of the instance, by `options`, to `game_version` and
/// `loader`. Worlds go only when `rule` allows them; mods only as `resolved` found them
/// (none without it). With mods left out, or for vanilla (which runs none), none is
/// listed as staying behind either.
pub fn plan(
    contents: &Contents,
    game_version: &str,
    loader: Loader,
    options: CopyAsOptions,
    rule: WorldsRule,
    resolved: Option<&ResolvedMods>,
) -> CopyPlan {
    let packs = options.packs;
    let (mods, no_build, not_on_modrinth, unchecked) = if options.mods && loader != Loader::None {
        let resolved = resolved.cloned().unwrap_or_else(|| ResolvedMods {
            found: Vec::new(),
            missing: contents.mods.clone(),
        });
        (
            resolved.found,
            resolved.missing,
            contents.other_mods.clone(),
            contents.unchecked.clone(),
        )
    } else {
        (Vec::new(), Vec::new(), Vec::new(), Vec::new())
    };
    CopyPlan {
        game_version: game_version.to_string(),
        loader,
        worlds_rule: rule,
        worlds: if options.worlds && rule.allowed() {
            contents.worlds.clone()
        } else {
            Vec::new()
        },
        resource_packs: if packs {
            contents.resource_packs.clone()
        } else {
            Vec::new()
        },
        shader_packs: if packs {
            contents.shader_packs.clone()
        } else {
            Vec::new()
        },
        options: packs && contents.options,
        shader: contents.shader.clone().filter(|_| packs),
        mods,
        no_build,
        not_on_modrinth,
        unchecked,
    }
}

/// The name a copy gets unless one is given: "Survival (1.21.1 Fabric)", or
/// "Survival (1.21.1)" for vanilla.
pub fn default_name(name: &str, game_version: &str, loader: Loader) -> String {
    match loader.kind() {
        Some(kind) => format!("{name} ({game_version} {})", kind.label()),
        None => format!("{name} ({game_version})"),
    }
}

/// The copy's `instance.json`: `source`'s settings that aren't tied to the version, for
/// `game_version` and `loader`. Its own Java stays only on the same Minecraft version
/// (another may need another Java), and automatic setup leaves the game's options alone
/// when `options_copied`.
pub fn copy_record(
    source: &Instance,
    name: String,
    game_version: &str,
    loader: Option<&LoaderChoice>,
    options_copied: bool,
) -> Instance {
    Instance {
        name,
        icon: source.icon.clone(),
        background: source.background.clone(),
        slideshow: source.slideshow,
        play_color: source.play_color,
        accent: source.accent,
        group: source.group.clone(),
        game_version: game_version.to_string(),
        loader: Loader::from_kind(loader.map(|l| l.kind)),
        loader_version: loader.map(|l| l.version.clone()),
        created: instances::now(),
        java_override: source
            .java_override
            .clone()
            .filter(|_| source.game_version == game_version),
        memory: source.memory,
        jvm_args: source.jvm_args.clone(),
        game_args: source.game_args.clone(),
        window: source.window,
        env: source.env.clone(),
        prerelease_updates: source.prerelease_updates,
        auto_config: source.auto_config.clone().map(|mut auto| {
            auto.options = options_copied;
            auto
        }),
        ..Instance::default()
    }
}

/// What making a copy asks for.
#[derive(Clone, Debug)]
pub struct CopyAsRequest {
    /// The copy's name ([`default_name`] when `None`); made unique among the instances.
    pub name: Option<String>,
    pub loader: Option<LoaderChoice>,
    pub plan: CopyPlan,
}

#[derive(Debug, thiserror::Error)]
pub enum CopyAsError {
    #[error(transparent)]
    Content(#[from] ContentError),
    #[error(transparent)]
    Instance(#[from] InstanceError),
    #[error("cancelled")]
    Cancelled,
}

impl CopyAsError {
    fn from_content(err: ContentError) -> Self {
        match err {
            ContentError::Download(DownloadError::Cancelled) => CopyAsError::Cancelled,
            err => CopyAsError::Content(err),
        }
    }
}

/// What a finished copy did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyOutcome {
    pub instance: Instance,
    /// Titles of the mods installed, required mods included.
    pub installed: Vec<String>,
    /// The instance's own mods that aren't in the copy, by name: no build, not on
    /// Modrinth (or not checked), or not installed. One that another mod required, and
    /// so was installed anyway, isn't here.
    pub left_behind: Vec<String>,
    /// Mods the copied ones require that couldn't be installed: a title (or project id)
    /// and why.
    pub missing_dependencies: Vec<(String, String)>,
}

/// Makes the copy of `source` that `request` describes, as a new instance in `store`.
/// `api` is needed when mods go. `files_read` is called once nothing more is read from
/// the original (its worlds, packs and options are copied; the mods come from Modrinth),
/// so the caller can let the original be played again.
#[allow(clippy::too_many_arguments)]
pub async fn execute(
    store: &InstanceStore,
    api: Option<&Modrinth>,
    downloader: &Downloader,
    source: &Instance,
    request: &CopyAsRequest,
    progress: &InstallProgress,
    cancel: &CancelToken,
    files_read: impl FnOnce(),
) -> Result<CopyOutcome, CopyAsError> {
    let staging = store.new_staging()?;
    let result = build(
        store, api, downloader, source, request, progress, cancel, &staging, files_read,
    )
    .await;
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

#[allow(clippy::too_many_arguments)]
async fn build(
    store: &InstanceStore,
    api: Option<&Modrinth>,
    downloader: &Downloader,
    source: &Instance,
    request: &CopyAsRequest,
    progress: &InstallProgress,
    cancel: &CancelToken,
    staging: &Path,
    files_read: impl FnOnce(),
) -> Result<CopyOutcome, CopyAsError> {
    let plan = &request.plan;
    let from = InstanceDirs::new(store.dir(&source.id));
    let to = InstanceDirs::new(staging);
    let check = || {
        if cancel.is_cancelled() {
            Err(CopyAsError::Cancelled)
        } else {
            Ok(())
        }
    };

    if !plan.worlds.is_empty() {
        progress.set_label("Copying the worlds");
        check_room(
            staging,
            plan.worlds
                .iter()
                .map(|world| tree_size(&from.game.join("saves").join(world)))
                .sum(),
        )?;
        for world in &plan.worlds {
            check()?;
            let relative = Path::new("saves").join(world);
            copy_entry(&from.game.join(&relative), &to.game.join(&relative), cancel)?;
        }
        check()?;
        // their data packs' Modrinth records; without them the packs still work
        let _ = crate::datapacks::worlds_copied(&from, &to, &plan.worlds);
    }
    let packs: Vec<(ContentKind, &String)> = plan
        .resource_packs
        .iter()
        .map(|name| (ContentKind::ResourcePack, name))
        .chain(
            plan.shader_packs
                .iter()
                .map(|name| (ContentKind::ShaderPack, name)),
        )
        .collect();
    if !packs.is_empty() || plan.options {
        progress.set_label("Copying the resource packs, shaders and options");
        for (kind, name) in packs {
            check()?;
            copy_entry(
                &from.content_dir(kind).join(name),
                &to.content_dir(kind).join(name),
                cancel,
            )?;
        }
        if plan.options {
            copy_entry(
                &from.game.join(options::FILE),
                &to.game.join(options::FILE),
                cancel,
            )?;
            // A file it can't rewrite still goes as it is: the game reads what it can.
            let _ = options::carry_packs(&to.game, &source.game_version, &plan.game_version);
            carry_sharing(&from.instance, &to.instance)?;
        }
    }
    let servers = from.game.join(SERVERS_FILE);
    if servers.is_file() {
        copy_entry(&servers, &to.game.join(SERVERS_FILE), cancel)?;
    }
    // The instance's own icon and Play backgrounds, which its settings refer to.
    for name in [ICON_FILE, BACKGROUND_FILE, SLIDESHOW_DIR] {
        let path = from.instance.join(name);
        if fs::symlink_metadata(&path).is_ok() {
            copy_entry(&path, &to.instance.join(name), cancel)?;
        }
    }
    // Nothing more is read from the original.
    files_read();
    check()?;

    let target = Target::new(plan.game_version.clone(), plan.loader);
    let mut installed = Vec::new();
    let mut skipped: Vec<(String, String)> = Vec::new();
    let mut not_installed: Vec<String> = Vec::new();
    if !plan.mods.is_empty() {
        let api = api.ok_or_else(|| {
            ContentError::Invalid("Modrinth isn't available to download the mods.".into())
        })?;
        progress.set_label("Installing the mods");
        let roots = plan
            .mods
            .iter()
            .map(|(_, version)| PlannedFile {
                kind: ContentKind::Mod,
                version: version.clone(),
                project: None,
                depth: 0,
            })
            .collect();
        let install_plan = install::plan_all(api, &to, &target, roots, true)
            .await
            .map_err(CopyAsError::from_content)?;
        check()?;
        // What the plan couldn't find or fit. Installing skips more only for a file
        // already there, which in this new folder another mod of the copy put there.
        skipped = install_plan
            .skipped
            .iter()
            .map(|s| (s.project.clone(), s.reason.clone()))
            .collect();
        let planned = install_plan.skipped.len();
        let outcome = install::execute(
            api,
            downloader,
            &to,
            &target,
            install_plan,
            progress,
            cancel,
        )
        .await
        .map_err(CopyAsError::from_content)?;
        installed = outcome
            .installed
            .into_iter()
            .map(|(title, _)| title)
            .chain(outcome.skipped.into_iter().skip(planned).map(|s| s.project))
            .collect();
        // What was off stays off.
        let provenance = Provenance::load(&to.instance);
        let mut by_project = provenance.by_project(ContentKind::Mod);
        for (source_mod, _) in &plan.mods {
            match by_project.remove(&source_mod.project_id) {
                Some(names) if !source_mod.enabled => {
                    for name in names {
                        files::set_mod_enabled(&to, &name, false)?;
                    }
                }
                Some(_) => {}
                None => not_installed.push(source_mod.title.clone()),
            }
        }
    }
    let (left_behind, missing_dependencies) =
        sort_left_behind(plan, &installed, not_installed, skipped);
    if let Some(shader) = &plan.shader
        && plan.shader_packs.contains(shader)
        && let Some(loader) = listing::shader_state(&to)?.loader
    {
        files::select_shader(&to, loader, Some(shader))?;
    }
    check()?;

    let name = request
        .name
        .clone()
        .unwrap_or_else(|| default_name(&source.name, &plan.game_version, plan.loader));
    let record = copy_record(
        source,
        name,
        &plan.game_version,
        request.loader.as_ref(),
        plan.options,
    );
    let instance = store.commit_staged(staging, record)?;
    Ok(CopyOutcome {
        instance,
        installed,
        left_behind,
        missing_dependencies,
    })
}

/// The instance's own mods that aren't in the copy, and the required mods that couldn't
/// be installed. A mod the plan left behind that another mod pulled in anyway, by the
/// same name, is in the copy; a mod that couldn't be installed is one of the instance's
/// own when it has its name, and a missing dependency otherwise.
fn sort_left_behind(
    plan: &CopyPlan,
    installed: &[String],
    not_installed: Vec<String>,
    skipped: Vec<(String, String)>,
) -> (Vec<String>, Vec<(String, String)>) {
    let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
    let is_installed = |name: &str| installed.iter().any(|i| same(i, name));
    let own = |name: &str| plan.mods.iter().any(|(m, _)| same(&m.title, name));
    let mut left_behind: Vec<String> = Vec::new();
    for name in plan.left_behind().into_iter().chain(not_installed) {
        if !is_installed(&name) && !left_behind.iter().any(|l| same(l, &name)) {
            left_behind.push(name);
        }
    }
    let mut missing = Vec::new();
    for (name, reason) in skipped {
        if is_installed(&name) || left_behind.iter().any(|l| same(l, &name)) {
            continue;
        }
        if own(&name) {
            left_behind.push(name);
        } else {
            missing.push((name, reason));
        }
    }
    (left_behind, missing)
}

/// Copies the file or folder `from` to `to` (folders whole, skipping symbolic links).
/// One that is gone (deleted since the instance was read) is skipped. Once `cancel` is
/// cancelled, a folder's remaining files are skipped, and the caller then stops.
fn copy_entry(from: &Path, to: &Path, cancel: &CancelToken) -> Result<(), ContentError> {
    let metadata = match fs::symlink_metadata(from) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(io_error(from)(err)),
    };
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(io_error(parent))?;
    }
    if metadata.is_dir() {
        instances::copy_tree(from, to, &|_| cancel.is_cancelled())
            .map_err(|err| ContentError::Invalid(err.to_string()))
    } else if metadata.is_file() {
        fs::copy(from, to).map(|_| ()).map_err(io_error(to))
    } else {
        Ok(())
    }
}

/// Carries the instance's shared game settings choice (on or off, and the settings it
/// keeps its own) from the instance folder `from` to `to`, with its `options.txt`.
/// Without it the copy would count as using them, and its first launch would write the
/// shared settings over the options copied. What a game left to read back, and changes
/// waiting to be kept, stay with the original.
fn carry_sharing(from: &Path, to: &Path) -> Result<(), ContentError> {
    if !shared_options::instance_path(from).is_file() {
        return Ok(());
    }
    // A file it can't read leaves the copy with the default (sharing on).
    let Ok(sharing) = shared_options::load_instance(from) else {
        return Ok(());
    };
    let carried = shared_options::InstanceSharing {
        enabled: sharing.enabled,
        overrides: sharing.overrides,
        ..shared_options::InstanceSharing::default()
    };
    let path = shared_options::instance_path(to);
    storage::save(&path, &carried).map_err(|err| ContentError::Invalid(err.to_string()))
}

/// The bytes of the files in the folder (or file) at `path`, skipping symbolic links.
fn tree_size(path: &Path) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if metadata.is_file() {
        return metadata.len();
    }
    if !metadata.is_dir() {
        return 0;
    }
    fs::read_dir(path)
        .map(|entries| entries.flatten().map(|e| tree_size(&e.path())).sum())
        .unwrap_or(0)
}

/// Refuses to copy `bytes` into `dir` when its disk has less room free.
fn check_room(dir: &Path, bytes: u64) -> Result<(), ContentError> {
    match crate::snapshots::free_space(dir) {
        Some(free) if bytes > free => Err(ContentError::Invalid(format!(
            "The worlds need {} MB and the disk has {} MB free.",
            bytes.div_ceil(1_000_000),
            free / 1_000_000
        ))),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The names of the projects in `mods`.
    fn titles(mods: &[SourceMod]) -> BTreeSet<&str> {
        mods.iter().map(|m| m.title.as_str()).collect()
    }

    fn source_mod(project: &str, enabled: bool) -> SourceMod {
        SourceMod {
            project_id: project.into(),
            title: format!("{project} title"),
            file_name: format!("{project}.jar"),
            enabled,
        }
    }

    fn version(project: &str) -> RawVersion {
        serde_json::from_value(serde_json::json!({
            "id": format!("{project}-v"), "project_id": project, "name": project,
            "version_number": "1.0", "version_type": "release", "loaders": ["fabric"],
            "game_versions": ["1.21.1"],
        }))
        .unwrap()
    }

    fn contents() -> Contents {
        Contents {
            worlds: vec!["New World".into(), "Skyblock".into()],
            resource_packs: vec!["Faithful.zip".into()],
            shader_packs: vec!["BSL.zip".into(), "Complementary".into()],
            options: true,
            shader: Some("BSL.zip".into()),
            mods: vec![source_mod("sodium", true), source_mod("iris", false)],
            other_mods: vec!["Homemade".into()],
            unchecked: Vec::new(),
            lookup_error: None,
        }
    }

    #[test]
    fn worlds_go_only_to_the_same_or_a_newer_version() {
        let manifest = [
            "26.1",
            "26.1-snapshot-3",
            "1.21.4",
            "1.21.1",
            "1.20.1",
            "b1.7.3",
        ];
        assert_eq!(worlds_rule("1.21.1", "1.21.1", &manifest), WorldsRule::Same);
        assert_eq!(
            worlds_rule("1.20.1", "1.21.1", &manifest),
            WorldsRule::Newer
        );
        assert_eq!(
            worlds_rule("1.21.4", "1.21.1", &manifest),
            WorldsRule::Older
        );
        // the manifest places snapshots and old versions
        assert_eq!(
            worlds_rule("1.21.4", "26.1-snapshot-3", &manifest),
            WorldsRule::Newer
        );
        assert_eq!(
            worlds_rule("26.1", "26.1-snapshot-3", &manifest),
            WorldsRule::Older
        );
        assert_eq!(
            worlds_rule("b1.7.3", "1.20.1", &manifest),
            WorldsRule::Newer
        );
        // without the manifest, releases by number (the year-numbered ones are newer)
        assert_eq!(worlds_rule("1.20.1", "1.21", &[]), WorldsRule::Newer);
        assert_eq!(worlds_rule("26.1", "1.21.11", &[]), WorldsRule::Older);
        assert_eq!(worlds_rule("1.21.11", "1.21.9", &[]), WorldsRule::Older);
        // a snapshot it doesn't know can't be placed
        assert_eq!(worlds_rule("1.21.1", "24w14a", &[]), WorldsRule::Unknown);
        assert!(WorldsRule::Same.allowed() && WorldsRule::Newer.allowed());
        assert!(!WorldsRule::Older.allowed() && !WorldsRule::Unknown.allowed());
    }

    #[test]
    fn the_plan_takes_what_was_chosen() {
        let contents = contents();
        let resolved = ResolvedMods::sort(&contents.mods, |m| {
            (m.project_id == "sodium").then(|| version("sodium"))
        });
        let all = CopyAsOptions {
            worlds: true,
            packs: true,
            mods: true,
        };
        let plan = plan(
            &contents,
            "1.21.1",
            Loader::Fabric,
            all,
            WorldsRule::Newer,
            Some(&resolved),
        );
        assert_eq!(plan.worlds, ["New World", "Skyblock"]);
        assert_eq!(plan.resource_packs, ["Faithful.zip"]);
        assert_eq!(plan.shader_packs, ["BSL.zip", "Complementary"]);
        assert!(plan.options);
        assert_eq!(plan.shader.as_deref(), Some("BSL.zip"));
        assert_eq!(plan.mods.len(), 1);
        assert_eq!(plan.mods[0].1.project_id, "sodium");
        assert_eq!(titles(&plan.no_build), BTreeSet::from(["iris title"]));
        assert_eq!(plan.not_on_modrinth, ["Homemade"]);
        assert_eq!(plan.left_behind(), ["iris title", "Homemade"]);
        assert!(plan.needs_confirmation());
    }

    #[test]
    fn worlds_stay_on_an_older_version_and_parts_left_out_stay() {
        let contents = contents();
        let with_worlds = CopyAsOptions {
            worlds: true,
            ..CopyAsOptions::default()
        };
        for rule in [WorldsRule::Older, WorldsRule::Unknown] {
            let plan = plan(&contents, "1.20.1", Loader::Fabric, with_worlds, rule, None);
            assert!(plan.worlds.is_empty(), "{rule:?}");
        }
        // the defaults: no worlds, the packs and options, the mods
        let defaults = plan(
            &contents,
            "1.21.1",
            Loader::Fabric,
            CopyAsOptions::default(),
            WorldsRule::Same,
            None,
        );
        assert!(defaults.worlds.is_empty());
        assert_eq!(defaults.resource_packs.len(), 1);
        // not resolved (offline): every Modrinth mod counts as having no build
        assert!(defaults.mods.is_empty());
        assert_eq!(defaults.no_build.len(), 2);
        let nothing = CopyAsOptions {
            worlds: false,
            packs: false,
            mods: false,
        };
        let bare = plan(
            &contents,
            "1.21.1",
            Loader::Fabric,
            nothing,
            WorldsRule::Same,
            None,
        );
        assert!(bare.resource_packs.is_empty() && bare.shader_packs.is_empty());
        assert!(!bare.options && bare.shader.is_none());
        assert!(bare.mods.is_empty() && bare.left_behind().is_empty());
        assert!(!bare.needs_confirmation());
        // the same version with everything found asks nothing
        let found = ResolvedMods::sort(&contents.mods, |m| Some(version(&m.project_id)));
        let mut only_modrinth = contents.clone();
        only_modrinth.other_mods.clear();
        let same = plan(
            &only_modrinth,
            "1.21.1",
            Loader::Fabric,
            CopyAsOptions {
                worlds: true,
                ..CopyAsOptions::default()
            },
            WorldsRule::Same,
            Some(&found),
        );
        assert_eq!(same.worlds.len(), 2);
        assert_eq!(same.mods.len(), 2);
        assert!(!same.needs_confirmation());
    }

    #[test]
    fn mods_that_couldnt_be_checked_stay_behind_and_are_asked_about() {
        let mut contents = contents();
        contents.other_mods.clear();
        contents.unchecked = vec!["Mystery".into()];
        let found = ResolvedMods::sort(&contents.mods, |m| Some(version(&m.project_id)));
        let plan = plan(
            &contents,
            "1.21.1",
            Loader::Fabric,
            CopyAsOptions::default(),
            WorldsRule::Same,
            Some(&found),
        );
        assert_eq!(plan.unchecked, ["Mystery"]);
        assert_eq!(plan.left_behind(), ["Mystery"]);
        assert!(plan.needs_confirmation());
    }

    #[test]
    fn a_mod_another_one_pulled_in_isnt_left_behind() {
        let mut contents = contents();
        contents.other_mods = vec!["Fabric API".into(), "Homemade".into()];
        let resolved = ResolvedMods::sort(&contents.mods, |m| {
            (m.project_id == "sodium").then(|| version("sodium"))
        });
        let plan = plan(
            &contents,
            "1.21.1",
            Loader::Fabric,
            CopyAsOptions::default(),
            WorldsRule::Newer,
            Some(&resolved),
        );
        // Sodium was installed and required Fabric API (from CurseForge in the
        // original), which came along; one dependency couldn't be found
        let installed = vec!["sodium title".to_string(), "Fabric API".to_string()];
        let skipped = vec![
            ("Lib".to_string(), "Not found on Modrinth".to_string()),
            ("fabric api".to_string(), "Not found".to_string()),
        ];
        let (behind, missing) = sort_left_behind(&plan, &installed, Vec::new(), skipped);
        assert_eq!(behind, ["iris title", "Homemade"]);
        assert_eq!(
            missing,
            [("Lib".to_string(), "Not found on Modrinth".to_string())]
        );
        // one of its own mods that couldn't be installed stays behind, once
        let (behind, missing) = sort_left_behind(
            &plan,
            &[],
            vec!["sodium title".into()],
            vec![("sodium title".into(), "Incompatible".into())],
        );
        assert_eq!(
            behind,
            ["iris title", "Fabric API", "Homemade", "sodium title"]
        );
        assert!(missing.is_empty());
    }

    #[test]
    fn mods_are_sorted_by_whether_they_have_a_build() {
        let mods = [
            source_mod("a", true),
            source_mod("b", true),
            source_mod("c", false),
        ];
        let resolved = ResolvedMods::sort(&mods, |m| {
            (m.project_id != "b").then(|| version(&m.project_id))
        });
        let found: Vec<&str> = resolved
            .found
            .iter()
            .map(|(m, _)| m.project_id.as_str())
            .collect();
        assert_eq!(found, ["a", "c"]);
        assert_eq!(resolved.missing, [source_mod("b", true)]);
    }

    #[test]
    fn the_copy_keeps_the_settings_that_arent_tied_to_the_version() {
        use crate::instances::{AutoConfig, Memory};
        let source = Instance {
            id: "survival".into(),
            name: "Survival".into(),
            icon: Some("custom".into()),
            group: Some("Friends".into()),
            imported_from: Some("Prism Launcher".into()),
            game_version: "1.20.1".into(),
            loader: Loader::Forge,
            loader_version: Some("47.4.0".into()),
            created: 1,
            last_played: Some(5),
            play_time: 600,
            java_override: Some("/java17".into()),
            memory: Some(Memory {
                min: 1024,
                max: 6144,
            }),
            jvm_args: Some("-XX:+UseG1GC".into()),
            recommended_memory: Some(8192),
            auto_config: Some(AutoConfig {
                options: false,
                ..AutoConfig::default()
            }),
            ..Instance::default()
        };
        let fabric = LoaderChoice {
            kind: crate::game::loaders::LoaderKind::Fabric,
            version: "0.16.14".into(),
        };
        let copy = copy_record(
            &source,
            "Survival (1.21.1 Fabric)".into(),
            "1.21.1",
            Some(&fabric),
            true,
        );
        assert_eq!(copy.name, "Survival (1.21.1 Fabric)");
        assert_eq!(
            (copy.game_version.as_str(), copy.loader),
            ("1.21.1", Loader::Fabric)
        );
        assert_eq!(copy.loader_version.as_deref(), Some("0.16.14"));
        assert_eq!(copy.icon.as_deref(), Some("custom"));
        assert_eq!(copy.group.as_deref(), Some("Friends"));
        assert_eq!(copy.memory, source.memory);
        assert_eq!(copy.jvm_args, source.jvm_args);
        // tied to the old version, the pack or the old instance
        assert_eq!(copy.java_override, None);
        assert_eq!(copy.recommended_memory, None);
        assert_eq!(copy.imported_from, None);
        assert_eq!((copy.last_played, copy.play_time), (None, 0));
        assert!(copy.auto_config.unwrap().options);
        // on the same version, its own Java stays
        let same = copy_record(&source, "S".into(), "1.20.1", None, false);
        assert_eq!(same.java_override, source.java_override);
        assert_eq!((same.loader, same.loader_version), (Loader::None, None));

        assert_eq!(
            default_name("Survival", "1.21.1", Loader::Fabric),
            "Survival (1.21.1 Fabric)"
        );
        assert_eq!(
            default_name("Survival", "1.21.1", Loader::None),
            "Survival (1.21.1)"
        );
    }
}
