//! Where world backups live: `<lodestone>/world-backups/<uuid>/`, outside every server
//! folder and the image store.
//!
//! | File | What |
//! | --- | --- |
//! | `<id>.zip` | the world, a plain zip ([`super::writer`]) |
//! | `<id>.json` | its sidecar ([`WorldBackupRecord`]) |
//! | `.<id>.zip.part` | a zip being written; renamed to `<id>.zip` once whole |
//! | `.incoming-<16 hex>.tmp` | a sidecar being written ([`write_small_atomic`]) |
//!
//! A backup is written in this order: the part file, flushed; renamed to `<id>.zip`; the
//! folder flushed; then the sidecar, atomically. Deleting is the mirror image: the zip,
//! then the sidecar. So a crash leaves at worst a whole zip without a sidecar, which
//! [`WorldStore::list`] adopts as a manual backup, or a sidecar without a zip, which it
//! drops; never a half zip that is listed. Startup ([`WorldStore::sweep`]) deletes part
//! and temporary files a crash left.
//!
//! Everything here is blocking; the service holds the server's record lock around a list,
//! a final rename with its sidecar, and a delete.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use color_eyre::eyre::eyre;
use tracing::{info, warn};

use crate::backups::store::{check_backup_name, err};
use crate::error::{Error, ErrorKind};
use crate::images::{write_small_atomic, ImageService};
use crate::types::InstanceUuid;

use super::types::{WorldBackupKind, WorldBackupRecord, WORLD_RECORD_VERSION};

/// A sidecar is a few hundred bytes; anything bigger is damaged.
const MAX_RECORD_BYTES: u64 = 64 * 1024;
/// The most entries a server's folder may have before listing gives up, so a folder
/// someone filled outside Lodestone can't make every list read millions of names.
const MAX_FOLDER_ENTRIES: usize = 100_000;
/// The most top folders a zip adopted without its sidecar may have (the level and its two
/// split folders).
const MAX_ADOPTED_TOPS: usize = 3;

/// World backup ids, exactly as [`crate::backups::store::new_backup_id`] makes them
/// (`YYYYMMDD-HHMMSS-xxxx`): they name files, so nothing else is taken.
pub fn valid_world_backup_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    bytes.len() == 20
        && bytes.iter().enumerate().all(|(i, b)| match i {
            8 | 15 => *b == b'-',
            16..=19 => b.is_ascii_digit() || (b'a'..=b'f').contains(b),
            _ => b.is_ascii_digit(),
        })
}

/// The time an id names, in Unix seconds.
fn created_from_id(id: &str) -> Option<i64> {
    let time = chrono::NaiveDateTime::parse_from_str(id.get(..15)?, "%Y%m%d-%H%M%S").ok()?;
    Some(time.and_utc().timestamp())
}

pub fn not_found() -> Error {
    err(ErrorKind::NotFound, "World backup not found")
}

fn io_error(e: io::Error, what: impl Into<String>) -> Error {
    Error::from(eyre!(e).wrap_err(what.into()))
}

/// The world backups' folder (see the module). Cheap to clone.
#[derive(Debug, Clone)]
pub struct WorldStore {
    root: PathBuf,
    /// `<uuid>/<id>` of zips without a sidecar that couldn't be adopted, so each list
    /// doesn't read and log them again until the core restarts.
    not_adopted: Arc<Mutex<HashSet<String>>>,
}

