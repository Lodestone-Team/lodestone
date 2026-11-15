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

/// Whether `path` is a config (in `config/`, `defaultconfigs/` or `kubejs/`, whatever
/// their letter case), or a file the game writes itself (`options.txt`, say): a dropped
/// override there stays whatever [`Rules::dropped_overrides_go`] says. A path that isn't a
/// plain relative path counts, so it stays too.
fn kept_when_dropped(path: &str) -> bool {
    lodestone_image::CanonicalPath::new(path).map_or(true, |canonical| {
        let parts: Vec<&str> = canonical.components().collect();
        lodestone_image::paths::is_config(&canonical)
            || crate::images::scope::never_pack_content(&parts)
    })
}

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

    fn of(kind: HashKind, hex: String) -> Self {
        match kind {
            HashKind::Sha1 => Digest::Sha1(hex),
            HashKind::Sha256 => Digest::Sha256(hex),
        }
    }

    /// Whether both name the same hash of the same bytes.
    fn same(&self, other: &Digest) -> bool {
        self.kind() == other.kind() && self.hex().eq_ignore_ascii_case(other.hex())
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

/// What a file the merge decided to overwrite or remove must still be when the update
/// is applied, for the decision to hold: the update may be applied long after it was
/// planned (its prompt waits for the player, the game may run meanwhile), and a file the
/// player changed since must not be taken for the version's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expect {
    /// Not there: a file the update adds, which would go over the player's own.
    Absent,
    /// As the plan found it: the last version's file, unchanged (as read, or as its
    /// record and size said), or, for a file the update replaces whatever it holds (a
    /// server's mod the player changed, the player's own file at a path the version now
    /// uses), the bytes it held then, so a change made after the plan (while its files
    /// download, say) isn't overwritten unseen.
    Unchanged(Digest),
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
    /// What each file to overwrite or remove (by its path on disk) must still be when the
    /// update is applied: see [`Expect`]. Only a file that couldn't be read has none.
    pub expected: BTreeMap<String, Expect>,
}

/// The files on disk, hashed when asked, each once per hash. No game folder: nothing is
/// on disk (a new instance).
struct Disk<'a> {
    game: Option<&'a Path>,
    hashes: RefCell<HashMap<(String, HashKind), Option<String>>>,
}

impl Disk<'_> {
    fn hash(&self, relative: &str, kind: HashKind) -> Option<String> {
        let key = (relative.to_string(), kind);
        if let Some(found) = self.hashes.borrow().get(&key) {
            return found.clone();
        }
        let path = self.game?.join(relative);
        // only a file has a hash: a link at the path is the player's, whatever it names
        let found = std::fs::symlink_metadata(&path)
            .ok()
            .filter(|m| m.is_file())
            .and_then(|_| match kind {
                HashKind::Sha1 => sha1_on_disk(&path),
                HashKind::Sha256 => crate::cores::player_image::sha256_of_file(&path).ok(),
            });
        self.hashes.borrow_mut().insert(key, found.clone());
        found
    }

    /// What is at `relative` (or, for a mod, its `.disabled` name) on disk: a file, or
    /// anything else there (a link, a folder), which [`Disk::is_file`] tells apart.
    fn find(&self, relative: &str) -> Option<String> {
        let game = self.game?;
        let mut candidates = vec![relative.to_string()];
        if relative.starts_with("mods/") && !is_disabled_name(relative) {
            candidates.push(disabled_name(relative));
        }
        candidates
            .into_iter()
            .find(|path| std::fs::symlink_metadata(game.join(path)).is_ok())
    }

    /// Whether `actual` is a plain file, not a link or a folder: only a file is ever
    /// overwritten or removed. Anything else at a path the version places is the
    /// player's (a config they linked to a shared copy, say) and is left as it is.
    fn is_file(&self, actual: &str) -> bool {
        self.game.is_some_and(|game| {
            std::fs::symlink_metadata(game.join(actual)).is_ok_and(|m| m.is_file())
        })
    }

    /// What the file at `actual` must still be for a plan that replaces it whatever it
    /// holds: the bytes it holds now (hashed already by the comparison that found it
    /// changed). `None` when it can't be read.
    fn as_found(&self, actual: &str, kind: HashKind) -> Option<Expect> {
        self.hash(actual, kind)
            .map(|hex| Expect::Unchanged(Digest::of(kind, hex)))
    }

    /// The size of the file at `actual`.
    fn len(&self, actual: &str) -> Option<u64> {
        let meta = std::fs::symlink_metadata(self.game?.join(actual)).ok()?;
        meta.is_file().then_some(meta.len())
    }

    /// Whether `relative` names, in other letter case, the file at `other` on a disk that
    /// doesn't tell case apart: it is found, but its folder lists it only as `other`.
    fn same_file_other_case(&self, relative: &str, other: &str) -> bool {
        let Some(game) = self.game else {
            return false;
        };
        if relative == other || !relative.eq_ignore_ascii_case(other) {
            return false;
        }
        let path = game.join(relative);
        let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
            return false;
        };
        std::fs::symlink_metadata(&path).is_ok()
            && std::fs::read_dir(parent)
                .is_ok_and(|entries| !entries.flatten().any(|e| e.file_name() == name))
    }

    /// Whether the file at `actual` has `digest`.
    fn matches(&self, actual: &str, digest: &Digest) -> bool {
        self.hash(actual, digest.kind())
            .is_some_and(|hash| hash.eq_ignore_ascii_case(digest.hex()))
    }
}

