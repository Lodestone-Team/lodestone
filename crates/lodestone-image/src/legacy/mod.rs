//! Readers for the formats images replace, and writers back to them.
//!
//! Old data stays readable: each module here reads one format Lodestone wrote before
//! images and converts it into images, and converts those images back into the old format.
//! Nothing here depends on the core or the Launcher, so each module carries a copy of the
//! serde types it reads, with a note naming the file and commit they mirror.
//!
//! - [`core_backup`]: the core's backup manifests (schema 1) become data images.
//! - [`lodesnap`]: the Launcher's snapshot manifests and `.lodesnap` files (schema 1)
//!   become images.
//! - [`core_modpack`]: the core's modpack `record.json` and `history.json` become a lineage
//!   of content images on the pack as their base.
//! - [`launcher_modpack`]: the Launcher's `modpack.json` and `content.json` become a
//!   content image with the pack as its base and each file's Modrinth or CurseForge source.
//! - [`world_zip`]: the Launcher's world backup zips become data images, read without
//!   extracting them.
//!
//! An old format knows things an image doesn't model (a backup's id and statistics, a
//! snapshot's selection, the launch settings a snapshot kept, the titles a content cache
//! looked up), and lacks things an image needs (the SHA-256 of a file the old format only
//! knew by SHA-1). So a conversion gives three things:
//!
//! - the image, valid and sealed;
//! - the rest: what the old format had and the image doesn't, so the old file can be
//!   written again from the image and the rest, equal to the original;
//! - notes on what didn't go into the image and why, for logs and the UI.
//!
//! Files the image can't list (a name that isn't a valid canonical path, a path that is
//! never recorded, a file whose bytes nobody can hash) are in its `withheld` list as well
//! as in the notes, so a reader of the image knows they exist.
//!
//! Like the rest of the crate, nothing here touches files or the network: callers read the
//! old files and pass their bytes, and say what they know about file contents ([`Known`]).

pub mod core_backup;
pub mod core_modpack;
pub mod launcher_modpack;
pub mod lodesnap;
pub mod world_zip;

use std::collections::{BTreeSet, HashMap};

use crate::hex::{is_sha1, is_sha256, is_sha512};
use crate::manifest::{
    whole_number_field, Image, ProbeError, Withheld, WithheldReason, MAX_TEXT_LEN,
};
use crate::paths::MAX_PATH_BYTES;

/// Why an old file can't be read or written.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LegacyError {
    #[error("it isn't readable JSON: {0}")]
    Json(String),
    /// Made by a newer Lodestone: refused, like the old readers refused it.
    #[error("it has version {found}, newer than version {supported} that this reader knows")]
    Newer { found: u64, supported: u64 },
    #[error("it is invalid: {0}")]
    Invalid(String),
}

/// A file whose contents the caller knows: its SHA-256 and size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownFile {
    pub sha256: String,
    pub size: u64,
}

/// What the caller knows that an old format didn't record. The modpack records list files
/// by SHA-1 only, and an image needs each file's SHA-256 (its store key) and size; a base
/// pack needs the SHA-512 of its `.mrpack`. The caller finds them by hashing the files on
/// disk or in the old store, or asking Modrinth, before converting.
pub trait Known {
    /// The file with this SHA-1 (lowercase hex), if its bytes were hashed.
    fn file(&self, sha1: &str) -> Option<KnownFile>;

    /// The SHA-512 of the `.mrpack` of `version` of the Modrinth project `project`.
    fn pack_sha512(&self, project: &str, version: &str) -> Option<String>;

    /// Whether the file the old record keys as `path` (its enabled name) is disabled on
    /// disk now, as `<path>.disabled`. The records key disabled files by their enabled
    /// name; with this, the converted image lists such a file as it is on disk, so placing
    /// the image doesn't enable it and the next image recorded from disk doesn't show a
    /// remove and an add. Only images of the files as they are now ask (the Launcher's
    /// `content.json`, the core's `record.json`), not old history entries. None by default.
    fn disabled(&self, path: &str) -> bool {
        let _ = path;
        false
    }
}

/// What disabling a file appends to its name, the Launcher's and the core's way.
pub const DISABLED: &str = ".disabled";

/// [`Known`] from what the caller collected.
#[derive(Debug, Clone, Default)]
pub struct KnownFiles {
    files: HashMap<String, KnownFile>,
    packs: HashMap<(String, String), String>,
    disabled: std::collections::HashSet<String>,
}

impl KnownFiles {
    pub fn new() -> KnownFiles {
        KnownFiles::default()
    }

    /// Records the file with `sha1`. Hashes that aren't lowercase hex of the right length
    /// are ignored, so nothing invalid can reach an image this way.
    pub fn add_file(&mut self, sha1: &str, sha256: &str, size: u64) {
        let sha1 = sha1.to_ascii_lowercase();
        if is_sha1(&sha1) && is_sha256(sha256) {
            self.files.insert(
                sha1,
                KnownFile {
                    sha256: sha256.to_string(),
                    size,
                },
            );
        }
    }

