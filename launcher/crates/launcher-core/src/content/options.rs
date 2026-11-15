//! The resource packs in `options.txt`: `resourcePacks:[…]` and
//! `incompatibleResourcePacks:[…]`, JSON lists of pack ids.
//!
//! A pack in `resourcepacks/` has the id `file/<name>` (bare `<name>` before 1.13). The
//! game applies the list in order, so the last pack has the highest priority. Ids that
//! aren't `file/` packs (`vanilla`, `fabric`, packs that mods provide) are kept where they
//! are. The game drops an incompatible pack from `resourcePacks` when it loads the options
//! unless the pack is also in `incompatibleResourcePacks` (where the game's "enable
//! anyway" puts it), so enabling a pack that may not fit adds it there too.
//!
//! Every other line of the file is kept as it was, byte for byte.

use std::{fs, io, path::Path};

use crate::storage;

pub const FILE: &str = "options.txt";
pub const PACKS_KEY: &str = "resourcePacks";
pub const INCOMPATIBLE_KEY: &str = "incompatibleResourcePacks";

/// `options.txt`, as lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OptionsFile {
    lines: Vec<Line>,
    newline: &'static str,
    trailing_newline: bool,
}

/// A line of `options.txt`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Line {
    /// The text; for a line that isn't UTF-8, with U+FFFD for the bytes that aren't.
    text: String,
    /// The bytes of a line that isn't UTF-8 (one an old game wrote in the system's code
    /// page, or a mod), written back as they were unless the line is replaced.
    raw: Option<Vec<u8>>,
}

impl Line {
    fn new(text: String) -> Self {
        Self { text, raw: None }
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        match std::str::from_utf8(bytes) {
            Ok(text) => Self::new(text.to_string()),
            Err(_) => Self {
                text: String::from_utf8_lossy(bytes).into_owned(),
                raw: Some(bytes.to_vec()),
            },
        }
    }

    fn bytes(&self) -> &[u8] {
        self.raw.as_deref().unwrap_or(self.text.as_bytes())
    }
}

impl Default for OptionsFile {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            newline: "\n",
            trailing_newline: true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OptionsError {
    #[error("couldn't read or write {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("the {key} line of options.txt isn't a list Lodestone can read, so it was left alone")]
    Unreadable { key: &'static str },
}

/// Reads a JSON list of strings.
pub fn parse_list(value: &str) -> Option<Vec<String>> {
    serde_json::from_str::<Vec<String>>(value.trim()).ok()
}

/// Writes a list the way the game does: a JSON list of strings without spaces.
pub fn format_list(entries: &[String]) -> String {
    serde_json::to_string(entries).unwrap_or_else(|_| "[]".to_string())
}

impl OptionsFile {
    pub fn parse(text: &str) -> Self {
        Self::parse_bytes(text.as_bytes())
    }

    /// Parses a file that may not be UTF-8: its lines that aren't keep their bytes.
    pub fn parse_bytes(bytes: &[u8]) -> Self {
        let newline = if bytes.windows(2).any(|pair| pair == b"\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let trailing_newline = bytes.is_empty() || bytes.ends_with(b"\n");
        let lines = if bytes.is_empty() {
            Vec::new()
        } else {
            let body = bytes.strip_suffix(b"\n").unwrap_or(bytes);
            let mut lines: Vec<&[u8]> = body.split(|&b| b == b'\n').collect();
            // As `str::lines`: the `\r` of each `\r\n` isn't part of the line.
            let ended = lines.len() - usize::from(!trailing_newline);
            for line in &mut lines[..ended] {
                *line = line.strip_suffix(b"\r").unwrap_or(line);
            }
            lines.into_iter().map(Line::from_bytes).collect()
        };
        Self {
            lines,
            newline,
            trailing_newline,
        }
    }

    /// Reads `path`; a missing file is an empty one.
    pub fn read(path: &Path) -> Result<Self, OptionsError> {
        match fs::read(path) {
            Ok(bytes) => Ok(Self::parse_bytes(&bytes)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(OptionsError::Io {
                path: path.display().to_string(),
                source,
            }),
        }
    }

    /// The file's contents. Lines that weren't UTF-8 are written as they were read.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                bytes.extend_from_slice(self.newline.as_bytes());
            }
            bytes.extend_from_slice(line.bytes());
        }
        if self.trailing_newline && !self.lines.is_empty() {
            bytes.extend_from_slice(self.newline.as_bytes());
        }
        bytes
    }

    /// Writes the file atomically.
    pub fn write(&self, path: &Path) -> Result<(), OptionsError> {
        storage::write_atomic(path, &self.to_bytes()).map_err(|source| OptionsError::Io {
            path: path.display().to_string(),
            source,
        })
    }

    fn line_index(&self, key: &str) -> Option<usize> {
        self.lines
            .iter()
            .position(|line| line.text.split_once(':').is_some_and(|(k, _)| k == key))
    }

    /// The raw value of `key`.
    pub fn get(&self, key: &str) -> Option<&str> {
        let index = self.line_index(key)?;
        self.lines[index]
            .text
            .split_once(':')
            .map(|(_, value)| value)
    }

    /// The list under `key`: `Ok(None)` when the line is missing.
    pub fn list(&self, key: &'static str) -> Result<Option<Vec<String>>, OptionsError> {
        match self.get(key) {
            None => Ok(None),
            Some(value) => parse_list(value)
                .map(Some)
                .ok_or(OptionsError::Unreadable { key }),
        }
    }

    /// Sets `key`'s line, or adds it at the end.
    pub fn set(&mut self, key: &str, value: &str) {
        let line = Line::new(format!("{key}:{value}"));
        match self.line_index(key) {
            Some(index) => self.lines[index] = line,
            None => self.lines.push(line),
        }
    }

    /// Sets `key`'s line, or adds it as the first line (where the game writes
    /// `version`).
    pub fn set_first(&mut self, key: &str, value: &str) {
        let line = Line::new(format!("{key}:{value}"));
        match self.line_index(key) {
            Some(index) => self.lines[index] = line,
            None => self.lines.insert(0, line),
        }
    }

    /// The keys of the `key:value` lines, in file order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.lines
            .iter()
            .filter_map(|line| line.text.split_once(':').map(|(key, _)| key))
    }