/// How a merge treats the files on disk.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Rules<'a> {
    /// The new files under these folders (a server's `mods/`) replace the user's changed
    /// copies instead of keeping them.
    pub authoritative: &'a [&'a str],
    /// A file whose recorded hash is the new one's, and whose size on disk is the new
    /// file's, is taken as unchanged without being read: a server's view, which is
    /// planned at every Play that brings a change, so a change of one entry reads only
    /// what changed. (A file changed by hand to the same size is put right when its
    /// entry next changes, or by the check on a server's answer that the instance is in
    /// step, which reads the mods.)
    pub trust_recorded: bool,
    /// Paths (as the new version names them) that are read whatever `trust_recorded`
    /// says: the mods a server's answer that the instance is in step found missing or
    /// changed on disk.
    pub untrusted: Option<&'a BTreeSet<String>>,
    /// A dropped override that is unchanged is removed wherever it is but in a config
    /// folder or a file the game writes ([`kept_when_dropped`]), not only in `mods/`,
    /// `resourcepacks/` and `shaderpacks/`: a server's view, whose base pack's files
    /// (CraftTweaker's `scripts/`, say) go when the view's `removed` names them or the
    /// pack's next version drops them ("From one view to the next on disk"). A modpack
    /// update keeps them, as before.
    pub dropped_overrides_go: bool,
}

