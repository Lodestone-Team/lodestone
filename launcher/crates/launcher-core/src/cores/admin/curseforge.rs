//! CurseForge for a server's mods and plugins, as Lodestone Host offers it.
//!
//! The core has no CurseForge client and no key (`docs/instance-images.md`, "M6a
//! decisions"): the key Lodestone was issued is compiled into the apps people run, not
//! into a server anyone on it can ask. So the Host, which has the key as the Launcher
//! does, asks CurseForge itself, and the core only does what it does for any jar:
//!
//! - **Browsing**: searches, projects and files come from [`CurseForge`] and are turned
//!   into the Modrinth-shaped values the Mods tab already shows ([`search`],
//!   [`project`], [`version`]).
//! - **What is installed**: the core reports each jar's CurseForge fingerprint
//!   ([`InstalledMod::curseforge_fingerprint`]); [`identify`] asks CurseForge which
//!   files those are, and [`find_updates`] which of them have a newer file.
//! - **Installing**: [`plan_install`] works out the files (the picked one and its
//!   required dependencies, by the Launcher's rules), and [`install`] downloads them
//!   from CurseForge's CDN here, checked by SHA-1, and uploads them to the core as jars
//!   (`POST /instance/{uuid}/mods/upload`), then deletes the older files they replace.
//!   An upload of a jar Modrinth doesn't know needs the core's permission to write any
//!   file, which the owner has.
//!
//! A file whose author keeps it on CurseForge's website is never fetched from
//! CurseForge, and no CDN address is ever made up for it. If Modrinth has the very same
//! file (by SHA-1), the core installs it from Modrinth as it installs any Modrinth
//! version. Otherwise it is left in [`ServerInstallOutcome::blocked`]: the user
//! downloads it in their browser, [`find_downloaded`] spots it in their Downloads
//! folder, and [`install_downloaded`] checks it and uploads it. Nothing CurseForge
//! returns is kept.

use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    path::Path,
};

use futures::StreamExt as _;

use super::{
    AdminClient,
    mods::{
        InstalledMod, InstalledModKind, InstalledModLoader, ModInstallRequest, ModInstallResult,
        ModInstallSkipped, ModrinthDependency, ModrinthGalleryImage, ModrinthProject,
        ModrinthSearchHit, ModrinthSearchIndex, ModrinthSearchResponse, ModrinthVersion,
        ModrinthVersionFile, ModrinthVersionType,
    },
};
use crate::{
    content::{
        ContentKind, CurseForge, CurseForgeError, Modrinth, Target,
        curseforge::{
            self, CLASS_BUKKIT_PLUGINS, CLASS_MODPACKS, CLASS_MODS, CfFile, CfMod, CfSearch, CfSort,
        },
        install::MAX_DEPENDENCY_DEPTH,
        listing::curseforge_type_allowed,
    },
    cores::CoreError,
    game::download::{CancelToken, DownloadError, DownloadItem, Downloader, Progress},
    instances::Loader,
};

/// A jar may be at most this large, as the core's own limit for a mod.
pub const MAX_MOD_BYTES: u64 = 512 * 1024 * 1024;
/// How many projects' files are asked for at a time when looking for updates.
const UPDATE_LOOKUPS_AT_ONCE: usize = 4;

#[derive(Debug, thiserror::Error)]
pub enum ServerCfError {
    #[error(transparent)]
    CurseForge(#[from] CurseForgeError),
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("{0}")]
    Download(String),
    /// The file doesn't fit the server, or can't be installed as it is.
    #[error("{0}")]
    Invalid(String),
}

impl From<DownloadError> for ServerCfError {
    fn from(err: DownloadError) -> Self {
        ServerCfError::Download(err.to_string())
    }
}

/// What a server takes: the Mods tab's target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerTarget {
    pub kind: InstalledModKind,
    pub loader: InstalledModLoader,
    pub game_version: String,
}

impl ServerTarget {
    /// The CurseForge class its mods or plugins are in.
    pub fn class_id(&self) -> u32 {
        match self.kind {
            InstalledModKind::Mod => CLASS_MODS,
            InstalledModKind::Plugin => CLASS_BUKKIT_PLUGINS,
        }
    }

    /// The loader its mods are tagged with on CurseForge. Plugins have none: every
    /// Bukkit plugin is for Paper and Spigot alike.
    pub fn loader(&self) -> Loader {
        match self.loader {
            InstalledModLoader::Fabric => Loader::Fabric,
            InstalledModLoader::Forge => Loader::Forge,
            InstalledModLoader::NeoForge => Loader::NeoForge,
            InstalledModLoader::Paper
            | InstalledModLoader::Spigot
            | InstalledModLoader::Unknown => Loader::None,
        }
    }

    /// The loaders a search for mods is limited to (NeoForge on 1.20.1 also takes Forge
    /// mods); none for plugins.
    pub fn accepted_loaders(&self) -> Vec<Loader> {
        match self.kind {
            InstalledModKind::Mod => {
                curseforge::accepted_loaders(self.loader(), &self.game_version)
            }
            InstalledModKind::Plugin => Vec::new(),
        }
    }

    /// The one loader CurseForge can filter a project's files by, if there is one.
    fn loader_filter(&self) -> Option<Loader> {
        match self.accepted_loaders().as_slice() {
            [only] => Some(*only),
            _ => None,
        }
    }

