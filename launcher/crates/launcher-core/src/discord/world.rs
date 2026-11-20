//! The single-player world the game has open, from its save files: its name and day
//! (`level.dat`), the player's dimension, coordinates and game mode
//! (`playerdata/<uuid>.dat`, else `level.dat`'s `Player`), and their stats
//! (`stats/<uuid>.json`).
//!
//! The game writes them all when it saves: when the player pauses, every few minutes,
//! and when they leave the world. [`WorldWatcher::refresh`] looks at one file's
//! modification time and reads the others only when it changed, so checking often
//! costs one `stat` call.

use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

use lodestone_minecraft::{
    level::{self, GameMode, LevelSummary},
    nbt::{Compound, Tag},
};
use serde_json::Value;

use crate::worlds::SAVES;

/// Save files bigger than this aren't read: a player's data and stats are a few
/// kilobytes, and a world's `level.dat` rarely more.
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// A dimension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dimension {
    Overworld,
    Nether,
    End,
    /// A mod's dimension, by id (`twilightforest:twilight_forest`).
    Other(String),
}

impl Dimension {
    /// From the id 1.16 and later save (`minecraft:the_nether`).
    pub fn from_id(id: &str) -> Self {
        match id.strip_prefix("minecraft:").unwrap_or(id) {
            "overworld" => Dimension::Overworld,
            "the_nether" => Dimension::Nether,
            "the_end" => Dimension::End,
            _ => Dimension::Other(id.to_owned()),
        }
    }

    /// From the number versions before 1.16 save.
    fn from_number(number: i64) -> Self {
        match number {
            -1 => Dimension::Nether,
            0 => Dimension::Overworld,
            1 => Dimension::End,
            other => Dimension::Other(format!("dimension {other}")),
        }
    }

    /// The art asset's key in the Discord application.
    pub fn asset(&self) -> &'static str {
        match self {
            Dimension::Overworld => "overworld",
            Dimension::Nether => "the_nether",
            Dimension::End => "the_end",
            Dimension::Other(_) => "dimension",
        }
    }

    /// A mod dimension's name from its id: `twilightforest:twilight_forest` is
    /// "Twilight Forest". `None` for the game's own, which are translated.
    pub fn modded_name(&self) -> Option<String> {
        let Dimension::Other(id) = self else {
            return None;
        };
        let path = id.rsplit_once(':').map_or(id.as_str(), |(_, path)| path);
        let path = path.rsplit('/').next().unwrap_or(path);
        let words: Vec<String> = path
            .split(['_', '-', ' '])
            .filter(|word| !word.is_empty())
            .map(|word| {
                let mut chars = word.chars();
                chars
                    .next()
                    .map(|first| first.to_uppercase().chain(chars).collect())
                    .unwrap_or_default()
            })
            .collect();
        (!words.is_empty()).then(|| words.join(" "))
    }
}

/// The player, as the world last saved them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerState {
    pub dimension: Option<Dimension>,
    /// X, Y and Z.
    pub position: Option<[f64; 3]>,
    pub game_mode: Option<GameMode>,
}

impl PlayerState {
    fn from_compound(player: &Compound) -> Self {
        let dimension = match player.get("Dimension") {
            Some(Tag::String(id)) => Some(Dimension::from_id(&id.to_string_lossy())),
            Some(tag) => tag.as_i64().map(Dimension::from_number),
            None => None,
        };
        let position = match player.get("Pos") {
            Some(Tag::List(_, items)) if items.len() == 3 => {
                let mut xyz = [0.0; 3];
                let mut all = true;
                for (slot, item) in xyz.iter_mut().zip(items) {
                    match item {
                        Tag::Double(v) if v.is_finite() => *slot = *v,
                        Tag::Float(v) if v.is_finite() => *slot = f64::from(*v),
                        _ => all = false,
                    }
                }
                all.then_some(xyz)
            }
            _ => None,
        };
        PlayerState {
            dimension,
            position,
            game_mode: player.int("playerGameType").and_then(GameMode::from_id),
        }
    }

    /// The block the player stands in.
    pub fn block(&self) -> Option<[i64; 3]> {
        self.position
            .map(|[x, y, z]| [x.floor() as i64, y.floor() as i64, z.floor() as i64])
    }
}

/// A player's stats in the world. Each is `None` when the file doesn't have it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorldStats {
    pub deaths: Option<u64>,
    pub mob_kills: Option<u64>,
    pub player_kills: Option<u64>,
    /// Blocks mined, of every kind.
    pub blocks_mined: Option<u64>,
}

