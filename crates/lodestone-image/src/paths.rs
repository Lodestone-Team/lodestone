//! Canonical paths: how an image names a file, the same on both sides and on every
//! operating system.
//!
//! A [`CanonicalPath`] is relative to the game folder (a server's folder, a client's
//! `minecraft/` folder), `/`-separated, and can be created on Windows, macOS and Linux
//! alike, so an image recorded on one can be placed on any other without two entries
//! landing on the same file or outside the game folder. See [`CanonicalPath::new`] for the
//! rules. [`crate::layout`] maps canonical paths to and from a disk.
//!
//! This module also says what is never recorded ([`always_excluded`]), which side may hold a
//! file ([`placement`]), which files are configs ([`is_config`]), and which names clash on
//! a disk that ignores letter case ([`case_collisions`]).
//!
//! Names that guard something (what is never recorded, Lodestone's own files, which side
//! may hold a file, configs) match whatever their letter case: on Windows and on macOS's
//! default disks `.LODESTONE_CONFIG` and `LOGS/` are `.lodestone_config` and `logs/`.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Side;

/// A path is at most this many bytes long.
pub const MAX_PATH_BYTES: usize = 4096;
/// Each part of a path is at most this many bytes long (the common file system limit).
pub const MAX_COMPONENT_BYTES: usize = 255;

/// Why a path isn't a valid canonical path, or can't be mapped to or from a disk.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("the path is empty")]
    Empty,
    #[error("the path is longer than {MAX_PATH_BYTES} bytes")]
    TooLong,
    #[error("the path starts or ends with `/`, or has an empty part")]
    EmptyComponent,
    #[error("a part of the path is longer than {MAX_COMPONENT_BYTES} bytes")]
    ComponentTooLong,
    #[error("the path has a `.` or `..` part")]
    DotComponent,
    #[error("the path has a character that can't be in a file name: {0:?}")]
    BadCharacter(char),
    /// A character some disks ignore when they compare names (HFS+ ignores zero-width
    /// joiners, direction marks and the byte order mark), or that only changes how a name
    /// is shown: such a name could be another file's there, or look like one.
    #[error("the path has an invisible character, which some disks ignore in names: {0:?}")]
    IgnorableCharacter(char),
    #[error("the path has Windows' reserved name {0:?}")]
    ReservedName(String),
    #[error("a part of the path ends with a space or a dot, which Windows drops")]
    TrailingDotOrSpace,
    #[error("the path isn't in Unicode NFC (it has {0:?}, which NFC would change)")]
    NotNfc(char),
    #[error("{0:?} has the shape of a Windows short (8.3) name, which can stand for another file")]
    ShortName(String),
    #[error("{0:?} differs only in letter case from a name images give a meaning")]
    CaseVariant(String),
    #[error("the path is never recorded in an image")]
    Excluded,
    #[error("the file is never placed on the {0:?} side")]
    WrongSide(Side),
    #[error("the world {0:?} isn't one of this server's worlds")]
    UnknownWorld(String),
    #[error("the path can't be told apart from a world's files in this layout: {0}")]
    LayoutConflict(String),
    #[error("{0:?} can't be a world's folder name on a server")]
    BadWorldName(String),
}

/// A validated canonical path. Compared, sorted and hashed as its UTF-8 bytes, so every
/// computer orders entries the same way.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CanonicalPath(String);

impl CanonicalPath {
    /// Checks `path` and makes it a canonical path. It must be:
    ///
    /// - relative and `/`-separated, without empty, `.` or `..` parts (so it can't leave the
    ///   game folder);
    /// - at most [`MAX_PATH_BYTES`] long, with parts of at most [`MAX_COMPONENT_BYTES`];
    /// - free of NUL and other control characters, `\`, `:` and the other characters
    ///   Windows refuses in names (`<`, `>`, `"`, `|`, `?`, `*`), which also rules out drive
    ///   letters and absolute Windows paths;
    /// - free of Unicode's default-ignorable characters ([`is_default_ignorable`]): HFS+
    ///   ignores some of them in names, so `\u{200C}.lodestone_config` would be
    ///   `.lodestone_config` there, and direction overrides make a name show as another;
    /// - free of Windows' reserved device names (`CON`, `PRN`, `AUX`, `NUL`, `COM0`–`COM9`,
    ///   `LPT0`–`LPT9`, `CONIN$`, `CONOUT$`, also with an extension, in any case) and of
    ///   parts ending with a space or a dot, which Windows silently drops;
    /// - free of names shaped like Windows short (8.3) names (`LODEST~1`, `SERVER~1.PRO`),
    ///   which NTFS can resolve to another file ([`is_short_name_alias`]);
    /// - in Unicode NFC, as far as [`nfc_problem`] can tell;
    /// - not starting with a part that differs only in letter case from `saves` (where a
    ///   client's worlds are): such a path would land among the worlds on a disk that
    ///   ignores case without being a world file.
    pub fn new(path: impl Into<String>) -> Result<CanonicalPath, PathError> {
        let path = path.into();
        validate(&path)?;
        Ok(CanonicalPath(path))
    }

