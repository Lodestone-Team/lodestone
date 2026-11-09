//! The content list an instance's Content tab shows: the scanned files, matched to
//! Modrinth and CurseForge through [`Provenance`], with each pack's enabled state and
//! order and the shader mod's selection.

use std::collections::{BTreeSet, HashMap};

use lodestone_minecraft::{
    hash,
    modrinth::{ModrinthVersionType, RawVersion},
};

use super::{
    ContentError, ContentKind, CurseForge, CurseForgeError, InstanceDirs, Modrinth, Platforms,
    Target,
    curseforge::{self, CfFile, CfMod},
    io_error,
    options::{OptionsError, PackOptions},
    packs::{self, Compatibility},
    provenance::{CurseForgeSource, Entry, ModrinthSource, Provenance, nanos},
    scan::{self, ScannedFile},
    shaders::{self, ShaderLoader, ShaderSelection},
};

/// One file of the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub file: ScannedFile,
    pub sha512: Option<String>,
    pub modrinth: Option<ModrinthSource>,
    pub curseforge: Option<CurseForgeSource>,
    /// Mods: not `.disabled`. Resource packs: in `options.txt`'s list. Shader packs: the
    /// selected one.
    pub enabled: bool,
    /// Enabled resource packs: 0 is the highest priority.
    pub priority: Option<usize>,
    /// Resource packs: whether its formats fit the instance's version.
    pub compatibility: Compatibility,
}

impl Item {
    /// The name to show: the platform's title, the metadata's name, or the file name.
    pub fn display_name(&self) -> String {
        if let Some(source) = &self.modrinth
            && !source.title.is_empty()
        {
            return source.title.clone();
        }
        if let Some(source) = &self.curseforge
            && !source.title.is_empty()
        {
            return source.title.clone();
        }
        self.file.display_name()
    }

    /// The version to show: Modrinth's version number, else the metadata's, else
    /// CurseForge's file name for it (often long, so last).
    pub fn display_version(&self) -> Option<String> {
        self.modrinth
            .as_ref()
            .map(|m| m.version_number.clone())
            .filter(|v| !v.is_empty())
            .or_else(|| self.file.meta.version.clone())
            .or_else(|| {
                self.curseforge
                    .as_ref()
                    .map(|c| c.display_name.clone())
                    .filter(|v| !v.is_empty())
            })
    }

    /// Whether a platform knows the file.
    pub fn identified(&self) -> bool {
        self.modrinth.is_some() || self.curseforge.is_some()
    }
}

/// The shader mod and the pack it uses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShaderState {
    pub loader: Option<ShaderLoader>,
    pub selection: ShaderSelection,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Listing {
    pub kind: ContentKind,
    pub items: Vec<Item>,
    /// Why some files couldn't be matched to a platform (offline, rate limited).
    pub lookup_error: Option<String>,
    /// Why `options.txt` couldn't be read, for resource packs.
    pub options_error: Option<String>,
    pub shaders: ShaderState,
}

/// The sha512 and sha1 of each of `files`, in order, read on several threads. A file
/// whose sha512 matches what `provenance` has keeps its known sha1.
fn hash_files(
    dir: &std::path::Path,
    files: &[&ScannedFile],
    provenance: &Provenance,
    kind: ContentKind,
) -> Result<Vec<(String, String)>, ContentError> {
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(files.len())
        .max(1);
    let hash_one = |file: &ScannedFile| -> Result<(String, String), ContentError> {
        let path = dir.join(&file.file_name);
        let sha512 = hash::sha512_of(&path).map_err(io_error(&path))?;
        let known = provenance
            .get(kind, &file.file_name)
            .filter(|e| e.sha512 == sha512)
            .map(|e| e.sha1.clone());
        let sha1 = match known {
            Some(sha1) => sha1,
            None => hash::sha1_of(&path).map_err(io_error(&path))?,
        };
        Ok((sha512, sha1))
    };
    std::thread::scope(|scope| {
        let handles: Vec<_> = files
            .chunks(files.len().div_ceil(threads))
            .map(|chunk| scope.spawn(move || chunk.iter().map(|f| hash_one(f)).collect::<Vec<_>>()))
            .collect();
        let mut out = Vec::with_capacity(files.len());
        for handle in handles {
            let results = handle
                .join()
                .map_err(|_| ContentError::Invalid("Hashing a file failed.".into()))?;
            for result in results {
                out.push(result?);
            }
        }
        Ok(out)
    })
}