    /// Whether the file has no lines.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// The id of the pack file or folder `name` in `options.txt`.
pub fn pack_id(name: &str, game_version: &str) -> String {
    if uses_file_prefix(game_version) {
        format!("file/{name}")
    } else {
        name.to_string()
    }
}

/// Whether the game names pack files `file/<name>` (1.13 and later, and every version that
/// isn't an old release).
fn uses_file_prefix(game_version: &str) -> bool {
    let mut parts = game_version.split('.');
    match (
        parts.next().and_then(|p| p.parse::<u32>().ok()),
        parts.next().and_then(|p| p.parse::<u32>().ok()),
    ) {
        (Some(1), Some(minor)) => minor >= 13,
        _ => true,
    }
}

/// The pack file name an id refers to, if it is a pack from `resourcepacks/`. Built-in and
/// mod-provided ids (`vanilla`, `fabric`, `programmer_art`, …) are `None`.
pub fn pack_name(id: &str, game_version: &str) -> Option<String> {
    match id.strip_prefix("file/") {
        Some(name) => Some(name.to_string()),
        None if !uses_file_prefix(game_version) && id != "vanilla" && id != "programmer_art" => {
            Some(id.to_string())
        }
        None => None,
    }
}

/// The resource packs as `options.txt` has them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackOptions {
    /// The ids in the game's order: the last has the highest priority.
    pub enabled: Vec<String>,
    pub incompatible: Vec<String>,
    /// Whether `options.txt` has a `resourcePacks` line.
    pub has_line: bool,
}

impl PackOptions {
    pub fn read(game_dir: &Path) -> Result<Self, OptionsError> {
        let file = OptionsFile::read(&game_dir.join(FILE))?;
        let enabled = file.list(PACKS_KEY)?;
        Ok(Self {
            has_line: enabled.is_some(),
            enabled: enabled.unwrap_or_default(),
            incompatible: file.list(INCOMPATIBLE_KEY)?.unwrap_or_default(),
        })
    }