/// Plans the move from the files `current` records to the files `new` places, for the
/// game folder `game` (`None` for a new instance, which has none yet), by `rules`.
pub(crate) fn plan(
    game: Option<&Path>,
    current: &BTreeMap<String, PackFile>,
    new: &BTreeMap<String, Wanted>,
    rules: Rules<'_>,
) -> Merged {
    let authoritative = rules.authoritative;
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
    // Unchanged by its record and size, without reading it (see `Rules::trust_recorded`).
    let unchanged = |path: &str, old: Option<&PackFile>, actual: &str, wanted: &Wanted| {
        rules.trust_recorded
            && !rules.untrusted.is_some_and(|paths| paths.contains(path))
            && old.is_some_and(|old| old.digest().same(&wanted.digest))
            && disk.len(actual) == Some(wanted.fetch.size())
    };

    // The new paths by their letters folded, to find a removed file a new path names in
    // other letter case without comparing every pair.
    let mut new_folded: HashMap<String, Vec<&String>> = HashMap::new();
    for path in new.keys() {
        new_folded
            .entry(path.to_ascii_lowercase())
            .or_default()
            .push(path);
    }
    // The old version's files that go away, by project, to pair them with new versions.
    let mut removed_by_project: HashMap<String, String> = HashMap::new();
    for (path, old) in current {
        if new.contains_key(path) {
            continue;
        }
        let removable = match old.source {
            FileSource::Index => true,
            FileSource::Override => {
                CONTENT_DIRS
                    .iter()
                    .any(|dir| path.starts_with(&format!("{dir}/")))
                    || (rules.dropped_overrides_go && !kept_when_dropped(path))
            }
        };
        if !removable {
            continue;
        }
        match disk.find(path) {
            None => {}
            // a new file names it in other letter case: it is that file (renaming only the
            // letters of a file the disk can't tell apart would delete it)
            Some(actual)
                if new_folded
                    .get(&actual.to_ascii_lowercase())
                    .is_some_and(|others| {
                        others
                            .iter()
                            .any(|other| disk.same_file_other_case(other, &actual))
                    }) => {}
            Some(actual) if disk.matches(&actual, &old.digest()) => {
                if let Some(project) = &old.project {
                    removed_by_project.insert(project.clone(), actual.clone());
                }
                merged
                    .expected
                    .insert(actual.clone(), Expect::Unchanged(old.digest()));
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
            (_, Some(actual)) if !disk.is_file(&actual) => merged.kept_changed.push(actual),
            (old, Some(actual)) if unchanged(path, old, &actual, wanted) => {}
            (_, Some(actual)) if disk.matches(&actual, &wanted.digest) => {}
            (Some(old), Some(actual)) if disk.matches(&actual, &old.digest()) => {
                merged.updated.push((actual.clone(), actual.clone()));
                merged
                    .expected
                    .insert(actual.clone(), Expect::Unchanged(old.digest()));
                write(&mut merged, actual);
            }
            (Some(_), Some(actual)) if replaces(path) => {
                merged.updated.push((actual.clone(), actual.clone()));
                if let Some(expect) = disk.as_found(&actual, wanted.digest.kind()) {
                    merged.expected.insert(actual.clone(), expect);
                }
                write(&mut merged, actual);
            }
            (Some(_), Some(actual)) => merged.kept_changed.push(actual),
            (None, Some(actual)) => {
                merged.replaced_own.push(actual.clone());
                if let Some(expect) = disk.as_found(&actual, wanted.digest.kind()) {
                    merged.expected.insert(actual.clone(), expect);
                }
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
                merged.expected.insert(dest.clone(), Expect::Absent);
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
                merged.expected.insert(path.clone(), Expect::Absent);
                merged.writes.push((wanted.fetch.clone(), path.clone()));
            }
            (_, Some(actual)) if !disk.is_file(&actual) => merged.overrides_kept.push(actual),
            (old, Some(actual)) if unchanged(path, old, &actual, wanted) => {}
            (_, Some(actual)) if disk.matches(&actual, &wanted.digest) => {}
            (Some(old), Some(actual)) if disk.matches(&actual, &old.digest()) => {
                merged.overrides_written.push(actual.clone());
                merged
                    .expected
                    .insert(actual.clone(), Expect::Unchanged(old.digest()));
                merged.writes.push((wanted.fetch.clone(), actual));
            }
            (_, Some(actual)) if replaces(path) => {
                merged.overrides_written.push(actual.clone());
                if let Some(expect) = disk.as_found(&actual, wanted.digest.kind()) {
                    merged.expected.insert(actual.clone(), expect);
                }
                merged.writes.push((wanted.fetch.clone(), actual));
            }
            (_, Some(actual)) => merged.overrides_kept.push(actual),
        }
    }
    merged
}

/// Whether the file at `relative` in `game` is still as `expect` says. A file the update
/// would overwrite or remove that is gone now is (writing or removing it loses nothing).
pub(crate) fn still(game: &Path, relative: &str, expect: &Expect) -> bool {
    let path = game.join(relative);
    match expect {
        Expect::Absent => std::fs::symlink_metadata(&path)
            .is_err_and(|err| err.kind() == std::io::ErrorKind::NotFound),
        Expect::Unchanged(digest) => {
            let disk = Disk {
                game: Some(game),
                hashes: RefCell::new(HashMap::new()),
            };
            match std::fs::symlink_metadata(&path) {
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
                Ok(meta) if meta.is_file() => disk.matches(relative, digest),
                _ => false,
            }
        }
    }
}

/// What `modpack.json` records for the files `new` places.
pub(crate) fn record_files(new: &BTreeMap<String, Wanted>) -> BTreeMap<String, PackFile> {
    new.iter()
        .map(|(path, wanted)| (path.clone(), wanted.record.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use lodestone_image::hex::sha256_hex;

    use super::*;

    /// A server's file, from the server.
    fn object(bytes: &[u8]) -> Wanted {
        Wanted {
            digest: Digest::Sha256(sha256_hex(bytes)),
            record: PackFile {
                sha1: String::new(),
                source: FileSource::Index,
                project: None,
                sha256: Some(sha256_hex(bytes)),
            },
            fetch: Fetch::Object {
                sha256: sha256_hex(bytes),
                size: bytes.len() as u64,
            },
        }
    }

    fn game_with(files: &[(&str, &[u8])]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, bytes) in files {
            let path = dir.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        }
        dir
    }

    /// A server's plan doesn't read a file whose record is the new file and whose size is
    /// right: here its bytes differ, which only reading would find.
    #[test]
    fn trusted_records_skip_reading_unchanged_files() {
        let game = game_with(&[("config/a.toml", b"AAAA"), ("mods/b.jar", b"BBBB")]);
        let mut a = object(b"aaaa");
        a.record.source = FileSource::Override;
        let current: BTreeMap<String, PackFile> = [
            ("config/a.toml".to_string(), a.record.clone()),
            ("mods/b.jar".to_string(), object(b"bbbb").record),
        ]
        .into();
        let new: BTreeMap<String, Wanted> = [
            ("config/a.toml".to_string(), a),
            ("mods/b.jar".to_string(), object(b"bbbb")),
        ]
        .into();
        let read = plan(Some(game.path()), &current, &new, Rules::default());
        assert_eq!(read.kept_changed, ["mods/b.jar"]);
        assert_eq!(read.overrides_kept, ["config/a.toml"]);
        let trusted = plan(
            Some(game.path()),
            &current,
            &new,
            Rules {
                trust_recorded: true,
                ..Rules::default()
            },
        );
        assert_eq!(trusted, Merged::default());
        // a size that differs is read
        std::fs::write(game.path().join("mods/b.jar"), b"BBBBB").unwrap();
        let trusted = plan(
            Some(game.path()),
            &current,
            &new,
            Rules {
                trust_recorded: true,
                ..Rules::default()
            },
        );
        assert_eq!(trusted.kept_changed, ["mods/b.jar"]);
    }

    /// What each file a plan changes must still be when it is applied: the last version's
    /// file, unchanged, for one it overwrites or removes as unchanged; nothing there for
    /// one it adds; and the bytes it found for one it replaces whatever it holds (a
    /// server's mod the player changed), so a change made after the plan stops it.
    #[test]
    fn the_plan_says_what_each_file_it_changes_must_still_be() {
        let game = game_with(&[
            ("mods/old.jar", b"old"),
            ("mods/changed.jar", b"the player's"),
            ("mods/up.jar", b"up 1"),
        ]);
        let current: BTreeMap<String, PackFile> = [
            ("mods/old.jar".to_string(), object(b"old").record),
            (
                "mods/changed.jar".to_string(),
                object(b"the server's").record,
            ),
            ("mods/up.jar".to_string(), object(b"up 1").record),
        ]
        .into();
        let new: BTreeMap<String, Wanted> = [
            ("mods/changed.jar".to_string(), object(b"the server's 2")),
            ("mods/up.jar".to_string(), object(b"up 2")),
            ("mods/added.jar".to_string(), object(b"added")),
        ]
        .into();
        let merged = plan(
            Some(game.path()),
            &current,
            &new,
            Rules {
                authoritative: &["mods"],
                ..Rules::default()
            },
        );
        let sha = |bytes: &[u8]| Expect::Unchanged(Digest::Sha256(sha256_hex(bytes)));
        let expected: BTreeMap<String, Expect> = [
            ("mods/old.jar".to_string(), sha(b"old")),
            ("mods/up.jar".to_string(), sha(b"up 1")),
            ("mods/added.jar".to_string(), Expect::Absent),
            // replaced whatever it holds, but only as the plan found it
            ("mods/changed.jar".to_string(), sha(b"the player's")),
        ]
        .into();
        assert_eq!(merged.expected, expected);
        assert!(merged.writes.iter().any(|(_, p)| p == "mods/changed.jar"));

        assert!(still(game.path(), "mods/old.jar", &sha(b"old")));
        std::fs::write(game.path().join("mods/old.jar"), b"edited").unwrap();
        assert!(!still(game.path(), "mods/old.jar", &sha(b"old")));
        std::fs::remove_file(game.path().join("mods/old.jar")).unwrap();
        assert!(still(game.path(), "mods/old.jar", &sha(b"old")));
        assert!(still(game.path(), "mods/added.jar", &Expect::Absent));
        std::fs::write(game.path().join("mods/added.jar"), b"the player's").unwrap();
        assert!(!still(game.path(), "mods/added.jar", &Expect::Absent));
    }

    /// A file the next version places under the same name in other letter case: on a disk
    /// that doesn't tell case apart (macOS's and Windows's) it is the same file, so it
    /// isn't removed; on one that does, the old name goes and the new one is written.
    #[test]
    fn a_change_of_letter_case_keeps_the_file() {
        let game = game_with(&[("mods/Sodium.jar", b"sodium")]);
        let current: BTreeMap<String, PackFile> =
            [("mods/Sodium.jar".to_string(), object(b"sodium").record)].into();
        let new: BTreeMap<String, Wanted> =
            [("mods/sodium.jar".to_string(), object(b"sodium"))].into();
        let merged = plan(Some(game.path()), &current, &new, Rules::default());
        if game.path().join("mods/SODIUM.JAR").exists() {
            assert!(merged.removals.is_empty());
            assert!(merged.writes.is_empty());
        } else {
            assert_eq!(merged.removals, ["mods/Sodium.jar"]);
            assert_eq!(merged.added, ["mods/sodium.jar"]);
        }
    }

    /// A server's dropped pack override goes when unchanged wherever it is but in a config
    /// folder or a file the game writes; a modpack update keeps it outside the content
    /// folders, as before.
    #[test]
    fn dropped_overrides_go_for_a_server_but_configs_stay() {
        let game = game_with(&[
            ("scripts/recipes.zs", b"recipes"),
            ("config/a.toml", b"a"),
            ("options.txt", b"options"),
            ("scripts/edited.zs", b"the player's"),
        ]);
        let current: BTreeMap<String, PackFile> = [
            ("scripts/recipes.zs", &b"recipes"[..]),
            ("config/a.toml", b"a"),
            ("options.txt", b"options"),
            ("scripts/edited.zs", b"the pack's"),
        ]
        .into_iter()
        .map(|(path, bytes)| {
            let mut record = object(bytes).record;
            record.source = FileSource::Override;
            (path.to_string(), record)
        })
        .collect();
        let new = BTreeMap::new();
        let modpack = plan(Some(game.path()), &current, &new, Rules::default());
        assert!(modpack.removals.is_empty());
        let server = plan(
            Some(game.path()),
            &current,
            &new,
            Rules {
                dropped_overrides_go: true,
                ..Rules::default()
            },
        );
        assert_eq!(server.removals, ["scripts/recipes.zs"]);
        assert_eq!(server.kept_changed, ["scripts/edited.zs"]);
    }

    /// A link (or a folder) at a path the version places is the player's: kept, never
    /// written over, and never taken for a file that appeared after the plan.
    #[cfg(unix)]
    #[test]
    fn a_link_at_a_placed_path_is_kept() {
        let game = game_with(&[("shared/xaero.cfg", b"shared")]);
        std::fs::create_dir_all(game.path().join("config")).unwrap();
        std::fs::create_dir_all(game.path().join("mods")).unwrap();
        std::os::unix::fs::symlink(
            game.path().join("shared/xaero.cfg"),
            game.path().join("config/xaero.cfg"),
        )
        .unwrap();
        std::os::unix::fs::symlink(
            game.path().join("shared/xaero.cfg"),
            game.path().join("mods/linked.jar"),
        )
        .unwrap();
        let mut config = object(b"server's");
        config.record.source = FileSource::Override;
        let new: BTreeMap<String, Wanted> = [
            ("config/xaero.cfg".to_string(), config),
            ("mods/linked.jar".to_string(), object(b"server's mod")),
        ]
        .into();
        let merged = plan(
            Some(game.path()),
            &BTreeMap::new(),
            &new,
            Rules {
                authoritative: &["mods"],
                ..Rules::default()
            },
        );
        assert!(merged.writes.is_empty());
        assert_eq!(merged.overrides_kept, ["config/xaero.cfg"]);
        assert_eq!(merged.kept_changed, ["mods/linked.jar"]);
        assert!(merged.expected.is_empty());
    }

    /// Paths a 204's check found changed are read though their records would be trusted.
    #[test]
    fn untrusted_paths_are_read() {
        let game = game_with(&[("mods/a.jar", b"AAAA"), ("mods/b.jar", b"BBBB")]);
        let current: BTreeMap<String, PackFile> = [
            ("mods/a.jar".to_string(), object(b"aaaa").record),
            ("mods/b.jar".to_string(), object(b"bbbb").record),
        ]
        .into();
        let new: BTreeMap<String, Wanted> = [
            ("mods/a.jar".to_string(), object(b"aaaa")),
            ("mods/b.jar".to_string(), object(b"bbbb")),
        ]
        .into();
        let untrusted: BTreeSet<String> = ["mods/b.jar".to_string()].into();
        let merged = plan(
            Some(game.path()),
            &current,
            &new,
            Rules {
                authoritative: &["mods"],
                trust_recorded: true,
                untrusted: Some(&untrusted),
                ..Rules::default()
            },
        );
        let written: Vec<&str> = merged.writes.iter().map(|(_, p)| p.as_str()).collect();
        assert_eq!(written, ["mods/b.jar"]);
    }
}