    /// Why `file` doesn't run on the server, or `None` if it does: the Launcher's rule
    /// for mods ([`Target::curseforge_incompatibility`]), the game version alone for
    /// plugins.
    pub fn incompatibility(&self, file: &CfFile) -> Option<String> {
        match self.kind {
            InstalledModKind::Mod => Target::new(self.game_version.clone(), self.loader())
                .curseforge_incompatibility(ContentKind::Mod, file),
            // a resource pack is checked by its game version only, as a plugin is
            InstalledModKind::Plugin => Target::new(self.game_version.clone(), Loader::None)
                .curseforge_incompatibility(ContentKind::ResourcePack, file),
        }
    }

    pub fn fits(&self, file: &CfFile) -> bool {
        self.incompatibility(file).is_none()
    }
}

// ---------------------------------------------------------------------------
// CurseForge's values as the Mods tab's

/// The Modrinth project type a CurseForge class is shown as.
fn project_type(class_id: Option<u32>) -> &'static str {
    match class_id {
        Some(CLASS_BUKKIT_PLUGINS) => "plugin",
        Some(CLASS_MODPACKS) => "modpack",
        Some(curseforge::CLASS_RESOURCE_PACKS) => "resourcepack",
        Some(curseforge::CLASS_SHADERS) => "shader",
        Some(curseforge::CLASS_DATA_PACKS) => "datapack",
        _ => "mod",
    }
}

/// How CurseForge orders what the Mods tab's sort asks for. CurseForge has no
/// relevance or follower order: its popularity stands in for both.
pub fn sort_of(index: ModrinthSearchIndex) -> CfSort {
    match index {
        ModrinthSearchIndex::Relevance | ModrinthSearchIndex::Follows => CfSort::Popularity,
        ModrinthSearchIndex::Downloads => CfSort::TotalDownloads,
        ModrinthSearchIndex::Newest | ModrinthSearchIndex::Updated => CfSort::LastUpdated,
    }
}

/// The orders a CurseForge search offers, as the Mods tab names them.
pub const SORTS: [ModrinthSearchIndex; 3] = [
    ModrinthSearchIndex::Relevance,
    ModrinthSearchIndex::Downloads,
    ModrinthSearchIndex::Updated,
];

/// A CurseForge project as a search result. CurseForge doesn't say whether players
/// need a mod, so its sides are unknown.
pub fn search_hit(project: &CfMod) -> ModrinthSearchHit {
    ModrinthSearchHit {
        project_id: project.id.to_string(),
        slug: project.slug.clone(),
        project_type: project_type(project.class_id).to_string(),
        title: project.name.clone(),
        description: project.summary.clone(),
        author: project
            .authors
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_default(),
        icon_url: project.icon_url().map(str::to_string),
        downloads: project.download_count.max(0.0) as u64,
        follows: 0,
        categories: Vec::new(),
        versions: Vec::new(),
        date_modified: project.date_modified.clone(),
        client_side: "unknown".to_string(),
        server_side: "unknown".to_string(),
    }
}

/// A CurseForge project as the project view shows one, with its description (HTML,
/// which [`crate::markdown::parse`] reads as it reads Markdown's own HTML).
pub fn project(project: &CfMod, description: String) -> ModrinthProject {
    ModrinthProject {
        id: project.id.to_string(),
        slug: project.slug.clone(),
        project_type: project_type(project.class_id).to_string(),
        title: project.name.clone(),
        description: project.summary.clone(),
        body: description,
        icon_url: project.icon_url().map(str::to_string),
        categories: Vec::new(),
        loaders: Vec::new(),
        game_versions: Vec::new(),
        downloads: project.download_count.max(0.0) as u64,
        followers: 0,
        client_side: "unknown".to_string(),
        server_side: "unknown".to_string(),
        license_id: None,
        source_url: None,
        issues_url: None,
        wiki_url: None,
        discord_url: None,
        gallery: project
            .screenshots
            .iter()
            .filter_map(|image| {
                Some(ModrinthGalleryImage {
                    url: image.url.clone().or_else(|| image.thumbnail_url.clone())?,
                    title: image.title.clone().filter(|t| !t.trim().is_empty()),
                    description: None,
                    featured: false,
                })
            })
            .collect(),
        published: String::new(),
        updated: project.date_modified.clone(),
    }
}

/// The name a file is listed by: its display name without `.jar` (CurseForge files have
/// no version number of their own).
pub fn file_label(file: &CfFile) -> String {
    let name = file.display_name_or_file();
    let cut = name.len().saturating_sub(4);
    if name.is_char_boundary(cut) && name[cut..].eq_ignore_ascii_case(".jar") && cut > 0 {
        name[..cut].to_string()
    } else {
        name.to_string()
    }
}

fn loader_id(loader: Loader) -> Option<&'static str> {
    match loader {
        Loader::None => None,
        Loader::Fabric => Some("fabric"),
        Loader::Quilt => Some("quilt"),
        Loader::Forge => Some("forge"),
        Loader::NeoForge => Some("neoforge"),
    }
}

/// A CurseForge file as a version of its project: its id is the file's.
pub fn version(file: &CfFile) -> ModrinthVersion {
    let tags = file.tags();
    ModrinthVersion {
        id: file.id.to_string(),
        project_id: file.mod_id.to_string(),
        name: file.display_name_or_file().to_string(),
        version_number: file_label(file),
        version_type: match file.release_type {
            2 => ModrinthVersionType::Beta,
            3 => ModrinthVersionType::Alpha,
            _ => ModrinthVersionType::Release,
        },
        game_versions: tags.game_versions,
        loaders: tags
            .loaders
            .iter()
            .filter_map(|l| loader_id(*l))
            .map(str::to_string)
            .collect(),
        date_published: file.file_date.clone(),
        downloads: 0,
        files: vec![ModrinthVersionFile {
            filename: file.file_name.clone(),
            size: file.file_length,
            primary: true,
            sha512: String::new(),
            url: file.download_url.clone().unwrap_or_default(),
        }],
        dependencies: file
            .dependencies
            .iter()
            .map(|d| ModrinthDependency {
                version_id: None,
                project_id: Some(d.mod_id.to_string()),
                file_name: None,
                dependency_type: match d.relation_type {
                    3 => "required",
                    5 => "incompatible",
                    1 | 6 => "embedded",
                    _ => "optional",
                }
                .to_string(),
            })
            .collect(),
    }
}