    /// The enabled pack files, highest priority first, as the game's screen lists them.
    pub fn enabled_files(&self, game_version: &str) -> Vec<String> {
        self.enabled
            .iter()
            .rev()
            .filter_map(|id| pack_name(id, game_version))
            .collect()
    }

    fn position(&self, name: &str) -> Option<usize> {
        self.enabled
            .iter()
            .position(|id| id == &format!("file/{name}") || id == name)
    }
}

/// How to change the pack list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackChange {
    /// Enables a pack on top (highest priority). `compatible` is whether the pack is known
    /// to fit the game version; otherwise it goes into `incompatibleResourcePacks` too.
    Enable {
        name: String,
        compatible: bool,
    },
    Disable {
        name: String,
    },
    /// Moves a pack one place up (higher priority) or down among the pack files.
    Raise {
        name: String,
    },
    Lower {
        name: String,
    },
    /// A pack's file was replaced by one with another name.
    Rename {
        from: String,
        to: String,
    },
}

/// Applies `change` to the `options.txt` in `game_dir`, creating it (with only the
/// `resourcePacks` line) if it doesn't exist. Lines other than the two lists are kept.
pub fn change_packs(
    game_dir: &Path,
    game_version: &str,
    change: &PackChange,
) -> Result<PackOptions, OptionsError> {
    let path = game_dir.join(FILE);
    let mut file = OptionsFile::read(&path)?;
    let had_line = file.list(PACKS_KEY)?.is_some();
    let mut packs = PackOptions {
        enabled: file
            .list(PACKS_KEY)?
            .unwrap_or_else(|| vec!["vanilla".to_string()]),
        incompatible: file.list(INCOMPATIBLE_KEY)?.unwrap_or_default(),
        has_line: true,
    };
    let had_incompatible_line = file.get(INCOMPATIBLE_KEY).is_some();
    let before = packs.clone();
    apply(&mut packs, game_version, change);
    if packs == before && had_line {
        return Ok(packs);
    }
    file.set(PACKS_KEY, &format_list(&packs.enabled));
    if had_incompatible_line || !packs.incompatible.is_empty() {
        file.set(INCOMPATIBLE_KEY, &format_list(&packs.incompatible));
    }
    file.write(&path)?;
    Ok(packs)
}

/// Rewrites the pack ids of the `options.txt` in `game_dir`, written by Minecraft `from`,
/// as Minecraft `to` names them (`file/<name>` from 1.13, bare `<name>` before), for a
/// copy of the file in an instance of another version. Other ids and lines are kept, and
/// so is a pack line that can't be read (hand-edited, say); a missing file is left
/// missing.
pub fn carry_packs(game_dir: &Path, from: &str, to: &str) -> Result<(), OptionsError> {
    let path = game_dir.join(FILE);
    if uses_file_prefix(from) == uses_file_prefix(to) || !path.is_file() {
        return Ok(());
    }
    let mut file = OptionsFile::read(&path)?;
    for key in [PACKS_KEY, INCOMPATIBLE_KEY] {
        if let Ok(Some(ids)) = file.list(key) {
            let carried: Vec<String> = ids
                .iter()
                .map(|id| pack_name(id, from).map_or_else(|| id.clone(), |name| pack_id(&name, to)))
                .collect();
            file.set(key, &format_list(&carried));
        }
    }
    file.write(&path)
}