impl WorldStats {
    /// From `stats/<uuid>.json`: 1.13 and later group stats by kind
    /// (`stats."minecraft:custom"."minecraft:deaths"`), earlier versions keep them flat
    /// (`stat.deaths`).
    pub fn parse(json: &Value) -> Self {
        let number = |value: Option<&Value>| value.and_then(Value::as_u64);
        if let Some(stats) = json.get("stats") {
            let custom = stats.get("minecraft:custom");
            let custom = |name: &str| number(custom.and_then(|c| c.get(name)));
            let mined = stats
                .get("minecraft:mined")
                .and_then(Value::as_object)
                .map(|blocks| blocks.values().filter_map(Value::as_u64).sum());
            return WorldStats {
                deaths: custom("minecraft:deaths"),
                mob_kills: custom("minecraft:mob_kills"),
                player_kills: custom("minecraft:player_kills"),
                blocks_mined: mined,
            };
        }
        let Some(flat) = json.as_object() else {
            return WorldStats::default();
        };
        let mined: u64 = flat
            .iter()
            .filter(|(key, _)| key.starts_with("stat.mineBlock."))
            .filter_map(|(_, value)| value.as_u64())
            .sum();
        WorldStats {
            deaths: number(flat.get("stat.deaths")),
            mob_kills: number(flat.get("stat.mobKills")),
            player_kills: number(flat.get("stat.playerKills")),
            blocks_mined: (mined > 0).then_some(mined),
        }
    }
}

/// What a world's save files say.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorldState {
    /// The world's folder in `saves/`.
    pub folder: String,
    pub summary: Option<LevelSummary>,
    /// The in-game day, counting from 1.
    pub day: Option<i64>,
    pub player: Option<PlayerState>,
    pub stats: Option<WorldStats>,
}

impl WorldState {
    /// The name the game shows: `LevelName`, else the folder.
    pub fn name(&self) -> &str {
        self.summary
            .as_ref()
            .and_then(|s| s.name.as_deref())
            .unwrap_or(&self.folder)
    }

    pub fn hardcore(&self) -> bool {
        self.summary.as_ref().is_some_and(|s| s.hardcore)
    }

    /// The player's game mode, else the world's.
    pub fn game_mode(&self) -> Option<GameMode> {
        self.player
            .as_ref()
            .and_then(|p| p.game_mode)
            .or_else(|| self.summary.as_ref().and_then(|s| s.game_mode))
    }
}

/// The world the game has open in `game_dir`: the one whose `session.lock` the game
/// wrote last (it does as it opens a world), else whose `level.dat` it wrote last.
pub fn open_world(game_dir: &Path) -> Option<PathBuf> {
    let saves = game_dir.join(SAVES);
    let mut best: Option<(SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(&saves).ok()?.flatten() {
        let dir = entry.path();
        let modified = ["session.lock", "level.dat"]
            .iter()
            .filter_map(|file| fs::metadata(dir.join(file)).ok()?.modified().ok())
            .max();
        if let Some(modified) = modified
            && best.as_ref().is_none_or(|(at, _)| modified > *at)
        {
            best = Some((modified, dir));
        }
    }
    best.map(|(_, dir)| dir)
}

/// Follows one world's save files.
#[derive(Debug)]
pub struct WorldWatcher {
    dir: PathBuf,
    /// `level.dat`'s modification time and size when last read.
    read_at: Option<(SystemTime, u64)>,
}

impl WorldWatcher {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir, read_at: None }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The world's state, if it saved since the last call (and on the first).
    pub fn refresh(&mut self) -> Option<WorldState> {
        let meta = fs::metadata(self.dir.join("level.dat")).ok()?;
        let stamp = (meta.modified().ok()?, meta.len());
        if self.read_at == Some(stamp) {
            return None;
        }
        self.read_at = Some(stamp);
        Some(read_world(&self.dir))
    }
}

/// Reads what the world's files say, leaving out what can't be read.
pub fn read_world(dir: &Path) -> WorldState {
    let folder = dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let level = read_small(&dir.join("level.dat")).and_then(|bytes| level::read_level(&bytes).ok());
    let data = level.as_ref().and_then(|root| root.value.compound("Data"));
    let summary = level.as_ref().and_then(LevelSummary::from_root);
    let day = data
        .and_then(|data| data.int("DayTime"))
        .filter(|ticks| *ticks >= 0)
        .map(|ticks| ticks / 24_000 + 1);
    let player = newest(&dir.join("playerdata"), "dat")
        .and_then(|path| read_small(&path))
        .and_then(|bytes| level::read_level(&bytes).ok())
        .map(|root| PlayerState::from_compound(&root.value))
        .or_else(|| {
            data.and_then(|data| data.compound("Player"))
                .map(PlayerState::from_compound)
        });
    let stats = newest(&dir.join("stats"), "json")
        .and_then(|path| read_small(&path))
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .map(|json| WorldStats::parse(&json));
    WorldState {
        folder,
        summary,
        day,
        player,
        stats,
    }
}