/// What a search is limited to: a game version, and any of some loaders.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchFilter {
    pub game_version: Option<String>,
    pub loaders: Vec<Loader>,
}

impl SearchFilter {
    /// Projects with a file for the server's game version (and loader, for mods).
    pub fn for_server(target: &ServerTarget) -> Self {
        Self {
            game_version: Some(target.game_version.clone()),
            loaders: target.accepted_loaders(),
        }
    }
}

/// The loader a Modrinth loader id names ("fabric"), for a search's filter.
pub fn loader_named(id: &str) -> Option<Loader> {
    match id.to_ascii_lowercase().as_str() {
        "fabric" => Some(Loader::Fabric),
        "quilt" => Some(Loader::Quilt),
        "forge" => Some(Loader::Forge),
        "neoforge" => Some(Loader::NeoForge),
        _ => None,
    }
}

/// A page of CurseForge projects of `class_id`, as a Modrinth search's page.
pub async fn search(
    cf: &CurseForge,
    class_id: u32,
    query: &str,
    filter: &SearchFilter,
    index: ModrinthSearchIndex,
    offset: u32,
    limit: u32,
) -> Result<ModrinthSearchResponse, CurseForgeError> {
    let mut search = CfSearch::new(class_id, query);
    search.sort = sort_of(index);
    search.offset = offset as usize;
    search.limit = limit as usize;
    search.game_version = filter.game_version.clone();
    search.loaders = filter.loaders.clone();
    let results = cf.search(&search).await?;
    Ok(ModrinthSearchResponse {
        hits: results.hits.iter().map(search_hit).collect(),
        offset: results.offset as u32,
        limit,
        total_hits: results.total.min(curseforge::MAX_SEARCH_RESULTS) as u32,
    })
}

/// Whether typed text is a curseforge.com link to open instead of a search.
pub fn looks_like_curseforge_link(text: &str) -> bool {
    text.to_ascii_lowercase().contains("curseforge.com/")
}

/// The files of `project` to list: for the server's game version and loader, or all of
/// them. Files of another loader that CurseForge returns anyway (it filters by one
/// loader at most) are left to the caller, which says which ones don't fit.
pub async fn files(
    cf: &CurseForge,
    project: u64,
    target: &ServerTarget,
    all: bool,
) -> Result<Vec<CfFile>, CurseForgeError> {
    if all {
        cf.files(project, None, None).await
    } else {
        let mut files = cf
            .files(project, Some(&target.game_version), target.loader_filter())
            .await?;
        files.retain(|file| target.fits(file));
        Ok(files)
    }
}

// ---------------------------------------------------------------------------
// What is installed

/// The CurseForge file an installed jar is, with its project.
#[derive(Clone, Debug, PartialEq)]
pub struct CfInstalled {
    pub project_id: u64,
    pub file_id: u64,
    pub title: String,
    /// The project's page on curseforge.com.
    pub page_url: String,
    pub icon_url: Option<String>,
    /// The file's label ([`file_label`]), shown where Modrinth has a version number.
    pub version: String,
    /// "release", "beta" or "alpha".
    pub version_type: &'static str,
    pub file_date: String,
    /// A newer file for the server, once [`find_updates`] looked.
    pub update: Option<CfFile>,
}

/// Which CurseForge files the installed jars are, by file name: those whose fingerprint
/// (as the core reports it) and size CurseForge knows. One request for the files and
/// one for their projects, however many jars there are. Jars that are also on Modrinth
/// are included: the Mods tab shows Modrinth's word for those, and this one says the
/// project is installed.
pub async fn identify(
    cf: &CurseForge,
    mods: &[InstalledMod],
) -> Result<HashMap<String, CfInstalled>, CurseForgeError> {
    let prints: Vec<u32> = mods
        .iter()
        .filter_map(|m| m.curseforge_fingerprint)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if prints.is_empty() {
        return Ok(HashMap::new());
    }
    let matches = cf.files_by_fingerprint(&prints).await?;
    // A fingerprint is 32 bits: a file of another size isn't that file.
    let matched = |m: &InstalledMod| {
        matches
            .get(&m.curseforge_fingerprint?)
            .filter(|hit| hit.file.file_length == 0 || hit.file.file_length == m.size)
    };
    let ids: Vec<u64> = mods
        .iter()
        .filter_map(|m| matched(m).map(|hit| hit.mod_id))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let projects: HashMap<u64, CfMod> = if ids.is_empty() {
        HashMap::new()
    } else {
        cf.projects(&ids)
            .await?
            .into_iter()
            .map(|p| (p.id, p))
            .collect()
    };
    Ok(mods
        .iter()
        .filter_map(|m| {
            let hit = matched(m)?;
            let project = projects.get(&hit.mod_id);
            Some((
                m.file_name.clone(),
                CfInstalled {
                    project_id: hit.mod_id,
                    file_id: hit.file.id,
                    title: project
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| file_label(&hit.file)),
                    page_url: project.map(|p| p.page_url()).unwrap_or_else(|| {
                        format!("https://www.curseforge.com/projects/{}", hit.mod_id)
                    }),
                    icon_url: project.and_then(|p| p.icon_url().map(str::to_string)),
                    version: file_label(&hit.file),
                    version_type: hit.file.version_type(),
                    file_date: hit.file.file_date.clone(),
                    update: None,
                },
            ))
        })
        .collect())
}

