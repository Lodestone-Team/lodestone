//! The content list an instance's Content tab shows: the scanned files, matched to
//! Modrinth through [`Provenance`], with each pack's enabled state and order and the
//! shader mod's selection.

use std::collections::{BTreeSet, HashMap};

use lodestone_minecraft::{
    hash,
    modrinth::{ModrinthVersionType, RawVersion},
};

use super::{
    ContentError, ContentKind, InstanceDirs, Modrinth, Target, io_error,
    options::{OptionsError, PackOptions},
    packs::{self, Compatibility},
    provenance::{Entry, ModrinthSource, Provenance, nanos},
    scan::{self, ScannedFile},
    shaders::{self, ShaderLoader, ShaderSelection},
};

/// One file of the list.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub file: ScannedFile,
    pub sha512: Option<String>,
    pub modrinth: Option<ModrinthSource>,
    /// Mods: not `.disabled`. Resource packs: in `options.txt`'s list. Shader packs: the
    /// selected one.
    pub enabled: bool,
    /// Enabled resource packs: 0 is the highest priority.
    pub priority: Option<usize>,
    /// Resource packs: whether its formats fit the instance's version.
    pub compatibility: Compatibility,
}

impl Item {
    /// The name to show: Modrinth's title, the metadata's name, or the file name.
    pub fn display_name(&self) -> String {
        match &self.modrinth {
            Some(source) if !source.title.is_empty() => source.title.clone(),
            _ => self.file.display_name(),
        }
    }

    /// The version to show: Modrinth's version number, else the metadata's.
    pub fn display_version(&self) -> Option<String> {
        self.modrinth
            .as_ref()
            .map(|m| m.version_number.clone())
            .filter(|v| !v.is_empty())
            .or_else(|| self.file.meta.version.clone())
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
    /// Why some files couldn't be matched to Modrinth (offline, rate limited).
    pub lookup_error: Option<String>,
    /// Why `options.txt` couldn't be read, for resource packs.
    pub options_error: Option<String>,
    pub shaders: ShaderState,
}

/// Brings `content.json` up to date with the folder of `kind`: new and changed files are
/// hashed, and, with `api`, files without a source are looked up on Modrinth by SHA-1.
/// Returns the files, the provenance, and why the lookup failed if it did.
pub async fn refresh(
    api: Option<&Modrinth>,
    dirs: &InstanceDirs,
    kind: ContentKind,
) -> Result<(Vec<ScannedFile>, Provenance, Option<String>), ContentError> {
    let files = scan::scan(dirs, kind)?;
    let mut provenance = Provenance::load(&dirs.instance);
    let before = provenance.clone();
    provenance.retain_present(kind, &files);
    let dir = dirs.content_dir(kind);
    for file in files.iter().filter(|f| !f.is_dir) {
        if provenance.current(file).is_some() {
            continue;
        }
        let path = dir.join(&file.file_name);
        let sha512 = hash::sha512_of(&path).map_err(io_error(&path))?;
        let old = provenance.get(kind, &file.file_name).cloned();
        let same = old.as_ref().filter(|e| e.sha512 == sha512);
        let sha1 = match same {
            Some(entry) => entry.sha1.clone(),
            None => hash::sha1_of(&path).map_err(io_error(&path))?,
        };
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

/// Lists the content of `kind`. Without `api` (or when Modrinth can't be reached) files
/// are matched only from what `content.json` already knows.
pub async fn list(
    api: Option<&Modrinth>,
    dirs: &InstanceDirs,
    target: &Target,
    kind: ContentKind,
) -> Result<Listing, ContentError> {
    let (files, provenance, lookup_error) = refresh(api, dirs, kind).await?;
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
}