    /// Joins already-valid canonical paths' parts; checks the result.
    pub fn join(&self, rest: &str) -> Result<CanonicalPath, PathError> {
        CanonicalPath::new(format!("{}/{}", self.0, rest))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The parts between `/`s; never empty.
    pub fn components(&self) -> impl Iterator<Item = &str> + Clone {
        self.0.split('/')
    }

    /// The first part (the top-level file or folder).
    pub fn first(&self) -> &str {
        self.0.split('/').next().unwrap_or_default()
    }

    /// The last part (the file's name).
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or_default()
    }

    /// The world a world file belongs to: `<world>` for `saves/<world>/<more>`.
    pub fn world(&self) -> Option<&str> {
        let mut parts = self.components();
        if parts.next() != Some("saves") {
            return None;
        }
        let world = parts.next()?;
        parts.next()?;
        Some(world)
    }

    /// Whether `self` is a folder that `other` is in (`a/b` is a folder of `a/b/c`).
    pub fn is_ancestor_of(&self, other: &CanonicalPath) -> bool {
        other.0.len() > self.0.len()
            && other.0.starts_with(&self.0)
            && other.0.as_bytes()[self.0.len()] == b'/'
    }
}

impl fmt::Display for CanonicalPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CanonicalPath {
    type Error = PathError;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        CanonicalPath::new(path)
    }
}

impl TryFrom<&str> for CanonicalPath {
    type Error = PathError;

    fn try_from(path: &str) -> Result<Self, Self::Error> {
        CanonicalPath::new(path)
    }
}

impl From<CanonicalPath> for String {
    fn from(path: CanonicalPath) -> String {
        path.0
    }
}

impl AsRef<str> for CanonicalPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

fn validate(path: &str) -> Result<(), PathError> {
    if path.is_empty() {
        return Err(PathError::Empty);
    }
    if path.len() > MAX_PATH_BYTES {
        return Err(PathError::TooLong);
    }
    for part in path.split('/') {
        validate_component(part)?;
    }
    let first = path.split('/').next().unwrap_or_default();
    if first != SAVES && same_name(first, SAVES) {
        return Err(PathError::CaseVariant(first.to_string()));
    }
    Ok(())
}

/// The canonical folder of a client's worlds.
const SAVES: &str = "saves";

/// Checks one part of a path (a file or folder name) by the rules of
/// [`CanonicalPath::new`]; a `/` in it is a bad character.
pub fn validate_component(part: &str) -> Result<(), PathError> {
    if part.is_empty() {
        return Err(PathError::EmptyComponent);
    }
    if part.len() > MAX_COMPONENT_BYTES {
        return Err(PathError::ComponentTooLong);
    }
    if part == "." || part == ".." {
        return Err(PathError::DotComponent);
    }
    if let Some(c) = part.chars().find(|&c| {
        c.is_control() || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
    }) {
        return Err(PathError::BadCharacter(c));
    }
    if let Some(c) = part.chars().find(|&c| is_default_ignorable(c)) {
        return Err(PathError::IgnorableCharacter(c));
    }
    if part.ends_with(' ') || part.ends_with('.') {
        return Err(PathError::TrailingDotOrSpace);
    }
    if is_reserved_windows_name(part) {
        return Err(PathError::ReservedName(part.to_string()));
    }
    if is_short_name_alias(part) {
        return Err(PathError::ShortName(part.to_string()));
    }
    if let Some(c) = nfc_problem(part) {
        return Err(PathError::NotNfc(c));
    }
    Ok(())
}

/// Whether `c` is one of Unicode's default-ignorable code points (and the other format
/// characters names never need): invisible, and ignored by some disks when they compare
/// names. HFS+ (Mac OS Extended, still common on external drives) ignores U+200C–U+200F,
/// U+202A–U+202E, U+206A–U+206F and U+FEFF, which is how git's CVE-2014-9390 reached
/// `.git`; the bidirectional overrides also make a name display as another (`evil\u{202E}gpj.exe`).
/// Refusing them keeps every rule that looks at names (protected files, never-recorded
/// files, case clashes) looking at the name the disk sees.
pub fn is_default_ignorable(c: char) -> bool {
    matches!(
        u32::from(c),
        0x00AD
            | 0x034F
            | 0x061C
            | 0x115F..=0x1160
            | 0x17B4..=0x17B5
            | 0x180B..=0x180F
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x206F
            | 0x3164
            | 0xFE00..=0xFE0F
            | 0xFEFF
            | 0xFFA0
            | 0xFFF0..=0xFFF8
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0000..=0xE0FFF
    )
}

/// Whether Windows takes `part` for a device: its name before the first `.` (without
/// trailing spaces) is `CON`, `PRN`, `AUX`, `NUL`, `CONIN$`, `CONOUT$`, or `COM`/`LPT`
/// followed by a digit or a superscript 1, 2 or 3, in any case.
pub fn is_reserved_windows_name(part: &str) -> bool {
    let stem = part
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ');
    let upper = stem.to_ascii_uppercase();
    if matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    for prefix in ["COM", "LPT"] {
        if let Some(rest) = upper.strip_prefix(prefix) {
            let mut chars = rest.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                if c.is_ascii_digit() || matches!(c, '¹' | '²' | '³') {
                    return true;
                }
            }
        }
    }
    false
}

