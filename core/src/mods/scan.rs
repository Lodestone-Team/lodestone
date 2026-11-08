//! Scans a mods folder: the jars in it, their SHA-512 hashes and their metadata.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use color_eyre::eyre::{eyre, Context};
use sha2::Digest;

use super::{
    is_disabled_name, is_mod_file_name, metadata, InstalledModLoader, ModFileMetadata, ModFolder,
};
use crate::error::Error;

/// A mod file on disk.
#[derive(Debug, Clone, PartialEq)]
pub struct ScannedFile {
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
    pub sha512: String,
    pub metadata: Option<ModFileMetadata>,
}

#[derive(Clone)]
struct CachedScan {
    size: u64,
    modified: Option<SystemTime>,
    sha512: String,
    metadata: Option<ModFileMetadata>,
}

/// Hashing every jar on each listing is slow for large folders, so results are kept for
/// files whose size and modification time did not change.
static SCAN_CACHE: LazyLock<Mutex<HashMap<(PathBuf, InstalledModLoader), CachedScan>>> =
    LazyLock::new(Default::default);
const SCAN_CACHE_MAX_ENTRIES: usize = 4096;

/// The SHA-512 of a file, as lowercase hex.
pub fn sha512_of(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha512::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn scan_file_blocking(path: &Path, loader: InstalledModLoader) -> Result<ScannedFile, Error> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| eyre!("{} has no valid file name", path.display()))?
        .to_string();
    let info =
        std::fs::symlink_metadata(path).with_context(|| format!("Failed to read {file_name}"))?;
    let size = info.len();
    let modified = info.modified().ok();
    let key = (path.to_path_buf(), loader);
    let cached = SCAN_CACHE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&key)
        .filter(|c| c.size == size && c.modified == modified && modified.is_some())
        .cloned();
    let scan = match cached {
        Some(scan) => scan,
        None => {
            let scan = CachedScan {
                size,
                modified,
                sha512: sha512_of(path).with_context(|| format!("Failed to read {file_name}"))?,
                metadata: metadata::read_jar_metadata(path, loader),
            };
            let mut cache = SCAN_CACHE.lock().unwrap_or_else(|e| e.into_inner());
            if cache.len() >= SCAN_CACHE_MAX_ENTRIES {
                cache.clear();
            }
            cache.insert(key, scan.clone());
            scan
        }
    };
    Ok(ScannedFile {
        enabled: !is_disabled_name(&file_name),
        file_name,
        size: scan.size,
        sha512: scan.sha512,
        metadata: scan.metadata,
    })
}

/// Scans one file.
pub async fn scan_file(path: &Path, loader: InstalledModLoader) -> Result<ScannedFile, Error> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || scan_file_blocking(&path, loader))
        .await
        .map_err(|e| eyre!("The scan task failed: {e}"))?
}

/// The regular files ending in `.jar` or `.jar.disabled` in the folder. Symbolic links and
/// other files are ignored, and a missing folder is empty.
pub async fn scan_folder(folder: &ModFolder) -> Result<Vec<ScannedFile>, Error> {
    let dir = folder.dir.clone();
    let loader = folder.loader;
    tokio::task::spawn_blocking(move || -> Result<Vec<ScannedFile>, Error> {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e).context("Failed to read the mods folder")?,
        };
        let mut files = Vec::new();
        for entry in entries {
            let entry = entry.context("Failed to read the mods folder")?;
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if !file_type.is_file() || name.starts_with('.') || !is_mod_file_name(name) {
                continue;
            }
            match scan_file_blocking(&entry.path(), loader) {
                Ok(file) => files.push(file),
                // a file removed while scanning is simply not listed
                Err(e) => tracing::warn!("Skipping {name}: {}", e.source),
            }
        }
        Ok(files)
    })
    .await
    .map_err(|e| eyre!("The scan task failed: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mods::metadata::tests::make_zip;

    #[tokio::test]
    async fn lists_only_regular_jar_files() {
        let dir = tempfile::tempdir().unwrap();
        let folder = ModFolder {
            loader: InstalledModLoader::Fabric,
            game_version: "1.20.1".into(),
            dir: dir.path().join("mods"),
        };
        // a missing folder is empty
        assert!(scan_folder(&folder).await.unwrap().is_empty());

        std::fs::create_dir(&folder.dir).unwrap();
        make_zip(
            &folder.dir.join("a.jar"),
            &[("fabric.mod.json", r#"{"id":"a","name":"A"}"#)],
        );
        std::fs::write(folder.dir.join("b.jar.disabled"), "not a zip").unwrap();
        std::fs::write(folder.dir.join("readme.txt"), "x").unwrap();
        std::fs::write(folder.dir.join("c.jar.bak"), "x").unwrap();
        std::fs::write(folder.dir.join(".lodestone-download-1.tmp"), "x").unwrap();
        std::fs::create_dir(folder.dir.join("folder.jar")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(folder.dir.join("a.jar"), folder.dir.join("link.jar")).unwrap();

        let mut files = scan_folder(&folder).await.unwrap();
        files.sort_by(|a, b| a.file_name.cmp(&b.file_name));
        let names: Vec<&str> = files.iter().map(|f| f.file_name.as_str()).collect();
        assert_eq!(names, vec!["a.jar", "b.jar.disabled"]);
        assert!(files[0].enabled);
        assert!(!files[1].enabled);
        assert_eq!(
            files[0].metadata.as_ref().unwrap().name.as_deref(),
            Some("A")
        );
        assert_eq!(files[1].metadata, None);
        assert_eq!(files[1].size, 9);
        assert_eq!(
            files[1].sha512,
            hex::encode(sha2::Sha512::digest(b"not a zip"))
        );
    }
}