impl WorldStore {
    /// The world backups under `root` (`<lodestone>/world-backups`).
    pub fn new(root: PathBuf) -> WorldStore {
        WorldStore {
            root,
            not_adopted: Default::default(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `<uuid>/`: a uuid images aren't kept for ([`ImageService::accepts`]: a Docker
    /// container, or one from a URL that isn't a uuid) is a bad request and never reaches
    /// the disk.
    pub fn dir(&self, uuid: &InstanceUuid) -> Result<PathBuf, Error> {
        if !ImageService::accepts(uuid) {
            return Err(err(ErrorKind::BadRequest, "Invalid server id"));
        }
        Ok(self.root.join(uuid.as_ref()))
    }

    /// `<id>.zip`. The id must be valid ([`valid_world_backup_id`]).
    pub fn zip_path(&self, uuid: &InstanceUuid, id: &str) -> Result<PathBuf, Error> {
        Ok(self.dir(uuid)?.join(format!("{}.zip", checked_id(id)?)))
    }

    pub fn record_path(&self, uuid: &InstanceUuid, id: &str) -> Result<PathBuf, Error> {
        Ok(self.dir(uuid)?.join(format!("{}.json", checked_id(id)?)))
    }

    /// `.<id>.zip.part`, where a zip is written until it is whole.
    pub fn part_path(&self, uuid: &InstanceUuid, id: &str) -> Result<PathBuf, Error> {
        Ok(self
            .dir(uuid)?
            .join(format!(".{}.zip.part", checked_id(id)?)))
    }

    /// Writes a backup's sidecar crash-safely.
    pub fn save_record(
        &self,
        uuid: &InstanceUuid,
        record: &WorldBackupRecord,
    ) -> Result<(), Error> {
        let json = serde_json::to_vec_pretty(record).map_err(|e| {
            err(
                ErrorKind::Internal,
                format!("Failed to write the world backup's record: {e}"),
            )
        })?;
        write_small_atomic(&self.record_path(uuid, &record.id)?, &json)
    }

    /// A backup's sidecar, checked: NotFound when its zip or sidecar is missing, or its zip
    /// isn't a plain file (a link isn't followed), as [`Self::list`] sees backups.
    pub fn load(&self, uuid: &InstanceUuid, id: &str) -> Result<WorldBackupRecord, Error> {
        if !valid_world_backup_id(id) {
            return Err(not_found());
        }
        let zip = self.zip_path(uuid, id)?;
        if !fs::symlink_metadata(&zip).is_ok_and(|m| m.file_type().is_file()) {
            return Err(not_found());
        }
        match read_record(&self.record_path(uuid, id)?, id)? {
            Some(record) => Ok(record),
            None => Err(not_found()),
        }
    }

    /// The server's world backups, newest first, brought in line with the zips first: a
    /// sidecar whose zip is gone is deleted (a crash in the middle of a delete), and a whole
    /// zip without a sidecar gets one (a crash between a zip's rename and its sidecar). A
    /// sidecar that can't be read is an error, never taken for no backups.
    pub fn list(&self, uuid: &InstanceUuid) -> Result<Vec<WorldBackupRecord>, Error> {
        let dir = self.dir(uuid)?;
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(io_error(e, "Failed to list the world backups")),
        };
        let mut zips = HashSet::new();
        let mut sidecars = HashSet::new();
        for (count, entry) in entries.enumerate() {
            if count >= MAX_FOLDER_ENTRIES {
                warn!("{} has too many files to list", dir.display());
                return Err(err(
                    ErrorKind::Internal,
                    "The server's world backups folder has too many files to list",
                ));
            }
            let entry = entry.map_err(|e| io_error(e, "Failed to list the world backups"))?;
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if let Some(id) = name
                .strip_suffix(".zip")
                .filter(|id| valid_world_backup_id(id))
            {
                if entry.file_type().is_ok_and(|t| t.is_file()) {
                    zips.insert(id.to_string());
                }
            } else if let Some(id) = name
                .strip_suffix(".json")
                .filter(|id| valid_world_backup_id(id))
            {
                sidecars.insert(id.to_string());
            }
        }
        let mut records = Vec::new();
        for id in &sidecars {
            if !zips.contains(id) {
                info!("Dropped the world backup {id} of {uuid}, whose zip is gone");
                let path = self.record_path(uuid, id)?;
                if let Err(e) = fs::remove_file(&path) {
                    warn!("Failed to delete {}: {e}", path.display());
                }
                continue;
            }
            // `None`: deleted meanwhile
            if let Some(record) = read_record(&self.record_path(uuid, id)?, id)? {
                records.push(record);
            }
        }
        for id in zips.iter().filter(|id| !sidecars.contains(*id)) {
            let key = format!("{uuid}/{id}");
            let mut not_adopted = self.not_adopted.lock().unwrap_or_else(|e| e.into_inner());
            if not_adopted.contains(&key) {
                continue;
            }
            match self.adopt(uuid, id) {
                Ok(record) => {
                    info!("Listed the world backup {id} of {uuid}, which had no record");
                    records.push(record);
                }
                Err(e) => {
                    warn!(
                        "The world backup {id} of {uuid} has no record and can't be read: {}",
                        e.source
                    );
                    not_adopted.insert(key);
                }
            }
        }
        records.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| b.id.cmp(&a.id)));
        Ok(records)
    }

