//! What is in a content folder, with the metadata that is cheap to read: a mod's
//! metadata file and icon, a resource pack's `pack.mcmeta` and `pack.png`, and whether a
//! shader pack has its `shaders/` folder where Iris and OptiFine look for it.
//!
//! Archives are opened with zip 0.6.6 and only small, named entries are read; nothing is
//! extracted.

use std::{
    fs::{self, File},
    io::{Read, Seek},
    path::Path,
    sync::Arc,
    time::SystemTime,
};

use lodestone_minecraft::jar;

use super::{
    ContentError, ContentKind, InstanceDirs, enabled_name, io_error, is_content_name,
    is_disabled_name, packs,
};

/// Metadata files and icons bigger than this are not read.
const MAX_ENTRY_BYTES: u64 = 1024 * 1024;
/// Icons are shown this big at most, in pixels.
pub const ICON_PIXELS: u32 = 64;

/// The mod loaders a jar has a metadata file for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModFormat {
    /// `fabric.mod.json`.
    Fabric,
    /// `quilt.mod.json`.
    Quilt,
    /// `META-INF/mods.toml` or `mcmod.info`.
    Forge,
    /// `META-INF/neoforge.mods.toml`.
    NeoForge,
}

/// What a file says about itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileMeta {
    /// A mod's id (`iris`, `sodium`).
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
    pub icon: Option<Arc<image::RgbaImage>>,
    /// The loaders a mod jar has metadata for.
    pub mod_formats: Vec<ModFormat>,
    /// Whether the jar is OptiFine (it has OptiFine's classes).
    pub optifine: bool,
    /// A resource pack's formats.
    pub formats: Option<packs::FormatRange>,
    /// Why the game may not load the file, such as a resource pack without `pack.mcmeta`.
    pub problem: Option<String>,
}

/// A file or folder in a content folder.
#[derive(Clone, Debug, PartialEq)]
pub struct ScannedFile {
    pub kind: ContentKind,
    /// The name on disk, with `.disabled` for disabled mods.
    pub file_name: String,
    pub is_dir: bool,
    /// Bytes, for files.
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub meta: FileMeta,
}

impl ScannedFile {
    /// A mod without `.disabled`; packs are enabled or not in the game's options instead.
    pub fn enabled_by_name(&self) -> bool {
        !(self.kind == ContentKind::Mod && is_disabled_name(&self.file_name))
    }

    /// The name with `.disabled` removed.
    pub fn enabled_name(&self) -> String {
        enabled_name(&self.file_name)
    }

    /// The name to show: the metadata's, else the file name without its extension.
    pub fn display_name(&self) -> String {
        if let Some(name) = &self.meta.name
            && !name.is_empty()
        {
            return name.clone();
        }
        let name = self.enabled_name();
        let lower = name.to_ascii_lowercase();
        for ext in [".jar", ".zip"] {
            if lower.ends_with(ext) && !self.is_dir {
                return name[..name.len() - ext.len()].to_string();
            }
        }
        name
    }
}

/// Lists the content folder of `kind`, sorted by file name ignoring case. A missing folder
/// is empty. Hidden files (Lodestone's temporary downloads among them), symbolic links
/// and names that aren't content are skipped.
///
/// Reading a file's metadata opens it (a jar's index, its mod file and icon), which is
/// most of the time a large folder takes. So it is read on several threads, and kept
/// in [`META_CACHE`] by path, size and modification time: listing the folder again, or
/// after a mod was turned on or off (a rename keeps both), reads only files that changed.
pub fn scan(dirs: &InstanceDirs, kind: ContentKind) -> Result<Vec<ScannedFile>, ContentError> {
    let dir = dirs.content_dir(kind);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(io_error(&dir)(err)),
    };
    // The files, without their metadata yet.
    let mut found = Vec::new();
    for entry in entries {
        let entry = entry.map_err(io_error(&dir))?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() || !(file_type.is_file() || file_type.is_dir()) {
            continue;
        }
        let is_dir = file_type.is_dir();
        if (is_dir && !kind.allows_folders()) || !is_content_name(kind, &name, is_dir) {
            continue;
        }
        let metadata = entry.metadata().ok();
        found.push(ScannedFile {
            kind,
            size: if is_dir {
                0
            } else {
                metadata.as_ref().map_or(0, |m| m.len())
            },
            modified: metadata.and_then(|m| m.modified().ok()),
            meta: FileMeta::default(),
            file_name: name,
            is_dir,
        });
    }
    read_metas(&dir, &mut found);
    // By name as shown, so a pack named "§6Gold" sorts under G.
    found.sort_by_cached_key(|file| {
        (
            crate::mc_text::strip(&file.file_name).to_lowercase(),
            file.file_name.clone(),
        )
    });
    Ok(found)
}

