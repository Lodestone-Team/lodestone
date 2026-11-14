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
//!   version names them), with the shader pack in use;
//! - mods Modrinth knows (from `content.json`, after a lookup by SHA-1), as each
//!   project's build for the new version and loader ([`resolve_mods`]), installed fresh
//!   through the usual Modrinth path (hashes checked) with any required mods they need
//!   now. A mod that was off stays off. Mods with no such build, and mods Modrinth
//!   doesn't know, stay behind.
//!
//! The settings that aren't tied to the version (memory, arguments, window, group,
//! icon, background) come along; play time, the modpack and a linked server don't. The
//! copy is built in a hidden folder and renamed into place, so a failed or cancelled
//! copy leaves nothing behind. The loader installs the next time the copy is played.

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
};

/// How many projects are looked up on Modrinth at once.
const PARALLEL_LOOKUPS: usize = 6;

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
    /// The names of its other mods: added by hand, from CurseForge, or not checked yet.
    pub other_mods: Vec<String>,
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
    let (files, provenance, _) = listing::refresh(api, dirs, ContentKind::Mod).await?;
    let mut mods: Vec<SourceMod> = Vec::new();
    let mut other_mods = Vec::new();
    for file in files.iter().filter(|f| !f.is_dir) {
        let source = provenance.current(file).and_then(|e| e.modrinth.as_ref());
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
            None => other_mods.push(file.display_name()),
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
}

impl CopyPlan {
    /// The mods that stay behind, by name.
    pub fn left_behind(&self) -> Vec<String> {
        self.no_build
            .iter()
            .map(|m| m.title.clone())
            .chain(self.not_on_modrinth.iter().cloned())
            .collect()
    }

    /// Whether the copy changes something the user should agree to first: worlds that
    /// are upgraded, or mods that stay behind.
    pub fn needs_confirmation(&self) -> bool {
        (!self.worlds.is_empty() && self.worlds_rule == WorldsRule::Newer)
            || !self.no_build.is_empty()
            || !self.not_on_modrinth.is_empty()
    }
}

/// Decides what goes: `contents` of the instance, by `options`, to `game_version` and
/// `loader`. Worlds go only when `rule` allows them; mods only as `resolved` found them
/// (none without it). With mods left out, none is listed as staying behind either.
pub fn plan(
    contents: &Contents,
    game_version: &str,
    loader: Loader,
    options: CopyAsOptions,
    rule: WorldsRule,
    resolved: Option<&ResolvedMods>,
) -> CopyPlan {
    let packs = options.packs;
    let (mods, no_build, not_on_modrinth) = if options.mods {
        let resolved = resolved.cloned().unwrap_or_else(|| ResolvedMods {
            found: Vec::new(),
            missing: contents.mods.clone(),
        });
        (
            resolved.found,
            resolved.missing,
            contents.other_mods.clone(),
        )
    } else {
        (Vec::new(), Vec::new(), Vec::new())
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
    /// What couldn't be installed: a title (or project id) and why.
    pub skipped: Vec<(String, String)>,
}

/// Makes the copy of `source` that `request` describes, as a new instance in `store`.
/// `api` is needed when mods go.
#[allow(clippy::too_many_arguments)]
pub async fn execute(
    store: &InstanceStore,
    api: Option<&Modrinth>,
    downloader: &Downloader,
    source: &Instance,
    request: &CopyAsRequest,
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<CopyOutcome, CopyAsError> {
    let staging = store.new_staging()?;
    let result = build(
        store, api, downloader, source, request, progress, cancel, &staging,
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
        for world in &plan.worlds {
            check()?;
            let relative = Path::new("saves").join(world);
            copy_entry(&from.game.join(&relative), &to.game.join(&relative))?;
        }
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
            )?;
        }
        if plan.options {
            copy_entry(&from.game.join(options::FILE), &to.game.join(options::FILE))?;
            options::carry_packs(&to.game, &source.game_version, &plan.game_version)
                .map_err(|err| ContentError::Invalid(err.to_string()))?;
        }
    }
    // The instance's own icon and Play backgrounds, which its settings refer to.
    for name in [ICON_FILE, BACKGROUND_FILE, SLIDESHOW_DIR] {
        let path = from.instance.join(name);
        if fs::symlink_metadata(&path).is_ok() {
            copy_entry(&path, &to.instance.join(name))?;
        }
    }
    check()?;

    let target = Target::new(plan.game_version.clone(), plan.loader);
    let mut installed = Vec::new();
    let mut skipped = Vec::new();
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
            .collect();
        skipped = outcome
            .skipped
            .into_iter()
            .map(|s| (s.project, s.reason))
            .collect();
        // What was off stays off.
        let provenance = Provenance::load(&to.instance);
        let mut by_project = provenance.by_project(ContentKind::Mod);
        for (source_mod, _) in plan.mods.iter().filter(|(m, _)| !m.enabled) {
            for name in by_project
                .remove(&source_mod.project_id)
                .unwrap_or_default()
            {
                files::set_mod_enabled(&to, &name, false)?;
            }
        }
    }
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
        skipped,
    })
}

/// Copies the file or folder `from` to `to` (folders whole, skipping symbolic links).
fn copy_entry(from: &Path, to: &Path) -> Result<(), ContentError> {
    let metadata = fs::symlink_metadata(from).map_err(io_error(from))?;
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(io_error(parent))?;
    }
    if metadata.is_dir() {
        instances::copy_tree(from, to, &|_| false)
            .map_err(|err| ContentError::Invalid(err.to_string()))
    } else if metadata.is_file() {
        fs::copy(from, to).map(|_| ()).map_err(io_error(to))
    } else {
        Ok(())
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