/// Whether `part` has the shape of a Windows short (8.3) name: a name of at most 8 bytes
/// ending in `~` and digits, with at most one extension of at most 3 bytes (`LODEST~1`,
/// `SESSIO~1.LOC`). NTFS gives long names such aliases (on the system drive, where the
/// Launcher's instances live, by default) and opens the long-named file through them, so
/// `LODEST~1` can be `.lodestone_config`. Refusing the shape keeps every rule that looks
/// at names (protected files, never-recorded files, case clashes) looking at the file's
/// real name. Ordinary names rarely have it (`backup~1` does, and is withheld).
pub fn is_short_name_alias(part: &str) -> bool {
    let (stem, extension) = match part.rsplit_once('.') {
        Some((stem, extension)) => (stem, extension),
        None => (part, ""),
    };
    if stem.contains('.') || stem.len() > 8 || extension.len() > 3 {
        return false;
    }
    match stem.rsplit_once('~') {
        Some((_, digits)) => !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

/// A character in `text` that Unicode normalization form C would change, if this crate
/// can tell; `None` when the text looks like NFC.
///
/// Paths must be NFC so that a name typed on macOS and the same name typed on Windows are
/// one entry. Full NFC needs Unicode's composition tables, which no dependency both
/// workspaces share provides, so this refuses what NFC changes in practice instead:
///
/// - combining accents and marks used with Latin, Greek and Cyrillic letters (decomposed
///   `é` is `e` + U+0301; NFC writes U+00E9), and the combining kana voicing marks;
/// - Hangul conjoining jamo, which NFC composes into syllables;
/// - characters NFC always replaces: singletons such as the Kelvin, Ångström and Ohm
///   signs and the CJK compatibility ideographs, and Unicode's composition exclusions.
///
/// Anything else is trusted to be NFC, including what this misses: Arabic hamza and madda
/// above or below an alef (U+0653–U+0655), Indic nukta and two-part vowel signs
/// (Devanagari U+0928 + U+093C, which NFC writes U+0929; Bengali, Oriya, Tamil, Malayalam,
/// Sinhala), and other scripts' compositions. ASCII always is NFC. Text this refuses can't be stored
/// under that name: an image withholds the file (`bad_name`) rather than record a name
/// another computer would spell differently.
pub fn nfc_problem(text: &str) -> Option<char> {
    if text.is_ascii() {
        return None;
    }
    text.chars().find(|&c| changed_by_nfc(c))
}

fn changed_by_nfc(c: char) -> bool {
    let code = u32::from(c);
    matches!(
        code,
        // Combining diacritical marks (and their extensions and supplement), combining
        // marks for symbols, combining half marks.
        0x0300..=0x036F
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF
            | 0xFE20..=0xFE2F
            // Combining kana voiced and semi-voiced sound marks.
            | 0x3099..=0x309A
            // Hangul conjoining jamo.
            | 0x1100..=0x11FF
            | 0xA960..=0xA97F
            | 0xD7B0..=0xD7FF
            // Singletons NFC replaces.
            | 0x0374
            | 0x037E
            | 0x0387
            | 0x1F71 | 0x1F73 | 0x1F75 | 0x1F77 | 0x1F79 | 0x1F7B | 0x1F7D
            | 0x1FBB | 0x1FBE | 0x1FC9 | 0x1FCB | 0x1FD3 | 0x1FDB | 0x1FE3 | 0x1FEB
            | 0x1FEE..=0x1FEF
            | 0x1FF9 | 0x1FFB | 0x1FFD
            | 0x2000..=0x2001
            | 0x2126
            | 0x212A..=0x212B
            | 0x2329..=0x232A
            // CJK compatibility ideographs (a few in this block are NFC-stable; refusing
            // the whole block is the safe side).
            | 0xF900..=0xFAFF
            | 0x2F800..=0x2FA1F
            // Composition exclusions: precomposed letters NFC decomposes.
            | 0x0958..=0x095F
            | 0x09DC..=0x09DD
            | 0x09DF
            | 0x0A33
            | 0x0A36
            | 0x0A59..=0x0A5B
            | 0x0A5E
            | 0x0B5C..=0x0B5D
            | 0x0F43 | 0x0F4D | 0x0F52 | 0x0F57 | 0x0F5C | 0x0F69 | 0x0F73
            | 0x0F75..=0x0F76
            | 0x0F78 | 0x0F81 | 0x0F93 | 0x0F9D | 0x0FA2 | 0x0FA7 | 0x0FAC | 0x0FB9
            | 0x2ADC
            | 0xFB1D | 0xFB1F
            | 0xFB2A..=0xFB36
            | 0xFB38..=0xFB3C
            | 0xFB3E
            | 0xFB40..=0xFB41
            | 0xFB43..=0xFB44
            | 0xFB46..=0xFB4E
            | 0x1D15E..=0x1D164
            | 0x1D1BB..=0x1D1C0
    )
}

/// Top-level folders that are never recorded: logs, crash reports, caches, natives,
/// profiler output, and the Launcher's snapshot staging.
const EXCLUDED_TOP_FOLDERS: [&str; 9] = [
    "logs",
    "crash-reports",
    "cache",
    ".cache",
    "natives",
    ".fabric",
    ".mixin.out",
    "debug",
    ".lodestone-snapshot",
];

/// Whether `path` is never recorded in an image (and never placed from one), whatever the
/// image's own excludes say:
///
/// - logs, crash reports, caches, natives and profiler output (these top-level folders:
///   `logs`, `crash-reports`, `cache`, `.cache`, `natives`, `.fabric`, `.mixin.out`,
///   `debug`), which only matter to the running game;
/// - `session.lock` at any depth, a running game's lock on a world;
/// - Lodestone's working folders: `.lodestone-restore*` at any depth, the Launcher's
///   `.lodestone-snapshot` staging, and the modpack store's `objects`, `staging` and
///   `snapshot` folders in `.lodestone_modpack/`;
/// - `.lodestone_config`, a server's identity.
pub fn always_excluded(path: &CanonicalPath) -> bool {
    let parts: Vec<&str> = path.components().collect();
    excluded_parts(&parts)
}

/// [`always_excluded`] for a path given as its parts, before it is validated, so a file
/// that is never recorded isn't reported for a bad name either.
/// Names match whatever their letter case (see the module).
pub(crate) fn excluded_parts(parts: &[&str]) -> bool {
    if parts
        .iter()
        .any(|part| starts_with_name(part, ".lodestone-restore"))
    {
        return true;
    }
    if parts
        .last()
        .is_some_and(|last| same_name(last, "session.lock"))
    {
        return true;
    }
    match parts {
        [only] => same_name(only, ".lodestone_config"),
        [top, _, ..] if in_names(top, &EXCLUDED_TOP_FOLDERS) => true,
        [top, folder, _, ..] if same_name(top, ".lodestone_modpack") => {
            in_names(folder, &["objects", "staging", "snapshot"])
        }
        _ => false,
    }
}

/// Whether `path` is one of Lodestone's own files (a top-level name starting with
/// `.lodestone`, whatever its letter case). Images from another computer must not write
/// these unless the user may write any file (the core's WriteGlobalFile), and the client
/// view never carries them.
pub fn is_lodestone_path(path: &CanonicalPath) -> bool {
    starts_with_name(path.first(), ".lodestone")
}

/// Which sides a canonical path may be placed on, from the paths table of
/// `docs/instance-images.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Placement {
    /// Mods, configs, worlds and everything else.
    Both,
    /// The server jar, its libraries, `server.properties`, ops, whitelist, bans and the
    /// EULA: never placed on a client.
    ServerOnly,
    /// Client options, resource packs, shader packs, screenshots, the server list, and loose
    /// files in `saves/` (not in a world): never placed on a server.
    ClientOnly,
}

impl Placement {
    pub fn allows(self, side: Side) -> bool {
        match self {
            Placement::Both => true,
            Placement::ServerOnly => side == Side::Server,
            Placement::ClientOnly => side == Side::Client,
        }
    }
}

const SERVER_ONLY_FILES: [&str; 7] = [
    "server.jar",
    "server.properties",
    "ops.json",
    "whitelist.json",
    "banned-players.json",
    "banned-ips.json",
    "eula.txt",
];
const SERVER_ONLY_FOLDERS: [&str; 1] = ["libraries"];
const CLIENT_ONLY_FILES: [&str; 6] = [
    "options.txt",
    "optionsof.txt",
    "optionsshaders.txt",
    "servers.dat",
    "servers.dat_old",
    "realms_persistence.json",
];
const CLIENT_ONLY_FOLDERS: [&str; 3] = ["resourcepacks", "shaderpacks", "screenshots"];

/// Which sides `path` may be placed on. Names match whatever their letter case.
pub fn placement(path: &CanonicalPath) -> Placement {
    let parts: Vec<&str> = path.components().collect();
    match parts.as_slice() {
        [file] if in_names(file, &SERVER_ONLY_FILES) => Placement::ServerOnly,
        [file] if in_names(file, &CLIENT_ONLY_FILES) => Placement::ClientOnly,
        [folder, _, ..] if in_names(folder, &SERVER_ONLY_FOLDERS) => Placement::ServerOnly,
        [folder, _, ..] if in_names(folder, &CLIENT_ONLY_FOLDERS) => Placement::ClientOnly,
        // A world's files are `saves/<world>/<more>`; `saves` itself as a file and loose
        // files in `saves/` only exist on clients (a server keeps worlds at its root).
        [SAVES] | [SAVES, _] => Placement::ClientOnly,
        _ => Placement::Both,
    }
}

/// Top-level folders whose files are configs, which the client view only carries when the
/// owner marked them for players.
const CONFIG_FOLDERS: [&str; 3] = ["config", "defaultconfigs", "kubejs"];

/// Whether `path` is a config: a file in `config/`, `defaultconfigs/` or `kubejs/`
/// (whatever their letter case: `Config/` is `config/` on most players' disks).
pub fn is_config(path: &CanonicalPath) -> bool {
    let mut parts = path.components();
    parts
        .next()
        .is_some_and(|top| in_names(top, &CONFIG_FOLDERS))
        && parts.next().is_some()
}

/// `part` folded the way [`case_collisions`] folds names; ASCII without allocating when it
/// is already lower case.
fn folded(part: &str) -> Cow<'_, str> {
    if part.is_ascii() {
        if part.bytes().any(|b| b.is_ascii_uppercase()) {
            Cow::Owned(part.to_ascii_lowercase())
        } else {
            Cow::Borrowed(part)
        }
    } else {
        Cow::Owned(fold(part))
    }
}