/// What a file's metadata is cached under: its path (a mod's without `.disabled`), its
/// size and its modification time. Folders aren't cached: their files can change
/// without the folder's time changing.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct MetaKey {
    kind: ContentKind,
    path: std::path::PathBuf,
    size: u64,
    modified: SystemTime,
}

/// Files' metadata by [`MetaKey`], for this process. Bounded: emptied when it grows
/// past [`META_CACHE_LIMIT`] entries.
static META_CACHE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<MetaKey, FileMeta>>,
> = std::sync::LazyLock::new(Default::default);
const META_CACHE_LIMIT: usize = 8192;

fn meta_key(dir: &Path, file: &ScannedFile) -> Option<MetaKey> {
    if file.is_dir {
        return None;
    }
    Some(MetaKey {
        kind: file.kind,
        path: dir.join(enabled_name(&file.file_name)),
        size: file.size,
        modified: file.modified?,
    })
}

/// Fills in each file's metadata: from the cache, else read on up to one thread per
/// core.
fn read_metas(dir: &Path, files: &mut [ScannedFile]) {
    let mut missing: Vec<usize> = Vec::new();
    {
        let cache = META_CACHE.lock().unwrap_or_else(|e| e.into_inner());
        for (index, file) in files.iter_mut().enumerate() {
            match meta_key(dir, file).and_then(|key| cache.get(&key)) {
                Some(meta) => file.meta = meta.clone(),
                None => missing.push(index),
            }
        }
    }
    if missing.is_empty() {
        return;
    }
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(missing.len())
        .max(1);
    let read: Vec<(usize, FileMeta)> = std::thread::scope(|scope| {
        let files = &*files;
        let handles: Vec<_> = missing
            .chunks(missing.len().div_ceil(threads))
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|&index| {
                            let file = &files[index];
                            let path = dir.join(&file.file_name);
                            (index, read_meta(file.kind, &path, file.is_dir))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
            .collect()
    });
    let mut cache = META_CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if cache.len() + read.len() > META_CACHE_LIMIT {
        cache.clear();
    }
    for (index, meta) in read {
        if let Some(key) = meta_key(dir, &files[index]) {
            cache.insert(key, meta.clone());
        }
        files[index].meta = meta;
    }
}

/// Reads the metadata of the file or folder at `path` as `kind`.
pub fn read_meta(kind: ContentKind, path: &Path, is_dir: bool) -> FileMeta {
    if is_dir {
        return match kind {
            ContentKind::Mod => FileMeta::default(),
            ContentKind::ResourcePack => folder_pack_meta(path),
            ContentKind::ShaderPack => folder_shader_meta(path),
        };
    }
    // Buffered: reading a jar's index makes a few small reads per entry, and a big
    // mod has tens of thousands of entries.
    let archive = File::open(path)
        .ok()
        .and_then(|file| zip::ZipArchive::new(std::io::BufReader::new(file)).ok());
    let Some(mut archive) = archive else {
        return FileMeta {
            problem: Some(match kind {
                ContentKind::Mod => "This isn't a valid jar file.".to_string(),
                _ => "This isn't a valid zip file.".to_string(),
            }),
            ..FileMeta::default()
        };
    };
    match kind {
        ContentKind::Mod => mod_meta(&mut archive),
        ContentKind::ResourcePack => zip_pack_meta(&mut archive),
        ContentKind::ShaderPack => zip_shader_meta(&mut archive),
    }
}

fn read_entry<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, name: &str) -> Option<Vec<u8>> {
    let entry = archive.by_name(name).ok()?;
    if !entry.is_file() || entry.size() > MAX_ENTRY_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    entry.take(MAX_ENTRY_BYTES).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn read_text<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, name: &str) -> Option<String> {
    let bytes = read_entry(archive, name)?;
    Some(
        String::from_utf8_lossy(&bytes)
            .trim_start_matches('\u{feff}')
            .to_string(),
    )
}

