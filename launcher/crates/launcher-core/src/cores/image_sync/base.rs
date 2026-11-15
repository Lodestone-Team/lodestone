//! A view's base pack: the `.mrpack` a `modrinth_pack` base names, whose client share a
//! player installs under the view's entries ("What a view places").
//!
//! A server can name any pack as its base, so the Launcher takes it only from Modrinth: the
//! version must be the named project's, Modrinth's listing of its `.mrpack` must have the
//! SHA-512 the base names, and the file is downloaded from Modrinth's CDN with that
//! SHA-512 checked. Packs are kept in `<data>/cache/server-packs/<sha512>.mrpack` (the
//! newest few, and any used in the last day), hashed again each time one is used, so a
//! player whose server updates its pack, or who makes a second instance, downloads it
//! once.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use lodestone_image::{Base, hex::is_sha512};
use lodestone_minecraft::mrpack::Side;

use super::SyncError;
use crate::{
    content::{Modrinth, ModrinthError, install::InstallProgress},
    game::download::{CancelToken, Downloader},
    modpack::{self, ModpackError, OverrideFile, PackInfo, import},
};

/// How many base packs the cache keeps (besides the ones used lately).
pub const CACHED_PACKS: usize = 8;
/// A pack used within this long is kept whatever [`CACHED_PACKS`] says: an install
/// prompt that waits for the player holds the pack its update copies from
/// ([`BasePack::path`]), and pruning after another server's install mustn't take it.
pub const RECENTLY_USED: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// A base pack that was found or downloaded, checked and read.
#[derive(Clone, Debug)]
pub struct BasePack {
    /// The `.mrpack` in the cache.
    pub path: PathBuf,
    pub pack: PackInfo,
    pub project: String,
    pub version: String,
    pub sha512: String,
}

impl BasePack {
    /// The pack's files at `paths` in its server share (`overrides/` with
    /// `server-overrides/` over them), read and hashed: where the core takes a `base`
    /// entry from when the client share has another copy at its path
    /// ([`super::placed::BaseShare`]). Only those files are read.
    pub fn server_copies(&self, paths: &BTreeSet<String>) -> Result<Vec<OverrideFile>, SyncError> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }
        let mut archive = modpack::open_pack(&self.path)?;
        Ok(modpack::read_overrides_of(
            &mut archive,
            &Side::Server.override_dirs(),
            Some(paths),
            &mut Vec::new(),
        )?)
    }
}

/// The base pack of a view whose base is `base`, from the cache in `cache` or Modrinth.
/// `None` for a view without one (a `loader` base, or none).
pub async fn base_pack(
    api: Option<&Modrinth>,
    downloader: &Downloader,
    cache: &Path,
    base: Option<&Base>,
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<Option<BasePack>, SyncError> {
    let (project, version, sha512) = match base {
        None | Some(Base::Loader {}) => return Ok(None),
        Some(Base::CurseforgePack { .. }) => return Err(SyncError::CurseForgeBase),
        Some(Base::ModrinthPack {
            project,
            version,
            sha512,
        }) => (project, version, sha512.to_ascii_lowercase()),
    };
    if !is_sha512(&sha512) {
        return Err(SyncError::Refused("its modpack has no valid hash".into()));
    }
    let found = |path: PathBuf| -> Result<BasePack, SyncError> {
        let pack = modpack::read_pack(&path)?;
        Ok(BasePack {
            path,
            pack,
            project: project.clone(),
            version: version.clone(),
            sha512: sha512.clone(),
        })
    };
    let cached = cache.join(format!("{sha512}.mrpack"));
    if cached.is_file() {
        match lodestone_minecraft::hash::sha512_of(&cached) {
            Ok(actual) if actual.eq_ignore_ascii_case(&sha512) => {
                touch(&cached);
                return found(cached).map(Some);
            }
            _ => {
                let _ = fs::remove_file(&cached);
            }
        }
    }
    let api = api.ok_or_else(|| {
        SyncError::Modpack(ModpackError::Invalid(
            "Modrinth can't be reached to download the server's modpack.".into(),
        ))
    })?;
    progress.set_label("Looking up the server's modpack");
    let listed = match api.version(version).await {
        Ok(listed) => listed,
        Err(ModrinthError::NotFound(_)) => {
            return Err(SyncError::BasePackGone {
                project: project.clone(),
                version: version.clone(),
            });
        }
        Err(err) => return Err(SyncError::Modpack(err.into())),
    };
    if listed.project_id != *project {
        return Err(SyncError::Refused(
            "the modpack version it names belongs to another project".into(),
        ));
    }
    let listed_sha512 = import::pack_file(&listed)
        .and_then(|file| file.hashes.sha512.as_deref())
        .map(str::to_ascii_lowercase);
    if listed_sha512.as_deref() != Some(sha512.as_str()) {
        return Err(SyncError::Refused(
            "its modpack isn't the file Modrinth has for that version".into(),
        ));
    }
    if cancel.is_cancelled() {
        return Err(SyncError::Cancelled);
    }
    // the download checks the SHA-512 Modrinth lists, which is the base's
    let downloaded = import::download_pack(api, downloader, &listed, cache, progress, cancel)
        .await
        .map_err(|err| match err {
            ModpackError::Cancelled => SyncError::Cancelled,
            err => SyncError::Modpack(err),
        })?;
    fs::rename(&downloaded, &cached).map_err(|err| {
        let _ = fs::remove_file(&downloaded);
        SyncError::io(cached.display(), err)
    })?;
    found(cached).map(Some)
}

/// Marks a cached pack as just used, so pruning keeps it.
fn touch(path: &Path) {
    if let Ok(file) = fs::File::options().write(true).open(path) {
        let _ = file.set_modified(std::time::SystemTime::now());
    }
}

/// Deletes all but the newest [`CACHED_PACKS`] packs in the cache, keeping any used
/// within [`RECENTLY_USED`] too (and deletes downloads a crash left there).
pub fn prune_cache(cache: &Path) {
    let Ok(entries) = fs::read_dir(cache) else {
        return;
    };
    let mut packs: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".lodestone-") {
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > std::time::Duration::from_secs(3600));
            if old {
                let _ = fs::remove_file(&path);
            }
            continue;
        }
        if let Some(modified) = name
            .strip_suffix(".mrpack")
            .filter(|sha| is_sha512(sha))
            .and_then(|_| entry.metadata().and_then(|m| m.modified()).ok())
        {
            packs.push((modified, path));
        }
    }
    packs.sort_by(|a, b| b.cmp(a));
    for (modified, path) in packs.into_iter().skip(CACHED_PACKS) {
        let recent = modified.elapsed().map_or(true, |age| age < RECENTLY_USED);
        if !recent {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;

    /// Past the newest [`CACHED_PACKS`], a pack used within the last day stays (an install
    /// prompt may still hold it) and an older one goes.
    #[test]
    fn pruning_keeps_the_newest_and_the_recently_used_packs() {
        let dir = tempfile::tempdir().unwrap();
        let pack = |i: u8, ago: Duration| {
            let path = dir
                .path()
                .join(format!("{}.mrpack", format!("{i:02x}").repeat(64)));
            fs::write(&path, b"pack").unwrap();
            fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(SystemTime::now() - ago)
                .unwrap();
            path
        };
        let recent: Vec<PathBuf> = (0..9u8)
            .map(|i| pack(i, Duration::from_secs(60 + u64::from(i))))
            .collect();
        let old = pack(9, Duration::from_secs(2 * 24 * 60 * 60));
        prune_cache(dir.path());
        assert!(recent.iter().all(|path| path.is_file()));
        assert!(!old.exists());
    }
}