/// Whether two names are one on a disk that ignores letter case (over-folding, like
/// [`case_collisions`]: `ſ` meets `s`).
pub(crate) fn same_name(a: &str, b: &str) -> bool {
    a == b || folded(a) == folded(b)
}

/// Whether `part`, folded, starts with `prefix` (lower-case ASCII).
pub(crate) fn starts_with_name(part: &str, prefix: &str) -> bool {
    folded(part).starts_with(prefix)
}

/// Whether `part`, folded, ends with `suffix` (lower-case ASCII).
pub(crate) fn ends_with_name(part: &str, suffix: &str) -> bool {
    folded(part).ends_with(suffix)
}

/// Whether `part` is one of `names` (lower-case ASCII), ignoring letter case.
pub(crate) fn in_names(part: &str, names: &[&str]) -> bool {
    let part = folded(part);
    names.iter().any(|name| part == *name)
}

/// Folds a name the way a case-insensitive disk might. Over-folding (Unicode lower case of
/// the upper case, so `ß` meets `SS`) reports more clashes than NTFS or APFS would, which
/// is the safe side.
fn fold(part: &str) -> String {
    part.to_uppercase().to_lowercase()
}

/// Pairs of paths that would land on the same file or folder on a disk that ignores letter
/// case (Windows, macOS by default): at the first part where they differ, the two parts are
/// equal ignoring case. So `mods/A.jar` and `mods/a.jar` clash, and so do `Config/x.toml`
/// and `config/y.toml` (one folder on such a disk, two in the image).
///
/// Each pair is (the path seen first, the later one), in the order given; a path is
/// reported once, against the first path it clashes with.
pub fn case_collisions<'a>(
    paths: impl IntoIterator<Item = &'a CanonicalPath>,
) -> Vec<(CanonicalPath, CanonicalPath)> {
    let paths: Vec<&CanonicalPath> = paths.into_iter().collect();
    case_collisions_of(paths.iter().map(|p| p.as_str()))
        .into_iter()
        .map(|(a, b)| (paths[a].clone(), paths[b].clone()))
        .collect()
}

