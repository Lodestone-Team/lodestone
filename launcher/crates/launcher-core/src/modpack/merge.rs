//! The three-way merge an update plans with: the files the last version placed (as
//! `modpack.json` records them), the files the new version places, and what is on disk.
//! A modpack update ([`super::update::plan_with`]) and a server's image
//! ([`crate::cores::image_sync::plan`]) both plan through it, so the player's own mods and
//! changed configs stay the same way in both (see [`super::update`] for the rules).
//!
//! A file is compared by the hash its source gives: a pack's files by SHA-1, as packs
//! publish them, and a server's files by SHA-256, which every image entry has (only
//! Modrinth and CurseForge files have a SHA-1 too). The disk is hashed only where a
//! comparison needs it, once per file and hash.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
};

use lodestone_minecraft::mrpack::PackDownload;

use super::{FileSource, OverrideFile, PackFile, import::sha1_on_disk};
use crate::content::{disabled_name, is_disabled_name};

/// The folders whose files a pack's dropped overrides are removed from.
const CONTENT_DIRS: [&str; 3] = ["mods", "resourcepacks", "shaderpacks"];

/// A file's hash, as the version that places it gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Digest {
    Sha1(String),
    Sha256(String),
}

impl Digest {
    fn kind(&self) -> HashKind {
        match self {
            Digest::Sha1(_) => HashKind::Sha1,
            Digest::Sha256(_) => HashKind::Sha256,
        }
    }

    fn hex(&self) -> &str {
        match self {
            Digest::Sha1(hex) | Digest::Sha256(hex) => hex,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum HashKind {
    Sha1,
    Sha256,
}

/// Where the bytes of a file to write come from.
#[derive(Clone, Debug, PartialEq)]
pub enum Fetch {
    /// Downloaded: a pack's index file, a Modrinth or CurseForge file. When the download
    /// has no SHA-1 (a server's Modrinth file may not), it is checked against
    /// `expect_sha256` as it downloads; otherwise against its SHA-1, and then against
    /// `expect_sha256` too.
    Download {
        download: PackDownload,
        expect_sha256: Option<String>,
    },
    /// Copied out of the pack's zip. `expect_sha256` is the server's hash of the file,
    /// when a server's view names the pack's copy.
    Override {
        file: OverrideFile,
        expect_sha256: Option<String>,
    },
    /// Downloaded from the server whose image names it.
    Object { sha256: String, size: u64 },
}

impl Fetch {
    /// The size it writes, when known.
    pub fn size(&self) -> u64 {
        match self {
            Fetch::Download { download, .. } => download.size,
            Fetch::Override { file, .. } => file.size,
            Fetch::Object { size, .. } => *size,
        }
    }
}

/// A file the new version places.
#[derive(Clone, Debug, PartialEq)]
pub struct Wanted {
    /// What the file on disk is compared with.
    pub digest: Digest,
    /// What `modpack.json` records for it once placed.
    pub record: PackFile,
    pub fetch: Fetch,
}

impl Wanted {
    /// A pack's index file.
    pub fn download(download: &PackDownload) -> Self {
        Wanted {
            digest: Digest::Sha1(download.sha1.clone()),
            record: PackFile {
                sha1: download.sha1.clone(),
                source: FileSource::Index,
                project: super::addon::download_project(download),
                sha256: None,
            },
            fetch: Fetch::Download {
                download: download.clone(),
                expect_sha256: None,
            },
        }
    }

    /// A pack's override file.
    pub fn override_file(file: &OverrideFile) -> Self {
        Wanted {
            digest: Digest::Sha1(file.sha1.clone()),
            record: PackFile {
                sha1: file.sha1.clone(),
                source: FileSource::Override,
                project: None,
                sha256: None,
            },
            fetch: Fetch::Override {
                file: file.clone(),
                expect_sha256: None,
            },
        }
    }

    fn project(&self) -> Option<&str> {
        self.record.project.as_deref()
    }
}

/// What the merge decided. Paths are relative to the game folder.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Merged {
    pub added: Vec<String>,
    pub updated: Vec<(String, String)>,
    pub removed: Vec<String>,
    pub kept_changed: Vec<String>,
    pub replaced_own: Vec<String>,
    pub overrides_written: Vec<String>,
    pub overrides_kept: Vec<String>,
    pub writes: Vec<(Fetch, String)>,
    pub removals: Vec<String>,
}

/// The files on disk, hashed when asked, each once per hash.
struct Disk<'a> {
    game: &'a Path,
    hashes: RefCell<HashMap<(String, HashKind), Option<String>>>,
}

impl Disk<'_> {
    fn hash(&self, relative: &str, kind: HashKind) -> Option<String> {
        let key = (relative.to_string(), kind);
        if let Some(found) = self.hashes.borrow().get(&key) {
            return found.clone();
        }
        let path = self.game.join(relative);
        let found = match kind {
            HashKind::Sha1 => sha1_on_disk(&path),
            HashKind::Sha256 => std::fs::symlink_metadata(&path)
                .ok()
                .filter(|m| m.is_file())
                .and_then(|_| crate::cores::player_image::sha256_of_file(&path).ok()),
        };
        self.hashes.borrow_mut().insert(key, found.clone());
        found
    }