    /// A sidecar for a whole zip that has none: a manual backup with no name, at the time
    /// its id names, with the level and folders its top folders say.
    fn adopt(&self, uuid: &InstanceUuid, id: &str) -> Result<WorldBackupRecord, Error> {
        let zip = self.zip_path(uuid, id)?;
        let size = fs::metadata(&zip)
            .map_err(|e| io_error(e, "Failed to read the world backup"))?
            .len();
        let TopFolders {
            folders,
            with_region,
            files: file_count,
        } = zip_top_folders(&zip)?;
        let level = adopted_level(&folders, &with_region)
            .ok_or_else(|| err(ErrorKind::Internal, "The zip has no world folder"))?;
        lodestone_image::ServerLayout::new(level.clone(), [], false).map_err(|e| {
            err(
                ErrorKind::Internal,
                format!("The zip's world folder isn't valid: {e}"),
            )
        })?;
        let record = WorldBackupRecord {
            version: WORLD_RECORD_VERSION,
            id: id.to_string(),
            name: None,
            kind: WorldBackupKind::Manual,
            created: created_from_id(id).unwrap_or_default(),
            level,
            folders,
            instance_name: String::new(),
            game_version: String::new(),
            flavour: String::new(),
            size,
            file_count,
            left_out: 0,
            live: false,
            quiesced: true,
            before_restoring: None,
        };
        self.save_record(uuid, &record)?;
        Ok(record)
    }

    /// Deletes a backup: its zip first, then its sidecar, so a crash in between never
    /// brings it back. A zip that can't be deleted (on Windows, while a download reads it)
    /// is a conflict and keeps the sidecar, so the backup stays listed.
    pub fn delete(&self, uuid: &InstanceUuid, id: &str) -> Result<(), Error> {
        if !valid_world_backup_id(id) {
            return Err(not_found());
        }
        let zip = self.zip_path(uuid, id)?;
        match fs::remove_file(&zip) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(not_found()),
            Err(e) => {
                warn!("Failed to delete {}: {e}", zip.display());
                return Err(err(
                    ErrorKind::Conflict,
                    "The world backup is in use (being downloaded?); try again in a moment",
                ));
            }
        }
        if let Some(dir) = zip.parent() {
            if let Err(e) = crate::backups::restore::sync_dir(dir) {
                warn!("Failed to flush {}: {e}", dir.display());
            }
        }
        let record = self.record_path(uuid, id)?;
        if let Err(e) = fs::remove_file(&record) {
            if e.kind() != io::ErrorKind::NotFound {
                // the next list drops it, as its zip is gone
                warn!("Failed to delete {}: {e}", record.display());
            }
        }
        Ok(())
    }

    /// When a server is deleted: every world backup of it.
    pub fn delete_server(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        let dir = self.dir(uuid)?;
        crate::images::remove_tree(&dir).map_err(|e| {
            warn!("Failed to delete {}: {e}", dir.display());
            io_error(e, "Failed to delete the server's world backups")
        })
    }

    /// At startup: deletes the part files and temporary sidecars a crash left in every
    /// server's folder.
    pub fn sweep(&self) {
        let Ok(servers) = fs::read_dir(&self.root) else {
            return;
        };
        for server in servers.flatten() {
            if !server.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let Ok(files) = fs::read_dir(server.path()) else {
                continue;
            };
            for file in files.flatten().take(MAX_FOLDER_ENTRIES) {
                let name = file.file_name();
                let Some(name) = name.to_str() else {
                    continue;
                };
                let leftover = (name.starts_with('.') && name.ends_with(".zip.part"))
                    || (name.starts_with(".incoming-") && name.ends_with(".tmp"));
                if leftover {
                    if let Err(e) = fs::remove_file(file.path()) {
                        warn!("Failed to delete {}: {e}", file.path().display());
                    }
                }
            }
        }
    }
}

fn checked_id(id: &str) -> Result<&str, Error> {
    if valid_world_backup_id(id) {
        Ok(id)
    } else {
        Err(not_found())
    }
}

