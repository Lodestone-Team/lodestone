//! CurseForge modpacks: a zip with `manifest.json` at its root, listing CurseForge files
//! by project and file id, and an `overrides/` folder (named by the manifest) copied over
//! the game folder.
//!
//! [`read_curseforge_pack`] reads and checks the zip; [`resolve`] asks CurseForge which
//! files those are and turns the pack into the same [`PackInfo`] a `.mrpack` gives, so
//! [`super::import`] makes the instance the same way. Files go to the folder their
//! project's class says (mods, resource packs, shader packs); optional mods come
//! disabled. A file whose author doesn't let launchers download it is taken from
//! Modrinth when Modrinth has the same bytes (by SHA-1), else left in
//! [`ResolvedPack::blocked`] for the user to download from its page.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    io::Read,
    path::{Path, PathBuf},
};

use lodestone_minecraft::{
    modrinth::search::is_valid_filter_value,
    mrpack::{PackDownload, SkippedPath},
};
use serde::Deserialize;

use super::{ModpackError, OverrideFile, PackInfo, invalid, open_pack, read_overrides, zip_error};
use crate::{
    content::{
        ContentKind, CurseForge, Modrinth, curseforge, curseforge_install::CfPlannedFile,
        safe_download_name,
    },
    game::loaders::LoaderKind,
};

pub const MANIFEST_FILE: &str = "manifest.json";
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    #[serde(default)]
    manifest_type: String,
    #[serde(default)]
    manifest_version: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    author: String,
    minecraft: ManifestMinecraft,
    #[serde(default)]
    files: Vec<ManifestFile>,
    #[serde(default)]
    overrides: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<ManifestLoader>,
}

#[derive(Deserialize)]
struct ManifestLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
struct ManifestFile {
    #[serde(rename = "projectID")]
    project_id: u64,
    #[serde(rename = "fileID")]
    file_id: u64,
    #[serde(default = "yes")]
    required: bool,
}

fn yes() -> bool {
    true
}

/// A CurseForge pack that was read and checked.
#[derive(Clone, Debug)]
pub struct CfPack {
    pub name: String,
    pub version: Option<String>,
    pub author: Option<String>,
    pub game_version: String,
    pub loader: Option<(LoaderKind, String)>,
    pub files: Vec<CfPackFile>,
    pub overrides: Vec<OverrideFile>,
    pub skipped: Vec<SkippedPath>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CfPackFile {
    pub project_id: u64,
    pub file_id: u64,
    pub required: bool,
}

/// Whether the zip at `path` is a CurseForge pack: it has `manifest.json` at its root
/// (and no Modrinth index, which takes precedence).
pub fn is_curseforge_pack(path: &Path) -> bool {
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(archive) = zip::ZipArchive::new(file) else {
        return false;
    };
    let names: HashSet<&str> = archive.file_names().collect();
    names.contains(MANIFEST_FILE) && !names.contains(lodestone_minecraft::mrpack::INDEX_FILE)
}

/// The loader a manifest's loader id names, and its version as [`super::loader_choice`]
/// takes it: `forge-47.2.0`, `fabric-0.15.0`, `quilt-0.26.0`, `neoforge-21.1.77`, and
/// the `neoforge-1.20.1-47.1.84` CurseForge writes for NeoForge on 1.20.1.
pub fn manifest_loader(id: &str, game_version: &str) -> Result<(LoaderKind, String), String> {
    let id = id.trim();
    let (name, version) = id
        .split_once('-')
        .ok_or_else(|| format!("The pack's loader {id:?} isn't one Lodestone Launcher knows."))?;
    let kind = match name.to_ascii_lowercase().as_str() {
        "forge" => LoaderKind::Forge,
        "neoforge" => LoaderKind::NeoForge,
        "fabric" => LoaderKind::Fabric,
        "quilt" => LoaderKind::Quilt,
        other => {
            return Err(format!(
                "The pack needs {other}, which Lodestone Launcher doesn't support."
            ));
        }
    };
    let version = match kind {
        LoaderKind::NeoForge => version
            .strip_prefix(&format!("{game_version}-"))
            .unwrap_or(version),
        _ => version,
    };
    if version.is_empty() || !is_valid_filter_value(version) || version == "recommended" {
        return Err(format!(
            "The pack's {} version {version:?} isn't usable.",
            kind.label()
        ));
    }
    Ok((kind, version.to_string()))
}

/// Reads and checks the CurseForge pack at `path`.
pub fn read_curseforge_pack(path: &Path) -> Result<CfPack, ModpackError> {
    let mut archive = open_pack(path)?;
    let manifest: Manifest = {
        let entry = archive
            .by_name(MANIFEST_FILE)
            .map_err(|_| invalid(format!("it has no {MANIFEST_FILE}")))?;
        if entry.size() > MAX_MANIFEST_BYTES {
            return Err(invalid(format!("{MANIFEST_FILE} is too large")));
        }
        let mut bytes = Vec::new();
        entry
            .take(MAX_MANIFEST_BYTES)
            .read_to_end(&mut bytes)
            .map_err(|e| invalid(format!("{MANIFEST_FILE} can't be read ({e})")))?;
        serde_json::from_slice(&bytes)
            .map_err(|e| invalid(format!("{MANIFEST_FILE} is invalid ({e})")))?
    };
    if !manifest.manifest_type.is_empty() && manifest.manifest_type != "minecraftModpack" {
        return Err(ModpackError::Invalid(format!(
            "The zip is a CurseForge {}, not a modpack.",
            manifest.manifest_type
        )));
    }
    if manifest.manifest_version > 1 {
        return Err(ModpackError::Invalid(format!(
            "CurseForge manifest version {} isn't supported.",
            manifest.manifest_version
        )));
    }
    // Some packs write "1.20.1." with a trailing dot.
    let game_version = manifest
        .minecraft
        .version
        .trim()
        .trim_end_matches('.')
        .to_string();
    if game_version.is_empty() || !is_valid_filter_value(&game_version) {
        return Err(ModpackError::Invalid(
            "The modpack's Minecraft version isn't valid.".into(),
        ));
    }
    let loader = match manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|l| l.primary)
        .or_else(|| manifest.minecraft.mod_loaders.first())
    {
        Some(loader) => {
            Some(manifest_loader(&loader.id, &game_version).map_err(ModpackError::Invalid)?)
        }
        None => None,
    };
    let overrides_dir = manifest
        .overrides
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .unwrap_or("overrides")
        .trim_matches('/');
    if overrides_dir.contains("..") || overrides_dir.contains('\\') {
        return Err(ModpackError::Invalid(
            "The modpack is unsafe: its overrides folder leaves the pack".into(),
        ));
    }
    let mut skipped = Vec::new();
    let prefix = format!("{overrides_dir}/");
    let overrides = read_overrides(&mut archive, &[prefix.as_str()], &mut skipped)?;
    let _ = zip_error;
    let mut seen = HashSet::new();
    let files = manifest
        .files
        .iter()
        .filter(|f| seen.insert(f.file_id))
        .map(|f| CfPackFile {
            project_id: f.project_id,
            file_id: f.file_id,
            required: f.required,
        })
        .collect();
    let name = manifest.name.trim().to_string();
    Ok(CfPack {
        name: if name.is_empty() {
            "Modpack".into()
        } else {
            name
        },
        version: Some(manifest.version.trim().to_string()).filter(|v| !v.is_empty()),
        author: Some(manifest.author.trim().to_string()).filter(|v| !v.is_empty()),
        game_version,
        loader,
        files,
        overrides,
        skipped,
    })
}