/// Brings `content.json` up to date with the folder of `kind`: new and changed files are
/// hashed, and files without a source are looked up: on Modrinth by SHA-1, then the ones
/// Modrinth didn't know on CurseForge by fingerprint. Returns the files, the provenance,
/// and why a lookup failed if one did.
pub async fn refresh(
    platforms: impl Into<Platforms<'_>>,
    dirs: &InstanceDirs,
    kind: ContentKind,
) -> Result<(Vec<ScannedFile>, Provenance, Option<String>), ContentError> {
    let platforms = platforms.into();
    let api = platforms.modrinth;
    let files = scan::scan(dirs, kind)?;
    let mut provenance = Provenance::load(&dirs.instance);
    let before = provenance.clone();
    provenance.retain_present(kind, &files);
    let dir = dirs.content_dir(kind);
    // Files not hashed yet (new, or changed since): hashed on up to one thread per
    // core, since a new instance's first listing hashes every file.
    let stale: Vec<&ScannedFile> = files
        .iter()
        .filter(|f| !f.is_dir && provenance.current(f).is_none())
        .collect();
    let hashed = hash_files(&dir, &stale, &provenance, kind)?;
    for (file, (sha512, sha1)) in stale.into_iter().zip(hashed) {
        let old = provenance.get(kind, &file.file_name).cloned();
        let same = old.as_ref().filter(|e| e.sha512 == sha512);
        provenance.insert(
            kind,
            &file.file_name,
            Entry {
                size: file.size,
                modified: file.modified.and_then(nanos),
                sha1,
                sha512,
                modrinth: same.and_then(|e| e.modrinth.clone()),
                looked_up: same.is_some_and(|e| e.looked_up),
                curseforge: same.and_then(|e| e.curseforge.clone()),
                fingerprint: same.and_then(|e| e.fingerprint),
                curseforge_looked_up: same.is_some_and(|e| e.curseforge_looked_up),
            },
        );
    }

    let mut lookup_error = None;
    if let Some(api) = api {
        let prefix = format!("{}/", kind.dir());
        let unknown: Vec<String> = provenance
            .files
            .iter()
            .filter(|(k, e)| k.starts_with(&prefix) && e.modrinth.is_none() && !e.looked_up)
            .map(|(_, e)| e.sha1.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if !unknown.is_empty() {
            match lookup(api, &unknown).await {
                Ok(found) => {
                    for (k, entry) in provenance.files.iter_mut() {
                        if !k.starts_with(&prefix) || entry.modrinth.is_some() || entry.looked_up {
                            continue;
                        }
                        match found.get(&entry.sha1) {
                            Some((version, project)) => {
                                entry.modrinth = Some(ModrinthSource::new(
                                    version,
                                    project.as_ref(),
                                    &entry.sha512,
                                ));
                            }
                            None => entry.looked_up = true,
                        }
                    }
                }
                Err(err) => lookup_error = Some(err.to_string()),
            }
        }
    }
    if let Some(cf) = platforms.curseforge {
        // Files Modrinth didn't know (or every file without a source, when Modrinth
        // can't be asked).
        let prefix = format!("{}/", kind.dir());
        let on_disk: HashMap<String, &str> = files
            .iter()
            .filter(|f| !f.is_dir)
            .map(|f| {
                (
                    super::provenance::key(kind, &f.file_name),
                    f.file_name.as_str(),
                )
            })
            .collect();
        let candidates: Vec<String> = provenance
            .files
            .iter()
            .filter(|(k, e)| {
                k.starts_with(&prefix)
                    && !e.identified()
                    && !e.curseforge_looked_up
                    && (e.looked_up || api.is_none())
                    && on_disk.contains_key(*k)
            })
            .map(|(k, _)| k.clone())
            .collect();
        let mut prints = BTreeSet::new();
        for key in &candidates {
            let Some(entry) = provenance.files.get_mut(key) else {
                continue;
            };
            if entry.fingerprint.is_none() {
                let path = dir.join(on_disk[key]);
                entry.fingerprint = curseforge::fingerprint_file(&path).ok();
            }
            prints.extend(entry.fingerprint);
        }
        if !prints.is_empty() {
            let prints: Vec<u32> = prints.into_iter().collect();
            match curseforge_lookup(cf, &prints).await {
                Ok(found) => {
                    for key in &candidates {
                        let Some(entry) = provenance.files.get_mut(key) else {
                            continue;
                        };
                        match entry.fingerprint.and_then(|print| found.get(&print)) {
                            Some((file, project)) => {
                                entry.curseforge =
                                    Some(CurseForgeSource::new(file, project.as_ref()))
                            }
                            None if entry.fingerprint.is_some() => {
                                entry.curseforge_looked_up = true
                            }
                            None => {}
                        }
                    }
                }
                Err(err) => {
                    lookup_error.get_or_insert(err.to_string());
                }
            }
        }
    }
    if provenance != before {
        // The list still works without the cache, so a failed save isn't an error.
        let _ = Provenance::update(&dirs.instance, |current| {
            current.merge_refreshed(kind, &provenance, &dir)
        });
    }
    Ok((files, provenance, lookup_error))
}

pub(crate) type Found = HashMap<
    String,
    (
        RawVersion,
        Option<lodestone_minecraft::modrinth::ModrinthProject>,
    ),
>;

/// Looks up SHA-1 hashes, with the projects of the versions found.
pub(crate) async fn lookup(api: &Modrinth, hashes: &[String]) -> Result<Found, ContentError> {
    let versions = api.versions_by_sha1(hashes).await?;
    let ids: Vec<String> = versions
        .values()
        .map(|v| v.project_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let projects: HashMap<String, _> = if ids.is_empty() {
        HashMap::new()
    } else {
        api.projects(&ids)
            .await?
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect()
    };
    Ok(versions
        .into_iter()
        .map(|(hash, version)| {
            let project = projects.get(&version.project_id).cloned();
            (hash, (version, project))
        })
        .collect())
}

/// Looks up fingerprints on CurseForge, with the projects of the files found.
pub(crate) async fn curseforge_lookup(
    cf: &CurseForge,
    prints: &[u32],
) -> Result<HashMap<u32, (CfFile, Option<CfMod>)>, ContentError> {
    let matches = cf.files_by_fingerprint(prints).await?;
    let ids: Vec<u64> = matches
        .values()
        .map(|m| m.mod_id)
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
    Ok(matches
        .into_iter()
        .map(|(print, hit)| {
            let project = projects.get(&hit.mod_id).cloned();
            (print, (hit.file, project))
        })
        .collect())
}

/// The shader mod in the instance's enabled mods, and the pack it uses.
pub fn shader_state(dirs: &InstanceDirs) -> Result<ShaderState, ContentError> {
    let mods = scan::scan(dirs, ContentKind::Mod)?;
    let provenance = Provenance::load(&dirs.instance);
    let loader = shaders::detect(&mods, &provenance);
    let selection = match loader {
        Some(loader) => shaders::read_selection(&dirs.game, loader)?,
        None => ShaderSelection::default(),
    };
    Ok(ShaderState { loader, selection })
}

/// Lists the content of `kind`. Without the platforms (or when they can't be reached)
/// files are matched only from what `content.json` already knows.
pub async fn list(
    platforms: impl Into<Platforms<'_>>,
    dirs: &InstanceDirs,
    target: &Target,
    kind: ContentKind,
) -> Result<Listing, ContentError> {
    let (files, provenance, lookup_error) = refresh(platforms, dirs, kind).await?;
    let shaders = if kind == ContentKind::ShaderPack {
        shader_state(dirs)?
    } else {
        ShaderState::default()
    };
    let (pack_options, options_error) = if kind == ContentKind::ResourcePack {
        match PackOptions::read(&dirs.game) {
            Ok(options) => (options, None),
            Err(err @ OptionsError::Unreadable { .. }) => {
                (PackOptions::default(), Some(err.to_string()))
            }
            Err(err) => return Err(ContentError::Invalid(err.to_string())),
        }
    } else {
        (PackOptions::default(), None)
    };
    let order = pack_options.enabled_files(&target.game_version);
    let items = files
        .into_iter()
        .map(|file| {
            let entry = provenance.get(kind, &file.file_name);
            let priority = order.iter().position(|name| name == &file.file_name);
            let enabled = match kind {
                ContentKind::Mod => file.enabled_by_name(),
                ContentKind::ResourcePack => priority.is_some(),
                ContentKind::ShaderPack => {
                    shaders.selection.pack.as_deref() == Some(file.file_name.as_str())
                }
            };
            let compatibility = if kind == ContentKind::ResourcePack {
                packs::compatibility(file.meta.formats.as_ref(), &target.game_version)
            } else {
                Compatibility::Unknown
            };
            Item {
                sha512: entry.map(|e| e.sha512.clone()),
                modrinth: entry.and_then(|e| e.modrinth.clone()),
                curseforge: entry.and_then(|e| e.curseforge.clone()),
                enabled,
                priority,
                compatibility,
                file,
            }
        })
        .collect();
    Ok(Listing {
        kind,
        items,
        lookup_error,
        options_error,
        shaders,
    })
}

/// The release types an update of a file may be, by the installed version's type:
/// at least as stable, unless `prerelease` (the instance's "Beta and alpha updates")
/// takes any. `None` puts no limit.
pub fn update_types(
    installed: Option<ModrinthVersionType>,
    prerelease: bool,
) -> Option<&'static [&'static str]> {
    if prerelease {
        return None;
    }
    match installed.unwrap_or(ModrinthVersionType::Release) {
        ModrinthVersionType::Release => Some(&["release"]),
        ModrinthVersionType::Beta => Some(&["release", "beta"]),
        ModrinthVersionType::Alpha => None,
    }
}

/// The newer versions of the listed files that came from Modrinth, by file name.
/// Without `prerelease`, a file only updates to a version at least as stable as the
/// one installed (see [`update_types`]).
pub async fn check_updates(
    api: &Modrinth,
    dirs: &InstanceDirs,
    target: &Target,
    kind: ContentKind,
    prerelease: bool,
) -> Result<HashMap<String, RawVersion>, ContentError> {
    let provenance = Provenance::load(&dirs.instance);
    let files = scan::scan(dirs, kind)?;
    let mut by_hash: HashMap<String, (String, ModrinthSource)> = HashMap::new();
    for file in &files {
        if let Some(entry) = provenance.current(file)
            && let Some(source) = &entry.modrinth
        {
            by_hash.insert(entry.sha1.clone(), (file.file_name.clone(), source.clone()));
        }
    }
    if by_hash.is_empty() {
        return Ok(HashMap::new());
    }
    // One request per set of allowed release types.
    let mut groups: Vec<(Option<&[&str]>, Vec<String>)> = Vec::new();
    for (hash, (_, source)) in &by_hash {
        let types = update_types(source.version_type, prerelease);
        match groups.iter_mut().find(|(t, _)| *t == types) {
            Some((_, hashes)) => hashes.push(hash.clone()),
            None => groups.push((types, vec![hash.clone()])),
        }
    }
    let mut latest = HashMap::new();
    for (types, hashes) in groups {
        latest.extend(
            api.latest_by_sha1(
                &hashes,
                &target.accepted_loaders(kind),
                std::slice::from_ref(&target.game_version),
                types,
            )
            .await?,
        );
    }
    let mut updates = HashMap::new();
    for (hash, version) in latest {
        let Some((file_name, source)) = by_hash.get(&hash) else {
            continue;
        };
        if version.id != source.version_id
            && version.project_id == source.project_id
            && version.date_published > source.date_published
            && target.incompatibility(kind, &version).is_none()
        {
            updates.insert(file_name.clone(), version);
        }
    }
    Ok(updates)
}

/// A newer CurseForge file for an installed one.
#[derive(Clone, Debug, PartialEq)]
pub struct CurseForgeUpdate {
    pub file: CfFile,
    /// What is installed.
    pub installed: CurseForgeSource,
}

impl CurseForgeUpdate {
    /// Whether the launcher may download it; otherwise the user downloads it from
    /// [`CurseForgeUpdate::page_url`].
    pub fn downloadable(&self) -> bool {
        self.file.downloadable()
    }

    pub fn page_url(&self) -> String {
        self.file.page_url(&self.installed.page_url)
    }
}

/// Whether a CurseForge file of type `candidate` ("release", "beta", "alpha") may update
/// one of type `installed`, by the same rule as [`update_types`].
pub fn curseforge_type_allowed(installed: &str, candidate: &str, prerelease: bool) -> bool {
    let installed = match installed {
        "beta" => ModrinthVersionType::Beta,
        "alpha" => ModrinthVersionType::Alpha,
        _ => ModrinthVersionType::Release,
    };
    update_types(Some(installed), prerelease).is_none_or(|allowed| allowed.contains(&candidate))
}

/// The newer files of the listed files that came from CurseForge (and not also from
/// Modrinth, whose check covers those), by file name. Files the author doesn't let
/// launchers download are included: the app links to them instead.
pub async fn check_curseforge_updates(
    cf: &CurseForge,
    dirs: &InstanceDirs,
    target: &Target,
    kind: ContentKind,
    prerelease: bool,
) -> Result<HashMap<String, CurseForgeUpdate>, ContentError> {
    let provenance = Provenance::load(&dirs.instance);
    let files = scan::scan(dirs, kind)?;
    let mut installed: Vec<(String, CurseForgeSource)> = Vec::new();
    for file in &files {
        if let Some(entry) = provenance.current(file)
            && entry.modrinth.is_none()
            && let Some(source) = &entry.curseforge
        {
            installed.push((file.file_name.clone(), source.clone()));
        }
    }
    // CurseForge filters by one loader at most; with more (Quilt also takes Fabric
    // files) the files are filtered here instead.
    let loaders = curseforge::accepted_loaders(target.loader, &target.game_version);
    let loader = match (kind, loaders.as_slice()) {
        (ContentKind::Mod, [only]) => Some(*only),
        _ => None,
    };
    let mut by_project: HashMap<u64, Vec<CfFile>> = HashMap::new();
    let mut updates = HashMap::new();
    for (file_name, source) in installed {
        if let std::collections::hash_map::Entry::Vacant(slot) = by_project.entry(source.project_id)
        {
            let found = match cf
                .files(source.project_id, Some(&target.game_version), loader)
                .await
            {
                Ok(found) => found,
                Err(CurseForgeError::NotFound(_)) => Vec::new(),
                Err(err) => return Err(err.into()),
            };
            slot.insert(found);
        }
        let newest = by_project[&source.project_id].iter().find(|file| {
            target.curseforge_incompatibility(kind, file).is_none()
                && curseforge_type_allowed(&source.version_type, file.version_type(), prerelease)
        });
        if let Some(newest) = newest
            && newest.id != source.file_id
            && newest.file_date > source.file_date
        {
            updates.insert(
                file_name,
                CurseForgeUpdate {
                    file: newest.clone(),
                    installed: source,
                },
            );
        }
    }
    Ok(updates)
}

#[cfg(test)]
mod update_type_tests {
    use super::*;

    #[test]
    fn updates_are_at_least_as_stable_unless_prereleases_are_on() {
        use ModrinthVersionType::*;
        assert_eq!(update_types(Some(Release), false), Some(&["release"][..]));
        assert_eq!(update_types(None, false), Some(&["release"][..]));
        assert_eq!(
            update_types(Some(Beta), false),
            Some(&["release", "beta"][..])
        );
        assert_eq!(update_types(Some(Alpha), false), None);
        assert_eq!(update_types(Some(Release), true), None);
    }

    #[test]
    fn curseforge_updates_follow_the_same_rule() {
        assert!(curseforge_type_allowed("release", "release", false));
        assert!(!curseforge_type_allowed("release", "beta", false));
        assert!(curseforge_type_allowed("beta", "beta", false));
        assert!(!curseforge_type_allowed("beta", "alpha", false));
        assert!(curseforge_type_allowed("alpha", "alpha", false));
        assert!(curseforge_type_allowed("release", "alpha", true));
    }
}