/// The level of a zip adopted with these top folders: the one the others are the split
/// folders of, or, when the zip holds only a split server's `_nether` and `_the_end`
/// folders (a restore's safety backup of a world whose level was gone), the level they
/// name. A lone folder named like a split folder is one when it has no `region` folder of
/// its own (Paper keeps a split dimension's regions in `DIM-1` or `DIM1` inside it), and
/// otherwise a vanilla level that happens to be called that: taking a split Nether as the
/// level would put its `level.dat` over the overworld's on restore.
fn adopted_level(folders: &[String], with_region: &BTreeSet<String>) -> Option<String> {
    let split_of = |top: &str| {
        top.strip_suffix("_nether")
            .or_else(|| top.strip_suffix("_the_end"))
            .filter(|prefix| !prefix.is_empty())
            .map(str::to_string)
    };
    let level = folders.iter().find(|top| {
        !folders
            .iter()
            .any(|other| split_of(top).as_deref() == Some(other.as_str()))
    })?;
    if let Some(prefix) = split_of(level) {
        let all_split = folders
            .iter()
            .all(|top| split_of(top).as_deref() == Some(&prefix));
        let lone_split = folders.len() == 1 && !with_region.contains(level);
        if all_split && (folders.len() > 1 || lone_split) {
            return Some(prefix);
        }
    }
    Some(level.clone())
}

/// Reads and checks the sidecar at `path` for `id`: `None` when it is missing, an error
/// when it can't be read or isn't valid. The error names the backup, not the path (it
/// reaches clients); the log has the path.
fn read_record(path: &Path, id: &str) -> Result<Option<WorldBackupRecord>, Error> {
    let damaged = |why: String| {
        warn!(
            "The world backup record {} can't be read: {why}",
            path.display()
        );
        err(
            ErrorKind::Internal,
            format!("The record of the world backup {id} can't be read: {why}"),
        )
    };
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(damaged(e.to_string())),
    };
    let mut bytes = Vec::new();
    file.take(MAX_RECORD_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| damaged(e.to_string()))?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err(damaged("it is too large".to_string()));
    }
    let record: WorldBackupRecord =
        serde_json::from_slice(&bytes).map_err(|e| damaged(e.to_string()))?;
    if record.version != WORLD_RECORD_VERSION {
        return Err(damaged(format!("version {} isn't known", record.version)));
    }
    if record.id != id {
        return Err(damaged("it names another backup".to_string()));
    }
    if let Some(name) = &record.name {
        if check_backup_name(name) != Ok(Some(name.clone())) {
            return Err(damaged("its name isn't valid".to_string()));
        }
    }
    if lodestone_image::ServerLayout::new(record.level.clone(), [], false).is_err() {
        return Err(damaged("its level isn't a folder name".to_string()));
    }
    Ok(Some(record))
}

/// The top folders of the zip at `path`, sorted, and its number of files. Reads only its
/// directory; at most [`MAX_ADOPTED_TOPS`] tops.
fn zip_top_folders(path: &Path) -> Result<TopFolders, Error> {
    let file = fs::File::open(path).map_err(|e| io_error(e, "Failed to open the zip"))?;
    let archive = zip::ZipArchive::new(io::BufReader::new(file))
        .map_err(|e| err(ErrorKind::Internal, format!("The zip can't be read: {e}")))?;
    let mut tops = BTreeSet::new();
    let mut with_region = BTreeSet::new();
    let mut files = 0u64;
    for name in archive.file_names() {
        let name =
            name.map_err(|e| err(ErrorKind::Internal, format!("The zip can't be read: {e}")))?;
        let mut parts = name.split('/');
        if let Some(top) = parts.next().filter(|top| !top.is_empty()) {
            tops.insert(top.to_string());
            if parts.next() == Some("region") {
                with_region.insert(top.to_string());
            }
        }
        if !name.ends_with('/') {
            files += 1;
        }
        if tops.len() > MAX_ADOPTED_TOPS {
            return Err(err(ErrorKind::Internal, "The zip isn't a world backup"));
        }
    }
    Ok(TopFolders {
        folders: tops.into_iter().collect(),
        with_region,
        files,
    })
}

