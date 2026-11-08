//! What a world's `level.dat` says about it: its name, when it was last played, the game
//! mode, the version that last saved it, and whether it is hardcore or allows cheats.
//!
//! `level.dat` is a gzipped [`nbt`](crate::nbt) document with the world's settings
//! under `Data`. See <https://minecraft.wiki/w/Java_Edition_level_format>.

use crate::nbt::{self, NbtError, Root, Tag};

/// `level.dat` is never this large unpacked; bigger files are refused.
pub const MAX_LEVEL_DAT_BYTES: u64 = 32 * 1024 * 1024;

/// The game mode a world starts players in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl GameMode {
    pub fn from_id(id: i64) -> Option<Self> {
        Some(match id {
            0 => GameMode::Survival,
            1 => GameMode::Creative,
            2 => GameMode::Adventure,
            3 => GameMode::Spectator,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            GameMode::Survival => "Survival",
            GameMode::Creative => "Creative",
            GameMode::Adventure => "Adventure",
            GameMode::Spectator => "Spectator",
        }
    }
}

/// The fields of `level.dat` a world list shows. Each is `None` when the file doesn't
/// have it (old versions lack `Version`, for example).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LevelSummary {
    pub name: Option<String>,
    /// Milliseconds since the Unix epoch.
    pub last_played: Option<i64>,
    pub game_mode: Option<GameMode>,
    pub hardcore: bool,
    /// Whether cheats (commands) are allowed.
    pub allow_commands: Option<bool>,
    /// The version that last saved the world, such as `1.20.1`.
    pub version_name: Option<String>,
    pub data_version: Option<i64>,
    pub snapshot: bool,
}

impl LevelSummary {
    pub fn from_root(root: &Root) -> Option<Self> {
        let data = root.value.compound("Data")?;
        let version = data.compound("Version");
        Some(LevelSummary {
            name: data.string("LevelName").filter(|n| !n.trim().is_empty()),
            last_played: data.int("LastPlayed"),
            game_mode: data.int("GameType").and_then(GameMode::from_id),
            hardcore: data.int("hardcore").unwrap_or(0) != 0,
            allow_commands: data.int("allowCommands").map(|v| v != 0),
            version_name: version
                .and_then(|v| v.string("Name"))
                .filter(|n| !n.is_empty()),
            data_version: data
                .int("DataVersion")
                .or_else(|| version.and_then(|v| v.int("Id"))),
            snapshot: version.and_then(|v| v.int("Snapshot")).unwrap_or(0) != 0,
        })
    }
}

/// Reads `level.dat` (gzipped or not).
pub fn read_level(bytes: &[u8]) -> Result<Root, NbtError> {
    nbt::read(&nbt::unpack(bytes, MAX_LEVEL_DAT_BYTES)?)
}

/// The summary of a `level.dat`, or why it can't be read. A file without `Data` is not a
/// world's `level.dat`.
pub fn summarize(bytes: &[u8]) -> Result<LevelSummary, NbtError> {
    LevelSummary::from_root(&read_level(bytes)?).ok_or(NbtError::NotNbt)
}

/// `level.dat` with the world's name set to `name`, gzipped, or `None` if the file
/// doesn't read and write back to the same bytes (so nothing else in it could change).
pub fn rename(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let unpacked = nbt::unpack(bytes, MAX_LEVEL_DAT_BYTES).ok()?;
    let mut root = nbt::read(&unpacked).ok()?;
    if nbt::write(&root) != unpacked {
        return None;
    }
    let data = root.value.get_mut("Data")?.as_compound_mut()?;
    data.set("LevelName", Tag::String(name.into()));
    Some(nbt::gzip(&nbt::write(&root)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nbt::Compound;

    const MODERN: &[u8] = include_bytes!("../testdata/level/1.21.11.dat");
    const LEGACY: &[u8] = include_bytes!("../testdata/level/1.8.9.dat");

    #[test]
    fn reads_a_1_21_11_world() {
        let summary = summarize(MODERN).unwrap();
        assert_eq!(summary.name.as_deref(), Some("Fixture World"));
        assert_eq!(summary.version_name.as_deref(), Some("1.21.11"));
        assert_eq!(summary.game_mode, Some(GameMode::Survival));
        assert!(!summary.hardcore);
        assert_eq!(summary.allow_commands, Some(false));
        assert!(summary.last_played.unwrap() > 1_700_000_000_000);
        assert!(summary.data_version.unwrap() > 4000);
        assert!(!summary.snapshot);
    }

    #[test]
    fn reads_a_1_8_9_world() {
        // Before 1.9 there is no `Version` compound and no `DataVersion`.
        let summary = summarize(LEGACY).unwrap();
        assert_eq!(summary.name.as_deref(), Some("Old Hardcore"));
        assert_eq!(summary.version_name, None);
        assert_eq!(summary.game_mode, Some(GameMode::Survival));
        assert!(summary.hardcore);
        assert_eq!(summary.allow_commands, Some(true));
        assert_eq!(summary.data_version, None);
    }

    #[test]
    fn the_fixtures_round_trip() {
        for fixture in [MODERN, LEGACY] {
            let unpacked = nbt::unpack(fixture, MAX_LEVEL_DAT_BYTES).unwrap();
            let root = nbt::read(&unpacked).unwrap();
            assert_eq!(nbt::write(&root), unpacked);
        }
    }

    #[test]
    fn renames_only_the_name() {
        let renamed = rename(MODERN, "New Name").unwrap();
        let before = read_level(MODERN).unwrap();
        let after = read_level(&renamed).unwrap();
        assert_eq!(
            summarize(&renamed).unwrap().name.as_deref(),
            Some("New Name")
        );
        let mut expected = before.clone();
        expected
            .value
            .get_mut("Data")
            .and_then(Tag::as_compound_mut)
            .unwrap()
            .set("LevelName", Tag::String("New Name".into()));
        assert_eq!(after, expected);
    }

    #[test]
    fn other_files_are_not_worlds() {
        let root = Root {
            value: Compound(vec![("Other".into(), Tag::Int(1))]),
            ..Root::default()
        };
        let bytes = nbt::gzip(&nbt::write(&root));
        assert!(summarize(&bytes).is_err());
        assert!(rename(&bytes, "x").is_none());
        assert!(summarize(b"PK\x03\x04 a zip").is_err());
    }
}