/// The file in `dir` with extension `ext` written last.
fn newest(dir: &Path, ext: &str) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|e| e == ext))
        .filter_map(|entry| Some((entry.metadata().ok()?.modified().ok()?, entry.path())))
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

fn read_small(path: &Path) -> Option<Vec<u8>> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        return None;
    }
    fs::read(path).ok()
}

#[cfg(test)]
mod tests {
    use lodestone_minecraft::nbt::{self, NbtString, Root};
    use serde_json::json;

    use super::*;

    fn compound(entries: Vec<(&str, Tag)>) -> Compound {
        Compound(
            entries
                .into_iter()
                .map(|(name, tag)| (NbtString::from(name), tag))
                .collect(),
        )
    }

    fn write_nbt(path: &Path, value: Compound) {
        let root = Root {
            name: NbtString::from(""),
            value,
        };
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, nbt::gzip(&nbt::write(&root))).unwrap();
    }

    fn player(dimension: Tag) -> Compound {
        compound(vec![
            ("Dimension", dimension),
            (
                "Pos",
                Tag::List(
                    6,
                    vec![Tag::Double(120.7), Tag::Double(64.0), Tag::Double(-30.2)],
                ),
            ),
            ("playerGameType", Tag::Int(1)),
        ])
    }

    #[test]
    fn reads_a_world() {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path().join("saves").join("My World");
        write_nbt(
            &world.join("level.dat"),
            compound(vec![(
                "Data",
                Tag::Compound(compound(vec![
                    ("LevelName", Tag::String("Survival Island".into())),
                    ("hardcore", Tag::Byte(1)),
                    ("DayTime", Tag::Long(24_000 * 41 + 500)),
                    ("Player", Tag::Compound(player(Tag::Int(0)))),
                ])),
            )]),
        );
        write_nbt(
            &world.join("playerdata").join("abc.dat"),
            player(Tag::String("minecraft:the_nether".into())),
        );
        fs::create_dir_all(world.join("stats")).unwrap();
        fs::write(
            world.join("stats").join("abc.json"),
            json!({ "stats": {
                "minecraft:custom": { "minecraft:deaths": 3, "minecraft:mob_kills": 54 },
                "minecraft:mined": { "minecraft:stone": 100, "minecraft:dirt": 20 },
            }})
            .to_string(),
        )
        .unwrap();

        assert_eq!(open_world(dir.path()), Some(world.clone()));
        let mut watcher = WorldWatcher::new(world);
        let state = watcher.refresh().unwrap();
        assert!(
            watcher.refresh().is_none(),
            "unchanged files aren't read again"
        );
        assert_eq!(state.name(), "Survival Island");
        assert!(state.hardcore());
        assert_eq!(state.day, Some(42));
        let player = state.player.as_ref().unwrap();
        assert_eq!(player.dimension, Some(Dimension::Nether), "playerdata wins");
        assert_eq!(player.block(), Some([120, 64, -31]));
        assert_eq!(state.game_mode(), Some(GameMode::Creative));
        let stats = state.stats.unwrap();
        assert_eq!(stats.deaths, Some(3));
        assert_eq!(stats.mob_kills, Some(54));
        assert_eq!(stats.blocks_mined, Some(120));
    }

    #[test]
    fn reads_old_players_and_stats() {
        let player = PlayerState::from_compound(&player(Tag::Int(1)));
        assert_eq!(player.dimension, Some(Dimension::End));
        let stats = WorldStats::parse(&json!({
            "stat.deaths": 2, "stat.mobKills": 7, "stat.mineBlock.minecraft.stone": 5,
        }));
        assert_eq!(stats.deaths, Some(2));
        assert_eq!(stats.mob_kills, Some(7));
        assert_eq!(stats.blocks_mined, Some(5));
    }

    #[test]
    fn names_mod_dimensions() {
        assert_eq!(
            Dimension::from_id("minecraft:overworld"),
            Dimension::Overworld
        );
        assert_eq!(
            Dimension::from_id("twilightforest:twilight_forest").modded_name(),
            Some("Twilight Forest".into())
        );
        assert_eq!(Dimension::Nether.modded_name(), None);
    }
}