/// Decodes a PNG or JPEG and makes it at most [`ICON_PIXELS`] square.
pub fn decode_icon(bytes: &[u8]) -> Option<Arc<image::RgbaImage>> {
    let image = image::load_from_memory(bytes).ok()?;
    let image = if image.width() > ICON_PIXELS || image.height() > ICON_PIXELS {
        image.thumbnail(ICON_PIXELS, ICON_PIXELS)
    } else {
        image
    };
    Some(Arc::new(image.to_rgba8()))
}

const MOD_METADATA: [(&str, ModFormat); 5] = [
    ("fabric.mod.json", ModFormat::Fabric),
    ("quilt.mod.json", ModFormat::Quilt),
    ("META-INF/neoforge.mods.toml", ModFormat::NeoForge),
    ("META-INF/mods.toml", ModFormat::Forge),
    ("mcmod.info", ModFormat::Forge),
];

fn mod_meta<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> FileMeta {
    let names: Vec<String> = archive.file_names().map(str::to_string).collect();
    let mut meta = FileMeta {
        optifine: names
            .iter()
            .any(|n| n.starts_with("net/optifine/") || n.starts_with("optifine/")),
        ..FileMeta::default()
    };
    for (file, format) in MOD_METADATA {
        if !names.iter().any(|n| n == file) {
            continue;
        }
        if !meta.mod_formats.contains(&format) {
            meta.mod_formats.push(format);
        }
        if meta.name.is_some() || meta.id.is_some() {
            continue;
        }
        let Some(text) = read_text(archive, file) else {
            continue;
        };
        let parsed = match file {
            "fabric.mod.json" => jar::parse_fabric_mod_json(&text),
            "quilt.mod.json" => jar::parse_quilt_mod_json(&text),
            "mcmod.info" => jar::parse_mcmod_info(&text),
            _ => {
                let manifest = read_text(archive, "META-INF/MANIFEST.MF")
                    .and_then(|m| jar::manifest_attribute(&m, "Implementation-Version"));
                jar::parse_mods_toml(&text, manifest)
            }
        };
        let Some(parsed) = parsed else {
            continue;
        };
        meta.id = parsed.id;
        meta.name = parsed.name;
        meta.version = parsed.version;
        meta.description = parsed.description;
        meta.authors = parsed.authors;
        meta.icon = jar::metadata_icon(file, &text)
            .and_then(|icon| read_entry(archive, &icon))
            .and_then(|bytes| decode_icon(&bytes));
    }
    if meta.mod_formats.is_empty() && !meta.optifine {
        meta.problem = Some("This jar has no mod metadata, so no loader will load it.".into());
    }
    meta
}

fn pack_meta_from(text: Option<String>, icon: Option<Vec<u8>>) -> FileMeta {
    let Some(text) = text else {
        return FileMeta {
            problem: Some("It has no pack.mcmeta, so the game won't list it.".into()),
            ..FileMeta::default()
        };
    };
    match packs::parse_pack_mcmeta(&text) {
        Some(pack) => FileMeta {
            description: (!pack.description.is_empty()).then_some(pack.description),
            formats: pack.formats,
            icon: icon.and_then(|bytes| decode_icon(&bytes)),
            ..FileMeta::default()
        },
        None => FileMeta {
            problem: Some("Its pack.mcmeta can't be read.".into()),
            icon: icon.and_then(|bytes| decode_icon(&bytes)),
            ..FileMeta::default()
        },
    }
}

fn zip_pack_meta<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> FileMeta {
    let text = read_text(archive, "pack.mcmeta");
    if text.is_none() && archive.file_names().any(|n| n.ends_with("/pack.mcmeta")) {
        return FileMeta {
            problem: Some(
                "Its pack.mcmeta is inside a folder in the zip, so the game won't list it.".into(),
            ),
            ..FileMeta::default()
        };
    }
    let icon = read_entry(archive, "pack.png");
    pack_meta_from(text, icon)
}