/// [`case_collisions`] over `/`-separated paths that needn't be canonical (a client's disk
/// paths, under `minecraft/`, can be longer than a canonical path may be), by index.
///
/// The folders seen so far form a tree, each node keyed by its parent and its folded name,
/// so memory and time are linear in the paths' total length however deep they are (a
/// table of every folded prefix would hold each deep path's prefixes whole, quadratic in
/// its depth).
pub(crate) fn case_collisions_of<'p>(
    paths: impl IntoIterator<Item = &'p str>,
) -> Vec<(usize, usize)> {
    // (parent node, folded name) → (exact name, its node, the first path through it).
    let mut nodes: HashMap<(usize, String), (&'p str, usize, usize)> = HashMap::new();
    let mut next_node = 1; // 0 is the root.
    let mut clashes = Vec::new();
    for (index, path) in paths.into_iter().enumerate() {
        let mut parent = 0;
        for part in path.split('/') {
            let key = (parent, fold(part));
            match nodes.get(&key) {
                Some(&(exact, _, first)) if exact != part => {
                    clashes.push((first, index));
                    break;
                }
                Some(&(_, node, _)) => parent = node,
                None => {
                    nodes.insert(key, (part, next_node, index));
                    parent = next_node;
                    next_node += 1;
                }
            }
        }
    }
    clashes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(path: &str) -> CanonicalPath {
        CanonicalPath::new(path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    #[test]
    fn accepts_ordinary_paths() {
        for ok in [
            "mods/jei-19.21.jar",
            "config/jei/jei-client.ini",
            "saves/New World/region/r.0.0.mca",
            "saves/Café/level.dat",
            "saves/世界/level.dat",
            "saves/Мир/level.dat",
            "saves/दुनिया/level.dat",
            ".lodestone_modpack/record.json",
            "a.b.c",
            "...x",
            "nulls.txt",
            "console.log",
            "com10",
            "lpt",
            "x/.hidden",
            "emoji 😀.txt",
        ] {
            assert!(CanonicalPath::new(ok).is_ok(), "{ok} should be valid");
        }
    }

    #[test]
    fn refuses_hostile_paths() {
        let cases: &[(&str, PathError)] = &[
            ("", PathError::Empty),
            ("/etc/passwd", PathError::EmptyComponent),
            ("a//b", PathError::EmptyComponent),
            ("a/", PathError::EmptyComponent),
            ("..", PathError::DotComponent),
            ("../x", PathError::DotComponent),
            ("a/../../b", PathError::DotComponent),
            ("./a", PathError::DotComponent),
            ("a/./b", PathError::DotComponent),
            ("a\\..\\b", PathError::BadCharacter('\\')),
            ("C:/Windows", PathError::BadCharacter(':')),
            ("C:", PathError::BadCharacter(':')),
            ("file.txt:stream", PathError::BadCharacter(':')),
            ("a\0b", PathError::BadCharacter('\0')),
            ("a\nb", PathError::BadCharacter('\n')),
            ("a\u{7f}b", PathError::BadCharacter('\u{7f}')),
            ("a\u{85}b", PathError::BadCharacter('\u{85}')),
            ("what?.txt", PathError::BadCharacter('?')),
            ("star*", PathError::BadCharacter('*')),
            ("a<b", PathError::BadCharacter('<')),
            ("a>b", PathError::BadCharacter('>')),
            ("a|b", PathError::BadCharacter('|')),
            ("say \"hi\"", PathError::BadCharacter('"')),
            ("name.", PathError::TrailingDotOrSpace),
            ("name ", PathError::TrailingDotOrSpace),
            ("mods/x.jar./y", PathError::TrailingDotOrSpace),
        ];
        for (path, expected) in cases {
            assert_eq!(
                CanonicalPath::new(*path).as_ref().err(),
                Some(expected),
                "{path:?}"
            );
        }
        assert_eq!(
            CanonicalPath::new("a".repeat(256)),
            Err(PathError::ComponentTooLong)
        );
        assert_eq!(
            CanonicalPath::new(vec!["a"; 2049].join("/")),
            Err(PathError::TooLong)
        );
        assert!(CanonicalPath::new(vec!["a"; 2048].join("/")).is_ok());
    }

    #[test]
    fn refuses_invisible_characters_disks_ignore() {
        // HFS+ ignores these when it compares names, so each would be the protected or
        // never-recorded file there (git's CVE-2014-9390 for `.git`).
        for (path, c) in [
            ("\u{200C}.lodestone_config", '\u{200C}'),
            (".lodestone\u{200D}_modpack/record.json", '\u{200D}'),
            ("\u{FEFF}session.lock", '\u{FEFF}'),
            ("lo\u{200B}gs/latest.log", '\u{200B}'),
            ("sa\u{200E}ves/w/level.dat", '\u{200E}'),
            // A direction override makes `evilgpj.exe` show as `evilexe.jpg`.
            ("mods/evil\u{202E}gpj.exe", '\u{202E}'),
            ("mods/a\u{2060}b.jar", '\u{2060}'),
            ("mods/soft\u{AD}hyphen.jar", '\u{AD}'),
            ("mods/tag\u{E0041}.jar", '\u{E0041}'),
            ("mods/x\u{FE0F}.jar", '\u{FE0F}'),
        ] {
            assert_eq!(
                CanonicalPath::new(path),
                Err(PathError::IgnorableCharacter(c)),
                "{path:?}"
            );
        }
        // Visible characters around them are fine.
        CanonicalPath::new("mods/日本語-mod.jar").unwrap();
        CanonicalPath::new("saves/Ünïcödé/level.dat").unwrap();
    }

    #[test]
    fn case_collisions_stay_linear_for_deep_paths() {
        // Distinct deep paths: a table of every folded prefix would hold each one's
        // prefixes whole (about 4 MiB per path); the tree holds each part once.
        let deep: Vec<CanonicalPath> = (0..200)
            .map(|i| p(&format!("c{i}/{}f", "a/".repeat(2000))))
            .collect();
        let started = std::time::Instant::now();
        assert!(case_collisions(&deep).is_empty());
        let mut clash = deep.clone();
        clash.push(p(&format!("c7/{}F", "a/".repeat(2000))));
        assert_eq!(
            case_collisions(&clash),
            [(deep[7].clone(), clash[200].clone())]
        );
        assert!(started.elapsed() < std::time::Duration::from_secs(20));
        // Indexes over plain text too, longer than a canonical path may be.
        let long = format!("minecraft/{}", "b/".repeat(2100));
        let paths = [
            format!("{long}X"),
            format!("{long}x"),
            "minecraft/y".to_string(),
        ];
        assert_eq!(
            case_collisions_of(paths.iter().map(String::as_str)),
            [(0, 1)]
        );
    }

    #[test]
    fn refuses_windows_device_names() {
        for bad in [
            "con",
            "CON",
            "Con.txt",
            "prn",
            "aux.json",
            "nul",
            "NUL.tar.gz",
            "nul .txt",
            "com0",
            "COM1",
            "com9.log",
            "lpt0",
            "LPT3",
            "COM¹",
            "lpt²",
            "conin$",
            "CONOUT$",
            "mods/con/x.jar",
            "saves/aux/level.dat",
        ] {
            assert!(
                matches!(CanonicalPath::new(bad), Err(PathError::ReservedName(_))),
                "{bad} should be refused"
            );
        }
        for ok in [
            "com",
            "com10",
            "lpt",
            "comx",
            "connect",
            "nul_",
            "auxiliary",
            "con-x",
        ] {
            assert!(!is_reserved_windows_name(ok), "{ok}");
        }
    }

    #[test]
    fn refuses_what_nfc_would_change() {
        // Decomposed é (e + combining acute) is refused; the composed one is fine.
        assert_eq!(
            CanonicalPath::new("saves/Cafe\u{301}"),
            Err(PathError::NotNfc('\u{301}'))
        );
        assert!(CanonicalPath::new("saves/Caf\u{e9}").is_ok());
        // Ångström and Kelvin signs, conjoining jamo, kana voicing mark, compatibility
        // ideograph, composition exclusion.
        for c in [
            '\u{212B}', '\u{212A}', '\u{1100}', '\u{3099}', '\u{F900}', '\u{0958}',
        ] {
            assert_eq!(
                CanonicalPath::new(format!("x{c}")),
                Err(PathError::NotNfc(c)),
                "{c:?}"
            );
        }
        // Precomposed Hangul and the spacing kana marks are NFC.
        assert!(CanonicalPath::new("세계").is_ok());
        assert!(CanonicalPath::new("\u{309B}").is_ok());
    }

    #[test]
    fn serde_validates() {
        let ok: CanonicalPath = serde_json::from_str(r#""mods/a.jar""#).unwrap();
        assert_eq!(ok.as_str(), "mods/a.jar");
        assert_eq!(serde_json::to_string(&ok).unwrap(), r#""mods/a.jar""#);
        assert!(serde_json::from_str::<CanonicalPath>(r#""../a.jar""#).is_err());
        assert!(serde_json::from_str::<CanonicalPath>(r#""a\\b""#).is_err());
    }

    #[test]
    fn parts() {
        let path = p("saves/world/region/r.0.0.mca");
        assert_eq!(path.first(), "saves");
        assert_eq!(path.file_name(), "r.0.0.mca");
        assert_eq!(path.world(), Some("world"));
        assert_eq!(p("saves/world").world(), None);
        assert_eq!(p("mods/a.jar").world(), None);
        assert!(p("saves/world").is_ancestor_of(&path));
        assert!(!p("saves/wor").is_ancestor_of(&path));
        assert!(!path.is_ancestor_of(&path));
        assert_eq!(p("mods").join("a.jar").unwrap(), p("mods/a.jar"));
        assert!(p("mods").join("../x").is_err());
    }

    #[test]
    fn always_excluded_table() {
        let cases = [
            ("logs/latest.log", true),
            ("logs", false),
            ("plugins/x/logs/a.log", false),
            ("crash-reports/crash.txt", true),
            ("cache/x", true),
            (".cache/x", true),
            ("natives/lwjgl.dylib", true),
            (".fabric/remappedJars/x.jar", true),
            (".mixin.out/x", true),
            ("debug/profile.txt", true),
            (".lodestone-snapshot/x", true),
            ("session.lock", true),
            ("saves/world/session.lock", true),
            ("world/session.lock", true),
            (".lodestone-restore-staging-0123/x", true),
            ("saves/w/.lodestone-restore-old-1/x", true),
            (".lodestone_config", true),
            ("world/.lodestone_config", false),
            (".lodestone_modpack/objects/ab/cd", true),
            (".lodestone_modpack/staging/x", true),
            (".lodestone_modpack/snapshot/x", true),
            (".lodestone_modpack/record.json", false),
            ("mods/a.jar", false),
            ("config/cache/x", false),
            // Whatever the letter case: on most disks these are the same files.
            (".LODESTONE_CONFIG", true),
            (".Lodestone_Config", true),
            ("LOGS/latest.log", true),
            ("Crash-Reports/x.txt", true),
            ("saves/w/Session.lock", true),
            ("SESSION.LOCK", true),
            (".LODESTONE-RESTORE-x/y", true),
            ("mods/.Lodestone-Restore-old/a.jar", true),
            (".LODESTONE_MODPACK/Objects/ab/cd", true),
            (".lodeſtone_config", true),
            (".LODESTONE_MODPACK/record.json", false),
        ];
        for (path, excluded) in cases {
            assert_eq!(always_excluded(&p(path)), excluded, "{path}");
        }
    }

    #[test]
    fn refuses_short_name_aliases() {
        for bad in [
            "LODEST~1",
            "lodest~2",
            "SESSIO~1.LOC",
            "SERVER~1.PRO",
            "mods/MOD~12.JAR",
            "~1",
            "a~1.b",
            "saves/WORLD~1/level.dat",
        ] {
            assert!(
                matches!(CanonicalPath::new(bad), Err(PathError::ShortName(_))),
                "{bad} should be refused"
            );
        }
        for ok in [
            "my-backup~2.zip",
            "longername~1",
            "a~1.json5",
            "a~1.b.c",
            "file~",
            "~$doc.docx",
            "a~b",
            "x~1y",
        ] {
            assert!(!is_short_name_alias(ok), "{ok}");
            assert!(CanonicalPath::new(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn refuses_case_variants_of_saves() {
        for bad in ["Saves/w/level.dat", "SAVES", "sAves/x"] {
            assert!(
                matches!(CanonicalPath::new(bad), Err(PathError::CaseVariant(_))),
                "{bad}"
            );
        }
        // Only at the top: deeper, `Saves` is an ordinary folder.
        assert!(CanonicalPath::new("config/Saves/x").is_ok());
        assert!(CanonicalPath::new("saves/Saves/level.dat").is_ok());
    }

    #[test]
    fn placement_table() {
        let cases = [
            ("mods/a.jar", Placement::Both),
            ("config/a.toml", Placement::Both),
            ("defaultconfigs/a.toml", Placement::Both),
            ("kubejs/server_scripts/a.js", Placement::Both),
            ("saves/world/level.dat", Placement::Both),
            ("saves/world", Placement::ClientOnly),
            ("saves", Placement::ClientOnly),
            ("server.jar", Placement::ServerOnly),
            ("libraries/net/x.jar", Placement::ServerOnly),
            ("server.properties", Placement::ServerOnly),
            ("ops.json", Placement::ServerOnly),
            ("whitelist.json", Placement::ServerOnly),
            ("eula.txt", Placement::ServerOnly),
            ("options.txt", Placement::ClientOnly),
            ("servers.dat", Placement::ClientOnly),
            ("resourcepacks/a.zip", Placement::ClientOnly),
            ("shaderpacks/a.zip", Placement::ClientOnly),
            ("screenshots/a.png", Placement::ClientOnly),
            ("config/options.txt", Placement::Both),
            ("libraries", Placement::Both),
            ("plugins/a.jar", Placement::Both),
            ("Server.Properties", Placement::ServerOnly),
            ("OPS.json", Placement::ServerOnly),
            ("Libraries/x.jar", Placement::ServerOnly),
            ("Options.txt", Placement::ClientOnly),
            ("ResourcePacks/a.zip", Placement::ClientOnly),
        ];
        for (path, expected) in cases {
            assert_eq!(placement(&p(path)), expected, "{path}");
        }
        assert!(Placement::ServerOnly.allows(Side::Server));
        assert!(!Placement::ServerOnly.allows(Side::Client));
        assert!(Placement::Both.allows(Side::Client));
    }

    #[test]
    fn configs() {
        assert!(is_config(&p("config/a.toml")));
        assert!(is_config(&p("defaultconfigs/a/b.toml")));
        assert!(is_config(&p("kubejs/startup_scripts/a.js")));
        assert!(!is_config(&p("config")));
        assert!(!is_config(&p("mods/config")));
        assert!(!is_config(&p("options.txt")));
        assert!(is_config(&p("Config/a.toml")));
        assert!(is_config(&p("KUBEJS/x.js")));
    }

    #[test]
    fn lodestone_paths() {
        assert!(is_lodestone_path(&p(".lodestone_config")));
        assert!(is_lodestone_path(&p(".lodestone_modpack/record.json")));
        assert!(!is_lodestone_path(&p("mods/.lodestone_client_mods.json")));
        assert!(is_lodestone_path(&p(".LODESTONE_MODPACK/record.json")));
        assert!(is_lodestone_path(&p(".Lodestone-anything")));
    }

    #[test]
    fn case_collisions_found() {
        let paths = [
            p("mods/a.jar"),
            p("mods/A.jar"),
            p("Config/x.toml"),
            p("config/y.toml"),
            p("config/z.toml"),
            p("saves/STRASSE/level.dat"),
            p("saves/straße/level.dat"),
            p("mods/b.jar"),
        ];
        let clashes = case_collisions(&paths);
        assert_eq!(
            clashes,
            vec![
                (p("mods/a.jar"), p("mods/A.jar")),
                (p("Config/x.toml"), p("config/y.toml")),
                (p("Config/x.toml"), p("config/z.toml")),
                (p("saves/STRASSE/level.dat"), p("saves/straße/level.dat")),
            ]
        );
    }

    #[test]
    fn no_case_collisions_for_distinct_names() {
        let paths = [
            p("mods/a.jar"),
            p("mods/b.jar"),
            p("config/a.jar"),
            p("mods/a.jar.disabled"),
        ];
        assert!(case_collisions(&paths).is_empty());
        // The same path twice is not a case clash (duplicates are checked elsewhere).
        assert!(case_collisions(&[p("x"), p("x")]).is_empty());
    }

    #[test]
    fn case_collision_between_file_and_folder() {
        let clashes = case_collisions(&[p("Data"), p("data/x.dat")]);
        assert_eq!(clashes, vec![(p("Data"), p("data/x.dat"))]);
    }
}