/// Sets [`CfInstalled::update`] for the jars with a newer file for the server, by the
/// Launcher's rule ([`crate::content::listing::check_curseforge_updates`]): the newest
/// file that fits and is at least as stable as the installed one. Jars Modrinth knows
/// are left out, as the core's own check covers them. One request per project, a few at
/// a time.
pub async fn find_updates(
    cf: &CurseForge,
    target: &ServerTarget,
    mods: &[InstalledMod],
    installed: &mut HashMap<String, CfInstalled>,
) -> Result<(), CurseForgeError> {
    let on_modrinth: HashSet<&str> = mods
        .iter()
        .filter(|m| m.modrinth.is_some())
        .map(|m| m.file_name.as_str())
        .collect();
    let projects: Vec<u64> = installed
        .iter()
        .filter(|(name, _)| !on_modrinth.contains(name.as_str()))
        .map(|(_, cf)| cf.project_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let loader = target.loader_filter();
    let found: Vec<(u64, Result<Vec<CfFile>, CurseForgeError>)> =
        futures::stream::iter(projects.into_iter().map(|project| async move {
            (
                project,
                cf.files(project, Some(&target.game_version), loader).await,
            )
        }))
        .buffer_unordered(UPDATE_LOOKUPS_AT_ONCE)
        .collect()
        .await;
    let mut by_project: HashMap<u64, Vec<CfFile>> = HashMap::new();
    for (project, files) in found {
        match files {
            Ok(files) => {
                by_project.insert(project, files);
            }
            // a project taken off CurseForge has no update
            Err(CurseForgeError::NotFound(_)) => {}
            Err(err) => return Err(err),
        }
    }
    for (name, entry) in installed.iter_mut() {
        entry.update = None;
        if on_modrinth.contains(name.as_str()) {
            continue;
        }
        // newest first
        let newest = by_project.get(&entry.project_id).and_then(|files| {
            files.iter().find(|file| {
                target.fits(file)
                    && curseforge_type_allowed(entry.version_type, file.version_type(), false)
            })
        });
        if let Some(newest) = newest
            && newest.id != entry.file_id
            && newest.file_date > entry.file_date
        {
            entry.update = Some(newest.clone());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Installing

/// A file an install puts on the server.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannedFile {
    pub file: CfFile,
    pub project: Option<CfMod>,
    /// 0 for the picked file, 1 for its dependencies, and so on.
    pub depth: usize,
    /// The installed files of the same project, which it replaces (by file name).
    pub replaces: Vec<String>,
    /// This very file is installed already: nothing is downloaded.
    pub installed: bool,
    /// The Modrinth version that has this very file, for one CurseForge's website keeps
    /// to itself: the core installs that version instead.
    pub on_modrinth: Option<String>,
}

impl PlannedFile {
    pub fn title(&self) -> String {
        self.project
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| file_label(&self.file))
    }

    /// The project's page on curseforge.com.
    pub fn project_page(&self) -> String {
        self.project
            .as_ref()
            .map(|p| p.page_url())
            .unwrap_or_else(|| format!("https://www.curseforge.com/projects/{}", self.file.mod_id))
    }

    /// Where the user downloads the file by hand.
    pub fn file_page(&self) -> String {
        self.file.page_url(&self.project_page())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerInstallPlan {
    /// What is downloaded and uploaded: the picked file first, then its dependencies.
    pub files: Vec<PlannedFile>,
    /// What the user has to download from curseforge.com and upload.
    pub blocked: Vec<PlannedFile>,
    pub skipped: Vec<ModInstallSkipped>,
}

fn skipped(project: impl ToString, reason: impl Into<String>) -> ModInstallSkipped {
    ModInstallSkipped {
        project_id: project.to_string(),
        reason: reason.into(),
    }
}

/// The newest of `files` that fits the server: a release if one fits, else any, and a
/// file that can be downloaded before one that can't ([`crate::content::curseforge_install::pick_file`]).
pub fn pick_file(target: &ServerTarget, mut files: Vec<CfFile>) -> Option<CfFile> {
    files.retain(|f| target.fits(f));
    curseforge::sort_newest_first(&mut files);
    files.sort_by_key(|f| (!f.downloadable(), f.release_type != 1));
    files.into_iter().next()
}

/// Plans what installing `file` puts on the server: the file, its required
/// dependencies that aren't installed (the newest fitting file of each, at most
/// [`MAX_DEPENDENCY_DEPTH`] levels deep), and which installed files each replaces.
/// A file that doesn't fit the server is refused unless `force`, and nothing is
/// planned if a planned file says it is incompatible with an installed project. With
/// `modrinth`, a file CurseForge's website keeps to itself is looked for there.
#[allow(clippy::too_many_arguments)]
pub async fn plan_install(
    cf: &CurseForge,
    modrinth: Option<&Modrinth>,
    target: &ServerTarget,
    installed: &HashMap<String, CfInstalled>,
    file: CfFile,
    project: Option<CfMod>,
    install_dependencies: bool,
    force: bool,
) -> Result<ServerInstallPlan, ServerCfError> {
    if !force && let Some(reason) = target.incompatibility(&file) {
        return Err(ServerCfError::Invalid(reason));
    }
    let mut by_project: HashMap<u64, Vec<(&String, &CfInstalled)>> = HashMap::new();
    for (name, entry) in installed {
        by_project
            .entry(entry.project_id)
            .or_default()
            .push((name, entry));
    }

    let mut plan = vec![PlannedFile {
        file,
        project,
        depth: 0,
        replaces: Vec::new(),
        installed: false,
        on_modrinth: None,
    }];
    let mut seen: HashSet<u64> = HashSet::from([plan[0].file.mod_id]);
    let mut skipped_list = Vec::new();
    if install_dependencies {
        let mut queue = VecDeque::from([0_usize]);
        while let Some(index) = queue.pop_front() {
            let depth = plan[index].depth + 1;
            let dependencies: Vec<u64> = plan[index]
                .file
                .dependencies
                .iter()
                .filter(|d| d.required())
                .map(|d| d.mod_id)
                .collect();
            for dependency in dependencies {
                if by_project.contains_key(&dependency) || !seen.insert(dependency) {
                    continue;
                }
                if depth > MAX_DEPENDENCY_DEPTH {
                    skipped_list.push(skipped(
                        dependency,
                        format!("More than {MAX_DEPENDENCY_DEPTH} levels of dependencies"),
                    ));
                    continue;
                }
                let found = match cf
                    .files(
                        dependency,
                        Some(&target.game_version),
                        target.loader_filter(),
                    )
                    .await
                {
                    Ok(found) => found,
                    Err(CurseForgeError::NotFound(_)) => {
                        skipped_list.push(skipped(dependency, "Not found on CurseForge"));
                        continue;
                    }
                    Err(err) => return Err(err.into()),
                };
                match pick_file(target, found) {
                    Some(file) => {
                        plan.push(PlannedFile {
                            file,
                            project: None,
                            depth,
                            replaces: Vec::new(),
                            installed: false,
                            on_modrinth: None,
                        });
                        queue.push_back(plan.len() - 1);
                    }
                    None => skipped_list.push(skipped(
                        dependency,
                        format!(
                            "No file for {} on Minecraft {}",
                            target.loader.as_str(),
                            target.game_version
                        ),
                    )),
                }
            }
        }
    }

    for planned in &plan {
        for dependency in planned
            .file
            .dependencies
            .iter()
            .filter(|d| d.incompatible() && d.mod_id != planned.file.mod_id)
        {
            if let Some((_, conflict)) = by_project
                .get(&dependency.mod_id)
                .and_then(|files| files.first())
            {
                return Err(ServerCfError::Invalid(format!(
                    "{} is incompatible with the installed {}. Remove it first.",
                    file_label(&planned.file),
                    conflict.title
                )));
            }
        }
    }

    let missing: Vec<u64> = plan
        .iter()
        .filter(|p| p.project.is_none())
        .map(|p| p.file.mod_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if !missing.is_empty() {
        let projects: HashMap<u64, CfMod> = cf
            .projects(&missing)
            .await?
            .into_iter()
            .map(|p| (p.id, p))
            .collect();
        for planned in plan.iter_mut().filter(|p| p.project.is_none()) {
            planned.project = projects.get(&planned.file.mod_id).cloned();
        }
    }

    for planned in &mut plan {
        let same_project = by_project
            .get(&planned.file.mod_id)
            .map(Vec::as_slice)
            .unwrap_or_default();
        planned.installed = same_project
            .iter()
            .any(|(_, entry)| entry.file_id == planned.file.id);
        planned.replaces = same_project
            .iter()
            .filter(|(_, entry)| entry.file_id != planned.file.id)
            .map(|(name, _)| (*name).clone())
            .collect();
        planned.replaces.sort();
    }
    // Files the author keeps on CurseForge's website: the same bytes on Modrinth, which
    // the core installs, else by hand.
    let kept = |p: &PlannedFile| !p.installed && !p.file.downloadable();
    if let Some(modrinth) = modrinth {
        let hashes: Vec<String> = plan
            .iter()
            .filter(|p| kept(p))
            .filter_map(|p| p.file.sha1())
            .collect();
        if !hashes.is_empty() {
            // Modrinth being out of reach only means the user downloads them
            let found = modrinth.versions_by_sha1(&hashes).await.unwrap_or_default();
            for planned in plan.iter_mut().filter(|p| kept(p)) {
                let Some(sha1) = planned.file.sha1() else {
                    continue;
                };
                // The core installs a version's primary file, so only a version whose
                // primary file is these bytes will do; else the wrong jar would land.
                planned.on_modrinth = found
                    .get(&sha1)
                    .filter(|version| {
                        version.primary_file().is_some_and(|f| {
                            f.hashes
                                .sha1
                                .as_deref()
                                .is_some_and(|h| h.eq_ignore_ascii_case(&sha1))
                        })
                    })
                    .map(|version| version.id.clone());
            }
        }
    }
    let (files, blocked): (Vec<_>, Vec<_>) = plan
        .into_iter()
        .partition(|p| p.installed || p.file.downloadable() || p.on_modrinth.is_some());
    Ok(ServerInstallPlan {
        files,
        blocked,
        skipped: skipped_list,
    })
}

/// What an install did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerInstallOutcome {
    /// As the core reports its own installs: the files now there, the names removed,
    /// and what was skipped.
    pub result: ModInstallResult,
    /// The files the user has to download from curseforge.com and upload.
    pub blocked: Vec<PlannedFile>,
}

/// The name a CurseForge file gets on the server: its own, as a plain `.jar` name.
fn jar_name(file: &CfFile) -> Result<String, ServerCfError> {
    crate::content::safe_download_name(ContentKind::Mod, &file.file_name)
        .filter(|name| name.len() <= 255 && !name.contains(['/', '\\']) && !name.contains(".."))
        .ok_or_else(|| {
            ServerCfError::Invalid(format!(
                "{} is not a jar file name that can be used",
                file.file_name
            ))
        })
}

/// A new, empty folder for one install's downloads, under the system's temporary folder.
/// The caller removes it.
pub fn staging_dir() -> std::io::Result<std::path::PathBuf> {
    let dir = std::env::temp_dir().join(format!(
        "lodestone-host-curseforge-{:016x}",
        u64::from_le_bytes(crate::random_bytes())
    ));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Downloads `file` from CurseForge's CDN into `staging` (a folder of the caller's, for
/// this install only) and returns its bytes, checked by SHA-1 and size.
async fn fetch(
    cf: &CurseForge,
    downloader: &Downloader,
    file: &CfFile,
    staging: &Path,
    cancel: &CancelToken,
) -> Result<Vec<u8>, ServerCfError> {
    if file.file_length > MAX_MOD_BYTES {
        return Err(ServerCfError::Invalid(format!(
            "{} is too large for a mod",
            file.file_name
        )));
    }
    let url = cf.download_url(file)?;
    let sha1 = file.sha1().ok_or_else(|| {
        ServerCfError::Invalid(format!("CurseForge sent no SHA-1 for {}", file.file_name))
    })?;
    let dest = staging.join(format!("{}.jar", file.id));
    let item = DownloadItem {
        url,
        dest: dest.clone(),
        sha1,
        size: (file.file_length > 0).then_some(file.file_length),
        executable: false,
    };
    let fetched = downloader.fetch(&item, &Progress::default(), cancel).await;
    let bytes = match fetched {
        Ok(()) => std::fs::read(&dest).map_err(|err| ServerCfError::Download(err.to_string())),
        Err(err) => Err(err.into()),
    };
    let _ = std::fs::remove_file(&dest);
    bytes
}

/// A jar to put on the server.
struct Upload {
    name: String,
    bytes: Vec<u8>,
    /// The installed files it replaces.
    replaces: Vec<String>,
    /// It replaces only disabled files, so it comes disabled.
    disabled: bool,
}

/// The server's files by lowercase name.
fn by_name(mods: &[InstalledMod]) -> HashMap<String, &InstalledMod> {
    mods.iter()
        .map(|m| (m.file_name.to_lowercase(), m))
        .collect()
}

/// The upload of `bytes` as `planned`'s file, after checking that its name is free on
/// the server or belongs to a file it replaces: a file never takes the place of a file
/// that isn't its own project's.
fn upload_of(
    on_server: &HashMap<String, &InstalledMod>,
    planned: &PlannedFile,
    replaces: &[String],
    bytes: impl FnOnce() -> Result<Vec<u8>, ServerCfError>,
) -> Result<Upload, ServerCfError> {
    let name = jar_name(&planned.file)?;
    let replaces_lower: HashSet<String> = replaces.iter().map(|n| n.to_lowercase()).collect();
    for taken in [
        name.to_lowercase(),
        format!("{}.disabled", name.to_lowercase()),
    ] {
        if on_server.contains_key(&taken) && !replaces_lower.contains(&taken) {
            return Err(ServerCfError::Invalid(format!(
                "A file named {name} is already on the server and isn't {}'s. Delete it first.",
                planned.title()
            )));
        }
    }
    let disabled = !replaces.is_empty()
        && replaces
            .iter()
            .all(|n| on_server.get(&n.to_lowercase()).is_some_and(|m| !m.enabled));
    Ok(Upload {
        name,
        bytes: bytes()?,
        replaces: replaces.to_vec(),
        disabled,
    })
}

/// Uploads `uploads` to the core in one go, then deletes the files they replace and
/// turns off the ones that come disabled. Adds what it did to `result`.
async fn place(
    admin: &AdminClient,
    uuid: &str,
    uploads: Vec<Upload>,
    result: &mut ModInstallResult,
) -> Result<(), ServerCfError> {
    if uploads.is_empty() {
        return Ok(());
    }
    for (index, upload) in uploads.iter().enumerate() {
        if uploads[..index]
            .iter()
            .any(|other| other.name.eq_ignore_ascii_case(&upload.name))
        {
            return Err(ServerCfError::Invalid(format!(
                "Two of the files to install are named {}",
                upload.name
            )));
        }
    }
    let overwrite = uploads.iter().any(|upload| {
        upload
            .replaces
            .iter()
            .any(|old| old.eq_ignore_ascii_case(&upload.name))
    });
    let mut files = Vec::new();
    let mut placed = Vec::new();
    for upload in uploads {
        files.push((upload.name.clone(), upload.bytes));
        placed.push((upload.name, upload.replaces, upload.disabled));
    }
    let installed = admin.upload_mods(uuid, &files, overwrite).await?;
    drop(files);
    result.installed.extend(installed);
    for (name, replaces, disabled) in placed {
        for old in replaces {
            // the upload took the place of a file of the same name
            if old.eq_ignore_ascii_case(&name) {
                continue;
            }
            admin.delete_mod(uuid, &old).await?;
            result.removed.push(old);
        }
        if disabled {
            let now = admin.set_mod_enabled(uuid, &name, false).await?;
            if let Some(slot) = result
                .installed
                .iter_mut()
                .find(|m| m.file_name.eq_ignore_ascii_case(&name))
            {
                *slot = now;
            }
        }
    }
    Ok(())
}

/// Runs `plan` on the server `uuid`: every file is downloaded first (so a failed
/// download installs nothing), then uploaded to the core in one go, then the older
/// files they replace are deleted. A file that replaces only disabled files comes
/// disabled. A file Modrinth has in CurseForge's stead is installed by the core, from
/// Modrinth. `mods` is the server's list as the plan saw it: a planned file never takes
/// the place of a file that isn't its own project's.
#[allow(clippy::too_many_arguments)]
pub async fn install(
    cf: &CurseForge,
    downloader: &Downloader,
    admin: &AdminClient,
    uuid: &str,
    mods: &[InstalledMod],
    plan: ServerInstallPlan,
    staging: &Path,
    cancel: &CancelToken,
) -> Result<ServerInstallOutcome, ServerCfError> {
    let on_server = by_name(mods);
    let mut result = ModInstallResult {
        skipped: plan.skipped.clone(),
        ..Default::default()
    };
    let mut uploads: Vec<Upload> = Vec::new();
    for planned in plan
        .files
        .iter()
        .filter(|p| !p.installed && p.on_modrinth.is_none())
    {
        // the name is checked before anything is downloaded
        let name_only = upload_of(&on_server, planned, &planned.replaces, || Ok(Vec::new()))?;
        let bytes = fetch(cf, downloader, &planned.file, staging, cancel).await?;
        uploads.push(Upload { bytes, ..name_only });
    }
    place(admin, uuid, uploads, &mut result).await?;
    // The same files on Modrinth: the core downloads those itself, and replaces the
    // versions of the project it knows. An older file only CurseForge knew goes after.
    for planned in plan.files.iter().filter(|p| !p.installed) {
        let Some(version_id) = &planned.on_modrinth else {
            continue;
        };
        let installed = admin
            .install_mod(
                uuid,
                &ModInstallRequest {
                    version_id: version_id.clone(),
                    install_dependencies: false,
                    // it was checked against the server, or forced, as a CurseForge file
                    force: true,
                },
            )
            .await?;
        let gone: HashSet<String> = installed.removed.iter().map(|n| n.to_lowercase()).collect();
        let kept: HashSet<String> = installed
            .installed
            .iter()
            .map(|m| m.file_name.to_lowercase())
            .collect();
        result.extend(installed);
        for old in &planned.replaces {
            let lower = old.to_lowercase();
            if gone.contains(&lower) || kept.contains(&lower) {
                continue;
            }
            match admin.delete_mod(uuid, old).await {
                Ok(()) => result.removed.push(old.clone()),
                Err(CoreError::NotFound(_)) => {}
                Err(err) => return Err(err.into()),
            }
        }
    }
    Ok(ServerInstallOutcome {
        result,
        blocked: plan.blocked,
    })
}

/// Looks in `folder` (not its subfolders) for the `blocked` files the user downloaded
/// by hand: files named like one of them whose SHA-1 is CurseForge's. Returns (index in
/// `blocked`, path) for each found.
pub fn find_downloaded(folder: &Path, blocked: &[PlannedFile]) -> Vec<(usize, std::path::PathBuf)> {
    let files: Vec<&CfFile> = blocked.iter().map(|planned| &planned.file).collect();
    crate::content::curseforge_install::find_downloaded_files(folder, &files)
}

/// Uploads the file the user downloaded by hand (`downloaded`, checked to be
/// `planned`'s) to the server, in place of the older files of its project. Their
/// download stays where it is. `mods` is the server's list now, and `known` what
/// [`identify`] makes of it. Nothing is done when the very file is installed already.
pub async fn install_downloaded(
    admin: &AdminClient,
    uuid: &str,
    mods: &[InstalledMod],
    known: &HashMap<String, CfInstalled>,
    planned: &PlannedFile,
    downloaded: &Path,
) -> Result<ModInstallResult, ServerCfError> {
    let file = &planned.file;
    if file.sha1().is_none() && file.file_fingerprint == 0 {
        return Err(ServerCfError::Invalid(format!(
            "CurseForge sent nothing to check {} against",
            file.file_name
        )));
    }
    let not_it = || {
        ServerCfError::Invalid(format!(
            "{} isn't {} from CurseForge",
            downloaded.display(),
            file.file_name
        ))
    };
    if std::fs::metadata(downloaded).is_ok_and(|m| m.len() > MAX_MOD_BYTES) {
        return Err(not_it());
    }
    // The bytes are read once, and the ones checked are the ones uploaded: the file
    // can't change in between.
    let bytes =
        std::fs::read(downloaded).map_err(|err| ServerCfError::Download(err.to_string()))?;
    if bytes.len() as u64 > MAX_MOD_BYTES || !is_curseforge_bytes(&bytes, file) {
        return Err(not_it());
    }
    let mut result = ModInstallResult::default();
    if known.values().any(|k| k.file_id == file.id) {
        return Ok(result);
    }
    let on_server = by_name(mods);
    let replaces = replaced_now(&on_server, known, planned);
    let upload = upload_of(&on_server, planned, &replaces, || Ok(bytes))?;
    place(admin, uuid, vec![upload], &mut result).await?;
    Ok(result)
}

/// Whether `bytes` are the CurseForge file `file`: by its SHA-1, else (CurseForge gave
/// none) by its fingerprint and, when known, its length. The rule of
/// [`crate::content::curseforge_install::is_curseforge_file`], on bytes in memory.
fn is_curseforge_bytes(bytes: &[u8], file: &CfFile) -> bool {
    match file.sha1() {
        Some(expected) => crate::game::download::sha1_hex(bytes).eq_ignore_ascii_case(&expected),
        None => {
            file.file_fingerprint != 0
                && u64::from(curseforge::fingerprint(bytes)) == file.file_fingerprint
                && (file.file_length == 0 || bytes.len() as u64 == file.file_length)
        }
    }
}

/// The files on the server now that `planned` replaces: those of its project that
/// `known` (what [`identify`] makes of the server) has, and those the plan found, which
/// still count when `known` came back empty (CurseForge out of reach). Only files still
/// on the server, each once.
fn replaced_now(
    on_server: &HashMap<String, &InstalledMod>,
    known: &HashMap<String, CfInstalled>,
    planned: &PlannedFile,
) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut replaces: Vec<String> = known
        .iter()
        .filter(|(_, k)| k.project_id == planned.file.mod_id)
        .map(|(name, _)| name)
        .chain(planned.replaces.iter())
        .filter(|name| on_server.contains_key(&name.to_lowercase()))
        .filter(|name| seen.insert(name.to_lowercase()))
        .cloned()
        .collect();
    replaces.sort();
    replaces
}

/// Plans and runs the install of `file` on the server `uuid` in one go: what the Mods
/// tab's Install and Update do. `mods` is the server's list and `known` what
/// [`identify`] made of it.
#[allow(clippy::too_many_arguments)]
pub async fn install_file(
    cf: &CurseForge,
    modrinth: Option<&Modrinth>,
    downloader: &Downloader,
    admin: &AdminClient,
    uuid: &str,
    target: &ServerTarget,
    mods: &[InstalledMod],
    known: &HashMap<String, CfInstalled>,
    file: CfFile,
    project: Option<CfMod>,
    install_dependencies: bool,
    force: bool,
) -> Result<ServerInstallOutcome, ServerCfError> {
    let plan = plan_install(
        cf,
        modrinth,
        target,
        known,
        file,
        project,
        install_dependencies,
        force,
    )
    .await?;
    let staging = staging_dir().map_err(|err| ServerCfError::Download(err.to_string()))?;
    let outcome = install(
        cf,
        downloader,
        admin,
        uuid,
        mods,
        plan,
        &staging,
        &CancelToken::new(),
    )
    .await;
    let _ = std::fs::remove_dir_all(&staging);
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, tags: &[&str]) -> CfFile {
        serde_json::from_value(serde_json::json!({
            "id": 7,
            "modId": 3,
            "displayName": name,
            "fileName": name,
            "gameVersions": tags,
        }))
        .unwrap()
    }

    fn target(kind: InstalledModKind, loader: InstalledModLoader) -> ServerTarget {
        ServerTarget {
            kind,
            loader,
            game_version: "1.20.1".into(),
        }
    }

    #[test]
    fn mods_fit_by_game_version_and_loader_and_plugins_by_game_version() {
        let fabric = target(InstalledModKind::Mod, InstalledModLoader::Fabric);
        assert!(fabric.fits(&file("a.jar", &["1.20.1", "Fabric"])));
        assert!(!fabric.fits(&file("a.jar", &["1.20.1", "Forge"])));
        assert!(!fabric.fits(&file("a.jar", &["1.19.2", "Fabric"])));
        // NeoForge forked from Forge in 1.20.1
        let neoforge = target(InstalledModKind::Mod, InstalledModLoader::NeoForge);
        assert!(neoforge.fits(&file("a.jar", &["1.20.1", "Forge"])));
        assert_eq!(
            neoforge.accepted_loaders(),
            [Loader::NeoForge, Loader::Forge]
        );
        assert_eq!(neoforge.loader_filter(), None);

        let paper = target(InstalledModKind::Plugin, InstalledModLoader::Paper);
        assert_eq!(paper.class_id(), CLASS_BUKKIT_PLUGINS);
        assert!(paper.accepted_loaders().is_empty());
        assert!(paper.fits(&file("p.jar", &["1.20.1", "1.20"])));
        assert!(!paper.fits(&file("p.jar", &["1.19"])));
        // a plugin tagged with no game version isn't refused for it
        assert!(paper.fits(&file("p.jar", &[])));
    }

    #[test]
    fn a_file_is_shown_as_a_version_of_its_project() {
        let mut cf: CfFile = file("Sodium 0.5.3.jar", &["1.20.1", "Fabric", "Client"]);
        cf.release_type = 2;
        let version = version(&cf);
        assert_eq!(version.id, "7");
        assert_eq!(version.project_id, "3");
        assert_eq!(version.version_number, "Sodium 0.5.3");
        assert_eq!(version.version_type, ModrinthVersionType::Beta);
        assert_eq!(version.game_versions, ["1.20.1"]);
        assert_eq!(version.loaders, ["fabric"]);
        assert_eq!(file_label(&file(".jar", &[])), ".jar");
    }

    #[test]
    fn links_and_sorts() {
        assert!(looks_like_curseforge_link(
            "https://www.CurseForge.com/minecraft/mc-mods/jei"
        ));
        assert!(!looks_like_curseforge_link("jei"));
        assert_eq!(sort_of(ModrinthSearchIndex::Relevance), CfSort::Popularity);
        assert_eq!(
            sort_of(ModrinthSearchIndex::Downloads),
            CfSort::TotalDownloads
        );
        assert_eq!(sort_of(ModrinthSearchIndex::Updated), CfSort::LastUpdated);
    }
}