    /// Records that the file the records key as `path` is disabled on disk.
    pub fn add_disabled(&mut self, path: &str) {
        self.disabled.insert(path.to_string());
    }

    /// Records the `.mrpack` hash of a Modrinth pack version.
    pub fn add_pack(&mut self, project: &str, version: &str, sha512: &str) {
        if is_sha512(sha512) {
            self.packs.insert(
                (project.to_string(), version.to_string()),
                sha512.to_string(),
            );
        }
    }
}

impl Known for KnownFiles {
    fn file(&self, sha1: &str) -> Option<KnownFile> {
        self.files.get(&sha1.to_ascii_lowercase()).cloned()
    }

    fn pack_sha512(&self, project: &str, version: &str) -> Option<String> {
        self.packs
            .get(&(project.to_string(), version.to_string()))
            .cloned()
    }

    fn disabled(&self, path: &str) -> bool {
        self.disabled.contains(path)
    }
}

/// Nothing known: conversions that need hashes withhold those files.
impl Known for () {
    fn file(&self, _: &str) -> Option<KnownFile> {
        None
    }

    fn pack_sha512(&self, _: &str, _: &str) -> Option<String> {
        None
    }
}

/// Something a conversion couldn't carry into the image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Note {
    /// A file the image doesn't list, by its path in the old format, and why. It is in the
    /// image's `withheld` list too, and kept in the rest.
    NotListed {
        path: String,
        reason: WithheldReason,
    },
    /// Schema-1 backups didn't record the exclude patterns they were taken with, so the
    /// image's `excludes` is empty without meaning that nothing was left out.
    ExcludesNotRecorded,
    /// The server's `level-name` isn't a single world folder that images can name, so its
    /// files are listed where they are on the server's disk, not as a world.
    LevelNotAWorld(String),
    /// Instance settings that are local and never in an image (Java, its arguments, memory,
    /// window, play time…), by field name. They are kept in the rest.
    LocalSettings(Vec<String>),
    /// Files outside the game folder (the Launcher's instance folder: its icon, records,
    /// shared options), kept in the rest. The icon and background also become settings.
    OutsideGameFolder(Vec<String>),
    /// The pack couldn't be named as the image's base: it isn't on Modrinth or CurseForge,
    /// or the SHA-512 of its `.mrpack` isn't known. Its files are listed with their own
    /// sources instead.
    PackNotReferenced { name: String },
    /// A file that would be a client setting (the icon, the background) is larger than an
    /// image's settings may name ([`crate::settings::MAX_STORED_FILE`]): the setting is left
    /// out, and the file stays in the rest.
    SettingFileTooLarge(String),
    /// A path the old format had is longer than an image's withheld list may show
    /// ([`crate::paths::MAX_PATH_BYTES`]): it is only in the rest and this note.
    PathTooLong(String),
}

/// The withheld reason for a file whose bytes no one could hash (see [`Known`]): it
/// couldn't be read.
const UNHASHED: WithheldReason = WithheldReason::Unreadable;

/// Checks that `bytes` is a JSON object of a sane shape ([`prescan`]: an old file from
/// elsewhere, a `.lodesnap`'s manifest, compresses a thousand to one) and returns its
/// whole-number field `key` (a schema or document version), `None` when it is missing.
/// Nothing but that field is kept ([`whole_number_field`]), so a hostile file can't make
/// this cost much more memory than its size.
///
/// [`prescan`]: crate::manifest::prescan
fn version_probe(bytes: &[u8], key: &str) -> Result<Option<u64>, LegacyError> {
    whole_number_field(bytes, key).map_err(|e| match e {
        ProbeError::Shape(e) => LegacyError::Invalid(format!("it {e}")),
        ProbeError::Json(e) => LegacyError::Json(e),
        ProbeError::NotObject => LegacyError::Invalid("it isn't a JSON object".into()),
        ProbeError::NotWhole => LegacyError::Invalid(format!("{key:?} isn't a whole number")),
        ProbeError::NotNumber => LegacyError::Invalid(format!("{key:?} isn't a number")),
    })
}

/// Checks that `bytes` is a JSON object of a sane shape, for files without a version.
fn object_probe(bytes: &[u8]) -> Result<(), LegacyError> {
    version_probe(bytes, "").map(|_| ())
}

/// Free text from an old file as an image's informational text: control characters become
/// spaces and it is cut to [`MAX_TEXT_LEN`] characters, which image validation requires.
fn clean_text(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(MAX_TEXT_LEN)
        .collect()
}

/// `bytes` as lowercase hex (a digest, for an image's hash fields).
fn lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0xf)]));
    }
    out
}