    /// The file at `relative` (or, for a mod, its `.disabled` name), as it is on disk.
    fn find(&self, relative: &str) -> Option<String> {
        let mut candidates = vec![relative.to_string()];
        if relative.starts_with("mods/") && !is_disabled_name(relative) {
            candidates.push(disabled_name(relative));
        }
        candidates
            .into_iter()
            .find(|path| std::fs::symlink_metadata(self.game.join(path)).is_ok_and(|m| m.is_file()))
    }

    /// Whether the file at `actual` has `digest`.
    fn matches(&self, actual: &str, digest: &Digest) -> bool {
        self.hash(actual, digest.kind())
            .is_some_and(|hash| hash.eq_ignore_ascii_case(digest.hex()))
    }
}

/// Plans the move from the files `current` records to the files `new` places, for the
/// game folder `game`. The new files under the folders `authoritative` (a server's
/// `mods/`) replace the user's changed copies instead of keeping them.
pub(crate) fn plan(
    game: &Path,
    current: &BTreeMap<String, PackFile>,
    new: &BTreeMap<String, Wanted>,
    authoritative: &[&str],
) -> Merged {
    let replaces = |path: &str| {
        authoritative
            .iter()
            .any(|dir| path.starts_with(&format!("{}/", dir.trim_end_matches('/'))))
    };
    let disk = Disk {
        game,
        hashes: RefCell::new(HashMap::new()),
    };
    let mut merged = Merged::default();

    // The old version's files that go away, by project, to pair them with new versions.
    let mut removed_by_project: HashMap<String, String> = HashMap::new();
    for (path, old) in current {
        if new.contains_key(path) {
            continue;
        }
        let removable = match old.source {
            FileSource::Index => true,
            FileSource::Override => CONTENT_DIRS
                .iter()
                .any(|dir| path.starts_with(&format!("{dir}/"))),
        };
        if !removable {
            continue;
        }
        match disk.find(path) {
            None => {}
            Some(actual) if disk.matches(&actual, &old.digest()) => {
                if let Some(project) = &old.project {
                    removed_by_project.insert(project.clone(), actual.clone());
                }
                merged.removals.push(actual);
            }
            Some(actual) => merged.kept_changed.push(actual),
        }
    }

    for (path, wanted) in new
        .iter()
        .filter(|(_, w)| w.record.source == FileSource::Index)
    {
        let write = |merged: &mut Merged, dest: String| {
            merged.writes.push((wanted.fetch.clone(), dest));
        };
        let actual = disk.find(path);
        match (current.get(path), actual) {
            (_, Some(actual)) if disk.matches(&actual, &wanted.digest) => {}
            (Some(old), Some(actual)) if disk.matches(&actual, &old.digest()) => {
                merged.updated.push((actual.clone(), actual.clone()));
                write(&mut merged, actual);
            }
            (Some(_), Some(actual)) if replaces(path) => {
                merged.updated.push((actual.clone(), actual.clone()));
                write(&mut merged, actual);
            }
            (Some(_), Some(actual)) => merged.kept_changed.push(actual),
            (None, Some(actual)) => {
                merged.replaced_own.push(actual.clone());
                write(&mut merged, actual);
            }
            (_, None) => {
                // A new version of a file the old version had: it stays disabled if it
                // was.
                let old = wanted.project().and_then(|p| removed_by_project.get(p));
                let dest = match old {
                    Some(old) if is_disabled_name(old) && path.starts_with("mods/") => {
                        disabled_name(path)
                    }
                    _ => path.clone(),
                };
                match old {
                    Some(old) => merged.updated.push((old.clone(), dest.clone())),
                    None => merged.added.push(dest.clone()),
                }
                write(&mut merged, dest);
            }
        }
    }
    // Paired removals are shown as updates, not as removals.
    let paired: BTreeSet<&String> = merged.updated.iter().map(|(old, _)| old).collect();
    merged.removed = merged
        .removals
        .iter()
        .filter(|p| !paired.contains(p))
        .cloned()
        .collect();

    for (path, wanted) in new
        .iter()
        .filter(|(_, w)| w.record.source == FileSource::Override)
    {
        let actual = disk.find(path);
        match (current.get(path), actual) {
            (_, None) => {
                merged.overrides_written.push(path.clone());
                merged.writes.push((wanted.fetch.clone(), path.clone()));
            }
            (_, Some(actual)) if disk.matches(&actual, &wanted.digest) => {}
            (Some(old), Some(actual)) if disk.matches(&actual, &old.digest()) => {
                merged.overrides_written.push(actual.clone());
                merged.writes.push((wanted.fetch.clone(), actual));
            }
            (_, Some(actual)) if replaces(path) => {
                merged.overrides_written.push(actual.clone());
                merged.writes.push((wanted.fetch.clone(), actual));
            }
            (_, Some(actual)) => merged.overrides_kept.push(actual),
        }
    }
    merged
}

/// What `modpack.json` records for the files `new` places.
pub(crate) fn record_files(new: &BTreeMap<String, Wanted>) -> BTreeMap<String, PackFile> {
    new.iter()
        .map(|(path, wanted)| (path.clone(), wanted.record.clone()))
        .collect()
}