fn apply(packs: &mut PackOptions, game_version: &str, change: &PackChange) {
    let matches = |id: &String, name: &str| id == &format!("file/{name}") || id == name;
    match change {
        PackChange::Enable { name, compatible } => {
            let id = pack_id(name, game_version);
            if packs.position(name).is_none() {
                packs.enabled.push(id.clone());
            }
            let listed = packs.incompatible.iter().any(|i| matches(i, name));
            if !compatible && !listed {
                packs.incompatible.push(id);
            }
        }
        PackChange::Disable { name } => {
            packs.enabled.retain(|id| !matches(id, name));
            packs.incompatible.retain(|id| !matches(id, name));
        }
        PackChange::Raise { name } | PackChange::Lower { name } => {
            let Some(from) = packs.position(name) else {
                return;
            };
            let is_file = |id: &String| pack_name(id, game_version).is_some();
            let neighbour = if matches!(change, PackChange::Raise { .. }) {
                (from + 1..packs.enabled.len()).find(|&i| is_file(&packs.enabled[i]))
            } else {
                (0..from).rev().find(|&i| is_file(&packs.enabled[i]))
            };
            if let Some(to) = neighbour {
                packs.enabled.swap(from, to);
            }
        }
        PackChange::Rename { from, to } => {
            let new_id = pack_id(to, game_version);
            for list in [&mut packs.enabled, &mut packs.incompatible] {
                for id in list.iter_mut() {
                    if matches(id, from) {
                        *id = new_id.clone();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPTIONS: &str = "version:3465\r\nautoJump:false\r\nresourcePacks:[\"vanilla\",\"fabric\",\"file/Faithful 32x.zip\",\"file/My \\\"quoted\\\" pack\"]\r\nincompatibleResourcePacks:[]\r\nlang:en_us\r\nunknownKey:[weird value\r\nkey_key.jump:key.keyboard.space\r\n";

    fn game_dir(options: Option<&str>) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        if let Some(options) = options {
            fs::write(dir.path().join(FILE), options).unwrap();
        }
        dir
    }

    fn read(dir: &tempfile::TempDir) -> String {
        fs::read_to_string(dir.path().join(FILE)).unwrap()
    }

    #[test]
    fn reads_packs_with_quotes_and_unknown_ids() {
        let dir = game_dir(Some(OPTIONS));
        let packs = PackOptions::read(dir.path()).unwrap();
        assert_eq!(
            packs.enabled,
            [
                "vanilla",
                "fabric",
                "file/Faithful 32x.zip",
                "file/My \"quoted\" pack"
            ]
        );
        assert_eq!(
            packs.enabled_files("1.20.1"),
            ["My \"quoted\" pack", "Faithful 32x.zip"]
        );
    }

    #[test]
    fn enable_disable_and_reorder_keep_other_lines() {
        let dir = game_dir(Some(OPTIONS));
        let path = dir.path();
        change_packs(
            path,
            "1.20.1",
            &PackChange::Enable {
                name: "New, \"odd\" pack.zip".into(),
                compatible: true,
            },
        )
        .unwrap();
        let text = read(&dir);
        assert!(text.contains(
            "resourcePacks:[\"vanilla\",\"fabric\",\"file/Faithful 32x.zip\",\"file/My \\\"quoted\\\" pack\",\"file/New, \\\"odd\\\" pack.zip\"]\r\n"
        ));
        // every other line is kept as it was
        for line in [
            "version:3465\r\n",
            "autoJump:false\r\n",
            "incompatibleResourcePacks:[]\r\n",
            "lang:en_us\r\n",
            "unknownKey:[weird value\r\n",
            "key_key.jump:key.keyboard.space\r\n",
        ] {
            assert!(text.contains(line), "{line:?} in {text:?}");
        }
        assert!(text.ends_with("\r\n"));
        assert_eq!(text.lines().count(), OPTIONS.lines().count());

        // lowering the top pack swaps it with the next pack file; vanilla and fabric stay
        let packs = change_packs(
            path,
            "1.20.1",
            &PackChange::Lower {
                name: "New, \"odd\" pack.zip".into(),
            },
        )
        .unwrap();
        assert_eq!(
            packs.enabled_files("1.20.1"),
            [
                "My \"quoted\" pack",
                "New, \"odd\" pack.zip",
                "Faithful 32x.zip"
            ]
        );
        let packs = change_packs(
            path,
            "1.20.1",
            &PackChange::Lower {
                name: "Faithful 32x.zip".into(),
            },
        )
        .unwrap();
        // already the lowest pack file: nothing moves past fabric or vanilla
        assert_eq!(&packs.enabled[..2], ["vanilla", "fabric"]);
        let packs = change_packs(
            path,
            "1.20.1",
            &PackChange::Raise {
                name: "Faithful 32x.zip".into(),
            },
        )
        .unwrap();
        assert_eq!(
            packs.enabled_files("1.20.1"),
            [
                "My \"quoted\" pack",
                "Faithful 32x.zip",
                "New, \"odd\" pack.zip"
            ]
        );

        let packs = change_packs(
            path,
            "1.20.1",
            &PackChange::Disable {
                name: "My \"quoted\" pack".into(),
            },
        )
        .unwrap();
        assert_eq!(
            packs.enabled,
            [
                "vanilla",
                "fabric",
                "file/New, \"odd\" pack.zip",
                "file/Faithful 32x.zip"
            ]
        );
        // the file still parses back to the same lists
        assert_eq!(PackOptions::read(path).unwrap().enabled, packs.enabled);
        assert!(read(&dir).contains("unknownKey:[weird value\r\n"));
    }

    #[test]
    fn incompatible_packs_are_listed_so_the_game_keeps_them() {
        let dir = game_dir(Some("resourcePacks:[\"vanilla\"]\nfov:0.5\n"));
        let packs = change_packs(
            dir.path(),
            "1.21.1",
            &PackChange::Enable {
                name: "Old.zip".into(),
                compatible: false,
            },
        )
        .unwrap();
        assert_eq!(packs.incompatible, ["file/Old.zip"]);
        assert_eq!(
            read(&dir),
            "resourcePacks:[\"vanilla\",\"file/Old.zip\"]\nfov:0.5\nincompatibleResourcePacks:[\"file/Old.zip\"]\n"
        );
        // renaming follows the pack in both lists; disabling removes it from both
        change_packs(
            dir.path(),
            "1.21.1",
            &PackChange::Rename {
                from: "Old.zip".into(),
                to: "Old v2.zip".into(),
            },
        )
        .unwrap();
        assert_eq!(
            read(&dir),
            "resourcePacks:[\"vanilla\",\"file/Old v2.zip\"]\nfov:0.5\nincompatibleResourcePacks:[\"file/Old v2.zip\"]\n"
        );
        change_packs(
            dir.path(),
            "1.21.1",
            &PackChange::Disable {
                name: "Old v2.zip".into(),
            },
        )
        .unwrap();
        assert_eq!(
            read(&dir),
            "resourcePacks:[\"vanilla\"]\nfov:0.5\nincompatibleResourcePacks:[]\n"
        );
    }

    #[test]
    fn a_missing_file_is_created_with_only_the_pack_line() {
        let dir = game_dir(None);
        change_packs(
            dir.path(),
            "1.20.1",
            &PackChange::Enable {
                name: "Pack.zip".into(),
                compatible: true,
            },
        )
        .unwrap();
        assert_eq!(
            read(&dir),
            "resourcePacks:[\"vanilla\",\"file/Pack.zip\"]\n"
        );
    }

    #[test]
    fn an_unreadable_list_is_left_alone() {
        let text = "resourcePacks:[\"vanilla\",\nlang:en_us\n";
        let dir = game_dir(Some(text));
        let err = change_packs(
            dir.path(),
            "1.20.1",
            &PackChange::Enable {
                name: "Pack.zip".into(),
                compatible: true,
            },
        )
        .unwrap_err();
        assert!(matches!(err, OptionsError::Unreadable { .. }));
        assert_eq!(read(&dir), text);
    }

    #[test]
    fn old_versions_use_bare_names() {
        assert_eq!(pack_id("Pack.zip", "1.12.2"), "Pack.zip");
        assert_eq!(pack_id("Pack.zip", "1.13"), "file/Pack.zip");
        assert_eq!(pack_id("Pack.zip", "26.3"), "file/Pack.zip");
        assert_eq!(pack_id("Pack.zip", "24w14a"), "file/Pack.zip");
        assert_eq!(pack_name("Pack.zip", "1.8.9").as_deref(), Some("Pack.zip"));
        assert_eq!(pack_name("vanilla", "1.8.9"), None);
        assert_eq!(pack_name("fabric", "1.20.1"), None);
        let dir = game_dir(Some("resourcePacks:[\"A.zip\"]\n"));
        let packs = change_packs(
            dir.path(),
            "1.8.9",
            &PackChange::Enable {
                name: "B.zip".into(),
                compatible: true,
            },
        )
        .unwrap();
        assert_eq!(packs.enabled, ["A.zip", "B.zip"]);
        assert_eq!(packs.enabled_files("1.8.9"), ["B.zip", "A.zip"]);
    }

    #[test]
    fn lines_split_as_str_lines_does() {
        for text in [
            "",
            "\n",
            "a",
            "a\r",
            "a\r\nb",
            "a\n\nb\n",
            "a\r\n\r\n",
            "a\rb\n",
        ] {
            let file = OptionsFile::parse(text);
            let lines: Vec<&str> = file.lines.iter().map(|line| line.text.as_str()).collect();
            assert_eq!(lines, text.lines().collect::<Vec<_>>(), "{text:?}");
        }
        for text in ["", "a\n", "a\r\nb\r\n", "a\nb", "a\n\nb\n"] {
            assert_eq!(OptionsFile::parse(text).to_bytes(), text.as_bytes());
        }
    }

    #[test]
    fn lines_that_arent_utf8_are_kept_byte_for_byte() {
        // `lastServer` and a pack name in Windows-1252, as an old game writes them on
        // Windows.
        let bytes = b"version:1343\r\nlastServer:caf\xe9.example.net\r\nresourcePacks:[\"Pack \xe9t\xe9.zip\"]\r\nfov:0.0\r\n";
        let mut file = OptionsFile::parse_bytes(bytes);
        assert_eq!(file.get("lastServer"), Some("caf\u{fffd}.example.net"));
        assert_eq!(file.to_bytes(), bytes);
        file.set("fov", "0.5");
        let mut expected = bytes.to_vec();
        expected.truncate(expected.len() - b"0.0\r\n".len());
        expected.extend_from_slice(b"0.5\r\n");
        assert_eq!(file.to_bytes(), expected);

        // Through the file on disk: changing another line keeps them.
        let dir = game_dir(None);
        let path = dir.path().join(FILE);
        fs::write(&path, bytes).unwrap();
        let mut file = OptionsFile::read(&path).unwrap();
        file.set_first("version", "1631");
        file.write(&path).unwrap();
        let written = fs::read(&path).unwrap();
        assert!(written.starts_with(b"version:1631\r\nlastServer:caf\xe9.example.net\r\n"));
        assert!(written.ends_with(b"resourcePacks:[\"Pack \xe9t\xe9.zip\"]\r\nfov:0.0\r\n"));
    }

    #[test]
    fn unchanged_lists_are_not_rewritten() {
        let text = "resourcePacks:[ \"vanilla\" ]\n";
        let dir = game_dir(Some(text));
        change_packs(
            dir.path(),
            "1.20.1",
            &PackChange::Disable {
                name: "Missing.zip".into(),
            },
        )
        .unwrap();
        assert_eq!(read(&dir), text);
    }
    #[test]
    fn carried_packs_are_named_as_the_other_version_names_them() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        fs::write(
            &path,
            "fov:0.5\nresourcePacks:[\"vanilla\",\"Faithful.zip\"]\nincompatibleResourcePacks:[\"Faithful.zip\"]\n",
        )
        .unwrap();
        carry_packs(dir.path(), "1.12.2", "1.21.1").unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "fov:0.5\nresourcePacks:[\"vanilla\",\"file/Faithful.zip\"]\nincompatibleResourcePacks:[\"file/Faithful.zip\"]\n"
        );
        carry_packs(dir.path(), "1.21.1", "1.12.2").unwrap();
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("resourcePacks:[\"vanilla\",\"Faithful.zip\"]")
        );
        // the same naming on both sides: the file isn't touched
        let before = fs::read(&path).unwrap();
        carry_packs(dir.path(), "1.12.2", "1.8.9").unwrap();
        assert_eq!(fs::read(&path).unwrap(), before);
        carry_packs(&dir.path().join("none"), "1.12.2", "1.21.1").unwrap();
        // a line that can't be read stays as it is, and the other is carried
        fs::write(
            &path,
            "resourcePacks:[\"vanilla\",\"Faith\nincompatibleResourcePacks:[\"Faithful.zip\"]\n",
        )
        .unwrap();
        carry_packs(dir.path(), "1.12.2", "1.21.1").unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "resourcePacks:[\"vanilla\",\"Faith\nincompatibleResourcePacks:[\"file/Faithful.zip\"]\n"
        );
    }
}