fn read_small_file(path: &Path) -> Option<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_ENTRY_BYTES {
        return None;
    }
    fs::read(path).ok()
}

fn folder_pack_meta(dir: &Path) -> FileMeta {
    let text = read_small_file(&dir.join("pack.mcmeta"))
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    pack_meta_from(text, read_small_file(&dir.join("pack.png")))
}

/// Iris and OptiFine find `shaders/` at the root of a pack or one folder down.
fn has_shaders_folder(names: impl Iterator<Item = String>) -> bool {
    names.into_iter().any(|name| {
        let parts: Vec<&str> = name.split('/').collect();
        matches!(parts.as_slice(), ["shaders", _, ..] | [_, "shaders", _, ..])
    })
}

fn no_shaders() -> FileMeta {
    FileMeta {
        problem: Some("It has no shaders folder, so it isn't a shader pack.".into()),
        ..FileMeta::default()
    }
}

fn zip_shader_meta<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> FileMeta {
    if has_shaders_folder(archive.file_names().map(str::to_string)) {
        FileMeta::default()
    } else {
        no_shaders()
    }
}

fn folder_shader_meta(dir: &Path) -> FileMeta {
    let is_dir = |path: &Path| fs::symlink_metadata(path).is_ok_and(|m| m.is_dir());
    if is_dir(&dir.join("shaders")) {
        return FileMeta::default();
    }
    let nested = fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            entry.file_type().is_ok_and(|t| t.is_dir()) && is_dir(&entry.path().join("shaders"))
        });
    if nested {
        FileMeta::default()
    } else {
        no_shaders()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Write as _;

    use super::*;

    /// Writes a zip with `entries` (name, bytes) to `path`.
    pub(crate) fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::FileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }

    pub(crate) fn png(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([10, 20, 30, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    #[test]
    fn lists_mods_with_metadata_and_icons() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = InstanceDirs::new(dir.path());
        let mods = dirs.content_dir(ContentKind::Mod);
        let icon = png(128, 128);
        write_zip(
            &mods.join("sodium.jar"),
            &[
                (
                    "fabric.mod.json",
                    br#"{"id": "sodium", "name": "Sodium", "version": "0.5.13", "authors": ["JellySquid"], "icon": "assets/sodium/icon.png"}"#,
                ),
                ("assets/sodium/icon.png", &icon),
            ],
        );
        write_zip(
            &mods.join("Forge Mod.jar.disabled"),
            &[
                (
                    "META-INF/mods.toml",
                    b"modLoader=\"javafml\"\n[[mods]]\nmodId=\"jei\"\ndisplayName=\"JEI\"\nversion=\"${file.jarVersion}\"\n",
                ),
                ("META-INF/MANIFEST.MF", b"Implementation-Version: 15.2.0\n"),
            ],
        );
        write_zip(
            &mods.join("optifine-hd.jar"),
            &[("net/optifine/Config.class", b"x")],
        );
        write_zip(&mods.join("empty.jar"), &[("readme.txt", b"x")]);
        fs::write(mods.join("broken.jar"), b"not a zip").unwrap();
        fs::write(mods.join(".lodestone-download.part"), b"x").unwrap();
        fs::write(mods.join("notes.txt"), b"x").unwrap();
        fs::create_dir_all(mods.join("folder.jar")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(mods.join("sodium.jar"), mods.join("link.jar")).unwrap();

        let files = scan(&dirs, ContentKind::Mod).unwrap();
        let names: Vec<&str> = files.iter().map(|f| f.file_name.as_str()).collect();
        assert_eq!(
            names,
            [
                "broken.jar",
                "empty.jar",
                "Forge Mod.jar.disabled",
                "optifine-hd.jar",
                "sodium.jar"
            ]
        );
        let sodium = &files[4];
        assert_eq!(sodium.meta.id.as_deref(), Some("sodium"));
        assert_eq!(sodium.display_name(), "Sodium");
        assert_eq!(sodium.meta.version.as_deref(), Some("0.5.13"));
        assert_eq!(sodium.meta.mod_formats, [ModFormat::Fabric]);
        let icon = sodium.meta.icon.as_ref().unwrap();
        assert_eq!((icon.width(), icon.height()), (64, 64));
        assert!(sodium.enabled_by_name());
        let jei = &files[2];
        assert!(!jei.enabled_by_name());
        assert_eq!(jei.meta.version.as_deref(), Some("15.2.0"));
        assert_eq!(jei.meta.mod_formats, [ModFormat::Forge]);
        assert!(files[3].meta.optifine);
        assert!(files[3].meta.problem.is_none());
        assert!(
            files[1]
                .meta
                .problem
                .as_deref()
                .unwrap()
                .contains("no mod metadata")
        );
        assert!(
            files[0]
                .meta
                .problem
                .as_deref()
                .unwrap()
                .contains("valid jar")
        );
        assert_eq!(files[1].display_name(), "empty");
    }

    #[test]
    fn lists_resource_packs_as_zips_and_folders() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = InstanceDirs::new(dir.path());
        let packs_dir = dirs.content_dir(ContentKind::ResourcePack);
        write_zip(
            &packs_dir.join("Faithful.zip"),
            &[
                (
                    "pack.mcmeta",
                    r#"{"pack": {"pack_format": 15, "description": "§aFaithful §r32x"}}"#
                        .as_bytes(),
                ),
                ("pack.png", &png(16, 16)),
            ],
        );
        write_zip(
            &packs_dir.join("Nested.zip"),
            &[(
                "Nested/pack.mcmeta",
                br#"{"pack": {"pack_format": 15, "description": ""}}"#,
            )],
        );
        write_zip(&packs_dir.join("Nothing.zip"), &[("assets/x.png", b"x")]);
        fs::create_dir_all(packs_dir.join("Folder pack/assets")).unwrap();
        fs::write(
            packs_dir.join("Folder pack/pack.mcmeta"),
            br#"{"pack": {"pack_format": 34, "supported_formats": [34, 42], "description": {"text": "Folder"}}}"#,
        )
        .unwrap();
        let files = scan(&dirs, ContentKind::ResourcePack).unwrap();
        let names: Vec<&str> = files.iter().map(|f| f.file_name.as_str()).collect();
        assert_eq!(
            names,
            ["Faithful.zip", "Folder pack", "Nested.zip", "Nothing.zip"]
        );
        assert_eq!(
            files[0].meta.description.as_deref().map(crate::mc_text::strip),
            Some("Faithful 32x".to_string())
        );
        assert_eq!(files[0].meta.formats.unwrap().min.major, 15);
        assert!(files[0].meta.icon.is_some());
        assert_eq!(files[0].display_name(), "Faithful");
        assert!(files[1].is_dir);
        assert_eq!(files[1].display_name(), "Folder pack");
        assert_eq!(files[1].meta.formats.unwrap().max.major, 42);
        assert!(
            files[2]
                .meta
                .problem
                .as_deref()
                .unwrap()
                .contains("inside a folder")
        );
        assert!(
            files[3]
                .meta
                .problem
                .as_deref()
                .unwrap()
                .contains("no pack.mcmeta")
        );
    }

    #[test]
    fn lists_shader_packs() {
        let dir = tempfile::tempdir().unwrap();
        let dirs = InstanceDirs::new(dir.path());
        let shaders = dirs.content_dir(ContentKind::ShaderPack);
        write_zip(&shaders.join("BSL.zip"), &[("shaders/composite.fsh", b"x")]);
        write_zip(
            &shaders.join("Nested.zip"),
            &[("Nested-master/shaders/final.fsh", b"x")],
        );
        write_zip(
            &shaders.join("Deep.zip"),
            &[("a/b/shaders/final.fsh", b"x")],
        );
        fs::create_dir_all(shaders.join("Folder/shaders")).unwrap();
        fs::write(shaders.join("Folder/shaders/final.fsh"), b"x").unwrap();
        fs::write(shaders.join("BSL.zip.txt"), b"options").unwrap();
        let files = scan(&dirs, ContentKind::ShaderPack).unwrap();
        let names: Vec<&str> = files.iter().map(|f| f.file_name.as_str()).collect();
        assert_eq!(names, ["BSL.zip", "Deep.zip", "Folder", "Nested.zip"]);
        assert!(files[0].meta.problem.is_none());
        assert!(files[1].meta.problem.is_some());
        assert!(files[2].meta.problem.is_none());
        assert!(files[3].meta.problem.is_none());
    }
}