/// A CurseForge pack with its files found: what to download, and what the user downloads
/// by hand once the instance exists.
#[derive(Clone, Debug)]
pub struct ResolvedPack {
    pub info: PackInfo,
    pub blocked: Vec<CfPlannedFile>,
}

/// The folder files of a project's class go to, or why they aren't installed.
fn folder_of(class: Option<u32>, file_name: &str) -> Result<ContentKind, String> {
    match class {
        Some(curseforge::CLASS_MODS) => Ok(ContentKind::Mod),
        Some(curseforge::CLASS_RESOURCE_PACKS) => Ok(ContentKind::ResourcePack),
        Some(curseforge::CLASS_SHADERS) => Ok(ContentKind::ShaderPack),
        Some(curseforge::CLASS_WORLDS) => Err("Worlds from modpacks aren't installed".into()),
        Some(curseforge::CLASS_DATA_PACKS) => {
            Err("Data packs from modpacks aren't installed; add them to a world".into())
        }
        // Without its project, a jar is most likely a mod.
        None if file_name.to_ascii_lowercase().ends_with(".jar") => Ok(ContentKind::Mod),
        _ => Err("Lodestone Launcher doesn't know where this kind of file goes".into()),
    }
}

/// Finds the pack's files on CurseForge (and, for files only CurseForge's website may
/// serve, on Modrinth) and makes the pack an importable [`PackInfo`].
pub async fn resolve(
    cf: &CurseForge,
    modrinth: Option<&Modrinth>,
    pack: &CfPack,
) -> Result<ResolvedPack, ModpackError> {
    let ids: Vec<u64> = pack.files.iter().map(|f| f.file_id).collect();
    let found: HashMap<u64, curseforge::CfFile> = cf
        .files_by_id(&ids)
        .await?
        .into_iter()
        .map(|f| (f.id, f))
        .collect();
    let mut project_ids: Vec<u64> = pack.files.iter().map(|f| f.project_id).collect();
    project_ids.sort_unstable();
    project_ids.dedup();
    let projects: HashMap<u64, curseforge::CfMod> = cf
        .projects(&project_ids)
        .await?
        .into_iter()
        .map(|p| (p.id, p))
        .collect();

    let mut skipped = pack.skipped.clone();
    let mut downloads: BTreeMap<PathBuf, PackDownload> = BTreeMap::new();
    let mut via_modrinth = Vec::new();
    let mut blocked = Vec::new();
    for entry in &pack.files {
        let Some(file) = found.get(&entry.file_id) else {
            skipped.push(SkippedPath {
                path: format!("CurseForge file {}", entry.file_id),
                reason: "CurseForge no longer has it".into(),
            });
            continue;
        };
        let project = projects.get(&file.mod_id);
        let kind = match folder_of(project.and_then(|p| p.class_id), &file.file_name) {
            Ok(kind) => kind,
            Err(reason) => {
                skipped.push(SkippedPath {
                    path: file.file_name.clone(),
                    reason,
                });
                continue;
            }
        };
        let Some(name) = safe_download_name(kind, &file.file_name) else {
            skipped.push(SkippedPath {
                path: file.file_name.clone(),
                reason: "its file name can't be used".into(),
            });
            continue;
        };
        if !entry.required && kind != ContentKind::Mod {
            skipped.push(SkippedPath {
                path: name,
                reason: "it is optional".into(),
            });
            continue;
        }
        // Optional mods come in disabled, for the user to turn on.
        let disk_name = if entry.required {
            name.clone()
        } else {
            crate::content::disabled_name(&name)
        };
        let path = PathBuf::from(kind.dir()).join(&disk_name);
        let Some(sha1) = file.sha1() else {
            skipped.push(SkippedPath {
                path: disk_name,
                reason: "CurseForge sent no SHA-1 to check it with".into(),
            });
            continue;
        };
        if file.downloadable() {
            let url = file
                .download_url
                .as_deref()
                .and_then(|u| url::Url::parse(u).ok())
                .ok_or_else(|| invalid(format!("{} has no usable download URL", file.file_name)))?;
            downloads.entry(path.clone()).or_insert(PackDownload {
                path,
                urls: vec![url],
                sha1,
                sha512: String::new(),
                size: file.file_length,
            });
        } else if entry.required {
            via_modrinth.push((path, kind, file.clone(), project.cloned(), sha1));
        } else {
            skipped.push(SkippedPath {
                path: disk_name,
                reason: "it is optional and only on CurseForge's website".into(),
            });
        }
    }

    // The same bytes on Modrinth, for files CurseForge's website keeps to itself.
    let hashes: Vec<String> = via_modrinth.iter().map(|(.., sha1)| sha1.clone()).collect();
    let on_modrinth = match modrinth {
        Some(modrinth) if !hashes.is_empty() => {
            modrinth.versions_by_sha1(&hashes).await.unwrap_or_default()
        }
        _ => HashMap::new(),
    };
    for (path, kind, file, project, sha1) in via_modrinth {
        let same = on_modrinth.get(&sha1).and_then(|version| {
            version.files.iter().find(|f| {
                f.hashes
                    .sha1
                    .as_deref()
                    .is_some_and(|h| h.eq_ignore_ascii_case(&sha1))
            })
        });
        match same.and_then(|f| {
            Some((
                url::Url::parse(&f.url).ok()?,
                f.hashes.sha512.clone()?,
                f.size,
            ))
        }) {
            Some((url, sha512, size)) => {
                downloads.entry(path.clone()).or_insert(PackDownload {
                    path,
                    urls: vec![url],
                    sha1,
                    sha512,
                    size,
                });
            }
            None => blocked.push(CfPlannedFile {
                kind,
                file,
                project,
                on_modrinth: None,
                depth: 0,
            }),
        }
    }

    Ok(ResolvedPack {
        info: PackInfo {
            name: pack.name.clone(),
            version_number: pack.version.clone(),
            summary: pack.author.clone(),
            game_version: pack.game_version.clone(),
            loader: pack.loader.clone(),
            downloads: downloads.into_values().collect(),
            overrides: pack.overrides.clone(),
            skipped,
        },
        blocked,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_loader_ids() {
        assert_eq!(
            manifest_loader("forge-47.2.0", "1.20.1").unwrap(),
            (LoaderKind::Forge, "47.2.0".into())
        );
        assert_eq!(
            manifest_loader("fabric-0.15.11", "1.20.1").unwrap(),
            (LoaderKind::Fabric, "0.15.11".into())
        );
        assert_eq!(
            manifest_loader("neoforge-21.1.77", "1.21.1").unwrap(),
            (LoaderKind::NeoForge, "21.1.77".into())
        );
        assert_eq!(
            manifest_loader("neoforge-1.20.1-47.1.84", "1.20.1").unwrap(),
            (LoaderKind::NeoForge, "47.1.84".into())
        );
        assert!(manifest_loader("liteloader-1.12", "1.12.2").is_err());
        assert!(manifest_loader("forge-recommended", "1.20.1").is_err());
        assert!(manifest_loader("forge", "1.20.1").is_err());
    }

    #[test]
    fn files_go_where_their_class_says() {
        assert_eq!(folder_of(Some(6), "a.jar"), Ok(ContentKind::Mod));
        assert_eq!(folder_of(Some(12), "a.zip"), Ok(ContentKind::ResourcePack));
        assert_eq!(folder_of(Some(6552), "a.zip"), Ok(ContentKind::ShaderPack));
        assert!(folder_of(Some(17), "w.zip").is_err());
        assert_eq!(folder_of(None, "a.jar"), Ok(ContentKind::Mod));
        assert!(folder_of(None, "a.zip").is_err());
    }
}