/// What adopting a zip reads from its names: its top folders, those with a `region` folder
/// directly inside, and how many files it holds.
struct TopFolders {
    folders: Vec<String>,
    with_region: BTreeSet<String>,
    files: u64,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn uuid() -> InstanceUuid {
        InstanceUuid::from("INSTANCE_world".to_string())
    }

    pub fn record(id: &str, created: i64) -> WorldBackupRecord {
        WorldBackupRecord {
            version: WORLD_RECORD_VERSION,
            id: id.to_string(),
            name: Some("Spawn".to_string()),
            kind: WorldBackupKind::Manual,
            created,
            level: "world".to_string(),
            folders: vec!["world".to_string()],
            instance_name: "Survival".to_string(),
            game_version: "1.21.1".to_string(),
            flavour: "vanilla".to_string(),
            size: 3,
            file_count: 1,
            left_out: 0,
            live: false,
            quiesced: true,
            before_restoring: None,
        }
    }

    /// A zip at `path` with these entries (a trailing `/` is a folder).
    pub fn write_zip(path: &Path, entries: &[(&str, &[u8])]) {
        use std::io::Write;
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut zip = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, bytes) in entries {
            let options = zip::write::SimpleFileOptions::default();
            if name.ends_with('/') {
                zip.add_directory(*name, options).unwrap();
            } else {
                zip.start_file(*name, options).unwrap();
                zip.write_all(bytes).unwrap();
            }
        }
        zip.finish().unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn ids_are_exact() {
        let id = crate::backups::store::new_backup_id(chrono::Utc::now());
        assert!(valid_world_backup_id(&id), "{id}");
        assert!(valid_world_backup_id("20261112-101500-ab12"));
        for bad in [
            "",
            "20261112-101500-AB12",
            "20261112-101500-ab1",
            "20261112_101500-ab12",
            "2026111a-101500-ab12",
            "../../x",
            "20261112-101500-ab12.zip",
            "20261112-101500-ab12/x",
        ] {
            assert!(!valid_world_backup_id(bad), "{bad:?}");
        }
        assert_eq!(created_from_id("20251112-101500-ab12"), Some(1762942500));
    }

