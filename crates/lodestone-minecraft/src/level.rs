//! What a world's `level.dat` says about it: its name, when it was last played, the game
//! mode, the version that last saved it, whether it is hardcore or allows cheats, and
//! which data packs it turns on and off.
//!
//! `level.dat` is a gzipped [`nbt`](crate::nbt) document with the world's settings
//! under `Data`. See <https://minecraft.wiki/w/Java_Edition_level_format>.

use crate::nbt::{self, Compound, NbtError, Root, Tag};

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
    edit_data(bytes, |data| {
        data.set("LevelName", Tag::String(name.into()));
        Some(())
    })
}

/// Reads `level.dat`, lets `edit` change its `Data`, and writes it back gzipped. `None`
/// if the file doesn't read and write back to the same bytes (so nothing but the edit
/// could change), has no `Data`, or `edit` gives up.
fn edit_data(bytes: &[u8], edit: impl FnOnce(&mut Compound) -> Option<()>) -> Option<Vec<u8>> {
    let unpacked = nbt::unpack(bytes, MAX_LEVEL_DAT_BYTES).ok()?;
    let mut root = nbt::read(&unpacked).ok()?;
    if nbt::write(&root) != unpacked {
        return None;
    }
    let data = root.value.get_mut("Data")?.as_compound_mut()?;
    edit(data)?;
    Some(nbt::gzip(&nbt::write(&root)))
}

/// The data packs a world turns on and off, `Data.DataPacks` in `level.dat` (1.13 and
/// later). Packs are named by source: `vanilla`, `file/<name>` for a pack in the
/// world's `datapacks` folder, and others for packs from mods. Enabled packs are in load
/// order: later ones win.
///
/// A pack in the folder that neither list names is new; the game turns it on the next
/// time it loads the world.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataPackLists {
    pub enabled: Vec<String>,
    pub disabled: Vec<String>,
}

impl DataPackLists {
    /// The id the game gives a pack in the world's `datapacks` folder.
    pub fn file_id(name: &str) -> String {
        format!("file/{name}")
    }

    /// Turns the pack `id` on (last, so it loads over the others) or off.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) {
        self.enabled.retain(|p| p != id);
        self.disabled.retain(|p| p != id);
        if enabled {
            self.enabled.push(id.to_string());
        } else {
            self.disabled.push(id.to_string());
        }
    }
}

fn string_list(compound: &Compound, name: &str) -> Vec<String> {
    match compound.get(name) {
        Some(Tag::List(_, items)) => items.iter().filter_map(Tag::as_str).collect(),
        _ => Vec::new(),
    }
}

fn to_list(items: &[String]) -> Tag {
    Tag::List(
        8,
        items
            .iter()
            .map(|s| Tag::String(s.as_str().into()))
            .collect(),
    )
}

/// The world's data pack lists, or `None` if `level.dat` has none (a world from before
/// 1.13, or one the game hasn't saved since).
pub fn data_packs(root: &Root) -> Option<DataPackLists> {
    let packs = root.value.compound("Data")?.compound("DataPacks")?;
    Some(DataPackLists {
        enabled: string_list(packs, "Enabled"),
        disabled: string_list(packs, "Disabled"),
    })
}

/// `level.dat` with its data pack lists set to `lists`, gzipped. `None` if it has no
/// lists to change (see [`data_packs`]) or doesn't write back to the same bytes.
pub fn set_data_packs(bytes: &[u8], lists: &DataPackLists) -> Option<Vec<u8>> {
    edit_data(bytes, |data| {
        let packs = data.get_mut("DataPacks")?.as_compound_mut()?;
        packs.set("Enabled", to_list(&lists.enabled));
        packs.set("Disabled", to_list(&lists.disabled));
        Some(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn data_pack_lists() {
        let root = read_level(MODERN).unwrap();
        let lists = data_packs(&root).unwrap();
        assert!(lists.enabled.contains(&"vanilla".to_string()), "{lists:?}");
        // before 1.13 there are none, and nothing to change
        assert_eq!(data_packs(&read_level(LEGACY).unwrap()), None);
        assert!(set_data_packs(LEGACY, &lists).is_none());

        let mut changed = lists.clone();
        changed.set_enabled(&DataPackLists::file_id("a.zip"), true);
        changed.set_enabled(&DataPackLists::file_id("b"), false);
        changed.set_enabled(&DataPackLists::file_id("a.zip"), false);
        // the fixture already lists the experimental packs as disabled
        assert!(lists.disabled.contains(&"trade_rebalance".to_string()));
        assert_eq!(
            changed.disabled[lists.disabled.len()..],
            ["file/b", "file/a.zip"]
        );
        assert!(!changed.enabled.contains(&"file/a.zip".to_string()));
        changed.set_enabled("file/b", true);
        assert_eq!(changed.enabled.last().map(String::as_str), Some("file/b"));

        let written = set_data_packs(MODERN, &changed).unwrap();
        let after = read_level(&written).unwrap();
        assert_eq!(data_packs(&after).unwrap(), changed);
        // only the lists changed
        let mut expected = root.clone();
        let packs = expected
            .value
            .get_mut("Data")
            .and_then(Tag::as_compound_mut)
            .and_then(|d| d.get_mut("DataPacks"))
            .and_then(Tag::as_compound_mut)
            .unwrap();
        packs.set("Enabled", to_list(&changed.enabled));
        packs.set("Disabled", to_list(&changed.disabled));
        assert_eq!(after, expected);
        assert_eq!(summarize(&written).unwrap(), summarize(MODERN).unwrap());
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