/// Lists a file the image can't hold in its `withheld` list, with a note. A path longer
/// than the list may show ([`MAX_PATH_BYTES`]) is only noted, so one such file doesn't make
/// the whole old file unreadable; the caller keeps it in its rest either way.
fn withhold(image: &mut Image, notes: &mut Vec<Note>, path: &str, reason: WithheldReason) {
    notes.push(Note::NotListed {
        path: path.to_string(),
        reason: reason.clone(),
    });
    if path.len() <= MAX_PATH_BYTES {
        image.withheld.push(Withheld {
            path: path.to_string(),
            reason,
        });
    } else {
        notes.push(Note::PathTooLong(path.to_string()));
    }
}

/// The indices of `paths` that are also the folder of another of them (`w/a` beside
/// `w/a/b`). An image can't hold both, so a conversion withholds these files as bad names
/// rather than failing whole on one odd entry of an old file from elsewhere. Linear after
/// a sort: one binary search per path, as [`Image::validate`] finds them.
fn file_folder_clashes<'p>(paths: impl IntoIterator<Item = &'p str>) -> BTreeSet<usize> {
    let mut sorted: Vec<(&str, usize)> = paths
        .into_iter()
        .enumerate()
        .map(|(index, path)| (path, index))
        .collect();
    sorted.sort_unstable();
    let mut clashes = BTreeSet::new();
    let mut folder = String::new();
    for &(path, index) in &sorted {
        folder.clear();
        folder.push_str(path);
        folder.push('/');
        let next = sorted.partition_point(|(other, _)| *other < folder.as_str());
        if sorted
            .get(next)
            .is_some_and(|(other, _)| other.starts_with(folder.as_str()))
        {
            clashes.insert(index);
        }
    }
    clashes
}

/// Seals `image` and checks it, so a conversion never hands out an invalid image.
fn finish(mut image: Image) -> Result<Image, LegacyError> {
    image.seal();
    image
        .validate()
        .map_err(|e| LegacyError::Invalid(format!("the converted image: {e}")))?;
    Ok(image)
}

/// A small seeded generator for the property-style tests: the same seed always gives the
/// same cases, so a failure can be replayed.
#[cfg(test)]
pub(crate) struct TestRng(u64);

#[cfg(test)]
impl TestRng {
    pub(crate) fn new(seed: u64) -> TestRng {
        TestRng(seed.max(1))
    }

    /// The next number (xorshift64*).
    pub(crate) fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// A number below `n` (`n` > 0).
    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// One of `items`.
    pub(crate) fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_files_ignore_bad_hashes_and_fold_case() {
        let mut known = KnownFiles::new();
        let sha1 = "A".repeat(40);
        known.add_file(&sha1, &"b".repeat(64), 3);
        known.add_file(&"c".repeat(40), "not a hash", 3);
        known.add_pack("p", "v", &"d".repeat(128));
        known.add_pack("p", "w", "short");
        assert_eq!(
            known.file(&"a".repeat(40)),
            Some(KnownFile {
                sha256: "b".repeat(64),
                size: 3
            })
        );
        assert_eq!(known.file(&"c".repeat(40)), None);
        assert_eq!(known.pack_sha512("p", "v"), Some("d".repeat(128)));
        assert_eq!(known.pack_sha512("p", "w"), None);
        assert_eq!(().file(&"a".repeat(40)), None);
    }

    #[test]
    fn files_that_are_also_folders_are_found() {
        let clashes = file_folder_clashes(["w/a/b", "w/a", "w/ab", "w/a.b", "x", "x/y/z"]);
        assert_eq!(clashes.into_iter().collect::<Vec<_>>(), [1, 4]);
    }

    #[test]
    fn text_is_cleaned_for_images() {
        assert_eq!(clean_text("a\nb\u{7}"), "a b ");
        assert_eq!(clean_text(&"x".repeat(5000)).chars().count(), MAX_TEXT_LEN);
    }

    #[test]
    fn only_objects_are_read() {
        assert!(matches!(object_probe(b"[1]"), Err(LegacyError::Invalid(_))));
        assert!(matches!(object_probe(b"7"), Err(LegacyError::Invalid(_))));
        assert!(matches!(object_probe(b"{"), Err(LegacyError::Json(_))));
        assert!(matches!(object_probe(b"nope"), Err(LegacyError::Json(_))));
        assert!(matches!(object_probe(b"{} {}"), Err(LegacyError::Json(_))));
        let doc = br#"{"schema": 1, "bad": -1, "text": "1", "float": 1.5, "deep": {"schema": 9}}"#;
        assert_eq!(version_probe(doc, "schema"), Ok(Some(1)));
        assert_eq!(version_probe(doc, "missing"), Ok(None));
        assert!(version_probe(doc, "bad").is_err());
        assert!(version_probe(doc, "text").is_err());
        assert!(version_probe(doc, "float").is_err());
        assert!(version_probe(doc, "deep").is_err());
        // A shape no real file has is refused before parsing.
        let deep = format!("{{\"x\":{}0{}}}", "[".repeat(40), "]".repeat(40));
        assert!(matches!(
            object_probe(deep.as_bytes()),
            Err(LegacyError::Invalid(_))
        ));
    }
}