    #[test]
    fn records_round_trip_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let store = WorldStore::new(dir.path().to_path_buf());
        let uuid = uuid();
        assert!(store.list(&uuid).unwrap().is_empty());
        for (id, created) in [("20260101-000000-aaaa", 1), ("20260102-000000-bbbb", 2)] {
            write_zip(
                &store.zip_path(&uuid, id).unwrap(),
                &[("world/level.dat", b"x")],
            );
            store.save_record(&uuid, &record(id, created)).unwrap();
        }
        let listed: Vec<String> = store
            .list(&uuid)
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(listed, ["20260102-000000-bbbb", "20260101-000000-aaaa"]);
        assert_eq!(
            store.load(&uuid, "20260101-000000-aaaa").unwrap(),
            record("20260101-000000-aaaa", 1)
        );
        // no temporary file is left
        assert_eq!(
            names(&store.dir(&uuid).unwrap()),
            [
                "20260101-000000-aaaa.json",
                "20260101-000000-aaaa.zip",
                "20260102-000000-bbbb.json",
                "20260102-000000-bbbb.zip"
            ]
        );
    }

    #[test]
    fn a_damaged_or_unknown_sidecar_is_an_error_never_no_backups() {
        let dir = tempfile::tempdir().unwrap();
        let store = WorldStore::new(dir.path().to_path_buf());
        let uuid = uuid();
        let id = "20260101-000000-aaaa";
        write_zip(
            &store.zip_path(&uuid, id).unwrap(),
            &[("world/level.dat", b"x")],
        );
        let path = store.record_path(&uuid, id).unwrap();
        fs::write(&path, b"{not json").unwrap();
        assert!(store.list(&uuid).is_err());
        assert!(store.load(&uuid, id).is_err());
        let mut newer = serde_json::to_value(record(id, 1)).unwrap();
        newer["version"] = 2.into();
        fs::write(&path, serde_json::to_vec(&newer).unwrap()).unwrap();
        assert!(store.list(&uuid).is_err());
        let mut other = record(id, 1);
        other.id = "20260101-000000-bbbb".to_string();
        fs::write(&path, serde_json::to_vec(&other).unwrap()).unwrap();
        assert!(store.list(&uuid).is_err());
        let mut level = record(id, 1);
        level.level = "../x".to_string();
        fs::write(&path, serde_json::to_vec(&level).unwrap()).unwrap();
        assert!(store.list(&uuid).is_err());
        let mut extra = serde_json::to_value(record(id, 1)).unwrap();
        extra["surprise"] = true.into();
        fs::write(&path, serde_json::to_vec(&extra).unwrap()).unwrap();
        assert!(store.list(&uuid).is_err());
        fs::write(&path, vec![b' '; MAX_RECORD_BYTES as usize + 1]).unwrap();
        assert!(store.list(&uuid).is_err());
    }

    /// A crash between a zip's rename and its sidecar leaves a whole zip: it is listed,
    /// with a sidecar written for it.
    #[test]
    fn an_orphaned_zip_is_adopted() {
        let dir = tempfile::tempdir().unwrap();
        let store = WorldStore::new(dir.path().to_path_buf());
        let uuid = uuid();
        let id = "20251112-101500-ab12";
        write_zip(
            &store.zip_path(&uuid, id).unwrap(),
            &[
                ("survival/", b""),
                ("survival/level.dat", b"x"),
                ("survival_nether/DIM-1/r.mca", b"y"),
            ],
        );
        let listed = store.list(&uuid).unwrap();
        assert_eq!(listed.len(), 1);
        let adopted = &listed[0];
        assert_eq!(adopted.kind, WorldBackupKind::Manual);
        assert_eq!(adopted.name, None);
        assert_eq!(adopted.created, 1762942500);
        assert_eq!(adopted.level, "survival");
        assert_eq!(adopted.folders, ["survival", "survival_nether"]);
        assert_eq!(adopted.file_count, 2);
        assert!(store.record_path(&uuid, id).unwrap().is_file());
        assert_eq!(store.load(&uuid, id).unwrap(), *adopted);
        // a zip that isn't one is left as it is, not listed
        fs::write(
            store.zip_path(&uuid, "20261112-101500-cd34").unwrap(),
            b"nope",
        )
        .unwrap();
        assert_eq!(store.list(&uuid).unwrap().len(), 1);
    }

    /// A crash in the middle of a delete leaves the sidecar: it is dropped, so a deleted
    /// backup never comes back.
    #[test]
    fn an_orphaned_sidecar_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let store = WorldStore::new(dir.path().to_path_buf());
        let uuid = uuid();
        let id = "20260101-000000-aaaa";
        write_zip(
            &store.zip_path(&uuid, id).unwrap(),
            &[("world/level.dat", b"x")],
        );
        store.save_record(&uuid, &record(id, 1)).unwrap();
        fs::remove_file(store.zip_path(&uuid, id).unwrap()).unwrap();
        assert!(store.list(&uuid).unwrap().is_empty());
        assert!(!store.record_path(&uuid, id).unwrap().exists());
        assert!(matches!(
            store.load(&uuid, id).unwrap_err().kind,
            ErrorKind::NotFound
        ));
    }

    #[test]
    fn deleting_removes_the_zip_then_the_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let store = WorldStore::new(dir.path().to_path_buf());
        let uuid = uuid();
        let id = "20260101-000000-aaaa";
        write_zip(
            &store.zip_path(&uuid, id).unwrap(),
            &[("world/level.dat", b"x")],
        );
        store.save_record(&uuid, &record(id, 1)).unwrap();
        store.delete(&uuid, id).unwrap();
        assert!(names(&store.dir(&uuid).unwrap()).is_empty());
        assert!(matches!(
            store.delete(&uuid, id).unwrap_err().kind,
            ErrorKind::NotFound
        ));
        store.delete_server(&uuid).unwrap();
        assert!(!store.dir(&uuid).unwrap().exists());
        // a server with none
        store.delete_server(&uuid).unwrap();
    }

    #[test]
    fn bad_uuids_and_ids_never_reach_the_disk() {
        let store = WorldStore::new(PathBuf::from("/nowhere"));
        for bad in ["", "..", "a/b", "DOCKER-x", "con"] {
            let uuid = InstanceUuid::from(bad.to_string());
            assert!(
                matches!(store.dir(&uuid).unwrap_err().kind, ErrorKind::BadRequest),
                "{bad:?}"
            );
            assert!(store.list(&uuid).is_err(), "{bad:?}");
            assert!(store.delete_server(&uuid).is_err(), "{bad:?}");
        }
        for bad in ["../x", "a.json", "20260101-000000-aaaa/../../x"] {
            assert!(matches!(
                store.load(&uuid(), bad).unwrap_err().kind,
                ErrorKind::NotFound
            ));
            assert!(matches!(
                store.delete(&uuid(), bad).unwrap_err().kind,
                ErrorKind::NotFound
            ));
            assert!(store.zip_path(&uuid(), bad).is_err());
        }
    }

    #[test]
    fn adopted_levels() {
        let level = |tops: &[&str], with_region: &[&str]| {
            adopted_level(
                &tops.iter().map(|t| t.to_string()).collect::<Vec<_>>(),
                &with_region.iter().map(|t| t.to_string()).collect(),
            )
        };
        assert_eq!(level(&["world"], &[]).as_deref(), Some("world"));
        assert_eq!(
            level(&["world", "world_nether"], &["world"]).as_deref(),
            Some("world")
        );
        assert_eq!(
            level(&["world", "world_nether", "world_the_end"], &["world"]).as_deref(),
            Some("world")
        );
        // a safety backup of split folders whose level was gone
        assert_eq!(
            level(&["world_nether", "world_the_end"], &[]).as_deref(),
            Some("world")
        );
        // a split folder alone names its level, so its level.dat never lands on the
        // overworld's
        assert_eq!(level(&["world_nether"], &[]).as_deref(), Some("world"));
        assert_eq!(level(&["world_the_end"], &[]).as_deref(), Some("world"));
        // a lone folder with regions of its own is a vanilla level called that
        assert_eq!(
            level(&["mine_nether"], &["mine_nether"]).as_deref(),
            Some("mine_nether")
        );
        assert_eq!(level(&["_nether"], &[]).as_deref(), Some("_nether"));
        assert_eq!(level(&[], &[]), None);
    }

    /// A zip that is a link isn't a backup to `load` either (the list never shows it), so
    /// a download or restore never reads through it.
    #[cfg(unix)]
    #[test]
    fn a_linked_zip_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = WorldStore::new(dir.path().to_path_buf());
        let uuid = uuid();
        let id = "20260101-000000-aaaa";
        let elsewhere = dir.path().join("elsewhere.zip");
        write_zip(&elsewhere, &[("world/level.dat", b"x")]);
        fs::create_dir_all(store.dir(&uuid).unwrap()).unwrap();
        std::os::unix::fs::symlink(&elsewhere, store.zip_path(&uuid, id).unwrap()).unwrap();
        store.save_record(&uuid, &record(id, 1)).unwrap();
        assert!(matches!(
            store.load(&uuid, id).unwrap_err().kind,
            ErrorKind::NotFound
        ));
    }

    /// A damaged record's error names the backup, not where Lodestone keeps it.
    #[test]
    fn errors_name_no_paths() {
        let dir = tempfile::tempdir().unwrap();
        let store = WorldStore::new(dir.path().to_path_buf());
        let uuid = uuid();
        let id = "20260101-000000-aaaa";
        write_zip(
            &store.zip_path(&uuid, id).unwrap(),
            &[("world/level.dat", b"x")],
        );
        fs::write(store.record_path(&uuid, id).unwrap(), b"{").unwrap();
        let message = store.list(&uuid).unwrap_err().source.to_string();
        assert!(message.contains(id), "{message}");
        assert!(
            !message.contains(&dir.path().to_string_lossy().to_string()),
            "{message}"
        );
    }

    #[test]
    fn sweep_removes_leftovers() {
        let dir = tempfile::tempdir().unwrap();
        let store = WorldStore::new(dir.path().to_path_buf());
        let uuid = uuid();
        let server = store.dir(&uuid).unwrap();
        fs::create_dir_all(&server).unwrap();
        for name in [
            ".20260101-000000-aaaa.zip.part",
            ".incoming-0123456789abcdef.tmp",
            "20260101-000000-bbbb.zip",
        ] {
            fs::write(server.join(name), b"x").unwrap();
        }
        store.sweep();
        assert_eq!(names(&server), ["20260101-000000-bbbb.zip"]);
    }
}
