//! Where a canonical path lives on a disk, and back.
//!
//! | Canonical | Server disk | Client disk |
//! | --- | --- | --- |
//! | `mods/…`, `config/…` and the rest | `<server>/…` | `<instance>/minecraft/…` |
//! | `server.jar`, `libraries/…`, `server.properties`, ops, whitelist… | as is | never placed |
//! | `options.txt`, `resourcepacks/…`, `shaderpacks/…`, `screenshots/…` | never placed | `minecraft/…` |
//! | `saves/<world>/…` | `<world>/…` (Paper: see below) | `minecraft/saves/<world>/…` |
//!
//! Canonical worlds use the vanilla layout: the Nether in `DIM-1` and the End in `DIM1`
//! inside the world. Paper, Spigot and Bukkit split the server's level into three folders,
//! `<level>/`, `<level>_nether/` (with the Nether in `DIM-1/`) and `<level>_the_end/` (the
//! End in `DIM1/`). The split folders' other files (their own `level.dat`, `uid.dat`, Paper's
//! settings) have no vanilla place, so they are kept in the world under `.bukkit_nether/`
//! and `.bukkit_the_end/`, which vanilla servers and clients ignore.
//!
//! [`DiskLayout::to_disk`] and [`DiskLayout::from_disk`] are inverse functions on each side:
//! a path that maps one way maps back to itself. Anything that would break that (a server
//! folder named `saves`, Paper's level with a leftover vanilla `DIM-1`) is an error, so the
//! caller withholds the file instead of recording a path that would come back elsewhere.
//! Disk paths are relative (to the instance folder for a client, the server folder for a
//! server) and `/`-separated; [`join_disk`] makes a file system path of one.
//!
//! A server's world names are compared ignoring letter case wherever a name could be
//! taken for one: on Windows and macOS's default disks `World/level.dat` is in the level
//! `world`, and a top-level file named `world` can't sit beside that folder. Such paths are
//! layout conflicts, both ways, so the mapping stays two-way on every disk.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::paths::{
    excluded_parts, in_names, placement, same_name, starts_with_name, validate_component,
    CanonicalPath, PathError,
};
use crate::Side;

/// A client's game folder inside its instance folder.
pub const CLIENT_GAME_FOLDER: &str = "minecraft";

const NETHER: &str = "DIM-1";
const END: &str = "DIM1";
const BUKKIT_NETHER: &str = ".bukkit_nether";
const BUKKIT_END: &str = ".bukkit_the_end";

/// Top-level server folders that can't be world names: they hold other content, and a
/// world there would make its files and theirs one folder.
const NOT_WORLD_NAMES: [&str; 12] = [
    "saves",
    "mods",
    "config",
    "defaultconfigs",
    "kubejs",
    "libraries",
    "plugins",
    "versions",
    "logs",
    "crash-reports",
    "cache",
    "debug",
];

/// A server folder's worlds: its level (`level-name`) and any other world folders, and
/// whether the level is split the Paper way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerLayout {
    level_name: String,
    worlds: BTreeSet<String>,
    split: bool,
}

impl ServerLayout {
    /// A server with the level `level_name` and the other world folders `other_worlds`
    /// (folders with a `level.dat`, say), named as they are on disk. `split` is for Paper,
    /// Spigot and Bukkit. Refuses world names that can't be told apart from other server
    /// files, and two worlds (or a world and a split level's folder) whose names differ
    /// only in letter case.
    pub fn new(
        level_name: impl Into<String>,
        other_worlds: impl IntoIterator<Item = String>,
        split: bool,
    ) -> Result<ServerLayout, PathError> {
        let level_name = level_name.into();
        let mut worlds = BTreeSet::new();
        check_world_name(&level_name)?;
        worlds.insert(level_name.clone());
        for world in other_worlds {
            check_world_name(&world)?;
            if split
                && (same_name(&world, &format!("{level_name}_nether"))
                    || same_name(&world, &format!("{level_name}_the_end")))
            {
                return Err(PathError::BadWorldName(world));
            }
            if worlds
                .iter()
                .any(|known| known != &world && same_name(known, &world))
            {
                return Err(PathError::BadWorldName(world));
            }
            worlds.insert(world);
        }
        Ok(ServerLayout {
            level_name,
            worlds,
            split,
        })
    }

    pub fn level_name(&self) -> &str {
        &self.level_name
    }

    pub fn is_split(&self) -> bool {
        self.split
    }

    /// Every world folder name, the level included, sorted.
    pub fn worlds(&self) -> impl Iterator<Item = &str> {
        self.worlds.iter().map(String::as_str)
    }

    fn is_world(&self, name: &str) -> bool {
        self.worlds.contains(name)
    }

    fn nether_folder(&self) -> String {
        format!("{}_nether", self.level_name)
    }

    fn end_folder(&self) -> String {
        format!("{}_the_end", self.level_name)
    }

    /// Whether a top-level name is, on a disk that ignores letter case, a world's folder or
    /// one of the split level's: those names belong to the worlds.
    fn is_world_like(&self, name: &str) -> bool {
        self.worlds.iter().any(|world| same_name(world, name))
            || (self.split
                && (same_name(name, &self.nether_folder()) || same_name(name, &self.end_folder())))
    }
}

fn check_world_name(name: &str) -> Result<(), PathError> {
    validate_component(name).map_err(|_| PathError::BadWorldName(name.to_string()))?;
    if in_names(name, &NOT_WORLD_NAMES) || starts_with_name(name, ".lodestone") {
        return Err(PathError::BadWorldName(name.to_string()));
    }
    Ok(())
}

/// Whether `name`, inside a split level, differs only in letter case from one of the
/// folders the split moves (`DIM-1`, `DIM1`, `.bukkit_nether`, `.bukkit_the_end`).
fn split_case_variant(name: &str) -> bool {
    [NETHER, END, BUKKIT_NETHER, BUKKIT_END]
        .iter()
        .any(|special| name != *special && same_name(name, special))
}

/// One side's disk: a client instance, or a server and its worlds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskLayout {
    Client,
    Server(ServerLayout),
}

impl DiskLayout {
    pub fn side(&self) -> Side {
        match self {
            DiskLayout::Client => Side::Client,
            DiskLayout::Server(_) => Side::Server,
        }
    }

    /// Where `path` goes on this disk. Errors when it is never recorded, never placed on
    /// this side, in a world this server doesn't have, or can't be placed without being
    /// read back as another path.
    pub fn to_disk(&self, path: &CanonicalPath) -> Result<String, PathError> {
        let parts: Vec<&str> = path.components().collect();
        if excluded_parts(&parts) {
            return Err(PathError::Excluded);
        }
        if !placement(path).allows(self.side()) {
            return Err(PathError::WrongSide(self.side()));
        }
        match self {
            DiskLayout::Client => Ok(format!("{CLIENT_GAME_FOLDER}/{path}")),
            DiskLayout::Server(server) => server_to_disk(server, path, &parts),
        }
    }

    /// The canonical path of the file at `relative` on this disk, `None` when it is not
    /// part of an image: outside the client's game folder, or never recorded
    /// ([`crate::paths::always_excluded`]). Errors when the name isn't a valid canonical
    /// path, the file is never placed on this side, or the layout can't tell it apart from a
    /// world's files; the caller withholds such files.
    pub fn from_disk(&self, relative: &str) -> Result<Option<CanonicalPath>, PathError> {
        let parts: Vec<&str> = relative.split('/').collect();
        let canonical: Vec<String> = match self {
            DiskLayout::Client => match parts.as_slice() {
                [CLIENT_GAME_FOLDER, rest @ ..] if !rest.is_empty() => {
                    rest.iter().map(|part| part.to_string()).collect()
                }
                _ => return Ok(None),
            },
            DiskLayout::Server(server) => server_from_disk(server, &parts)?,
        };
        let canonical_parts: Vec<&str> = canonical.iter().map(String::as_str).collect();
        if excluded_parts(&canonical_parts) {
            return Ok(None);
        }
        let path = CanonicalPath::new(canonical.join("/"))?;
        if !placement(&path).allows(self.side()) {
            return Err(PathError::WrongSide(self.side()));
        }
        Ok(Some(path))
    }
}

fn server_to_disk(
    server: &ServerLayout,
    path: &CanonicalPath,
    parts: &[&str],
) -> Result<String, PathError> {
    let conflict = || PathError::LayoutConflict(path.to_string());
    if let ["saves", world, rest @ ..] = parts {
        if rest.is_empty() {
            // `saves/<file>` is client-only, refused by placement before this.
            return Err(PathError::WrongSide(Side::Server));
        }
        if !server.is_world(world) {
            return Err(PathError::UnknownWorld(world.to_string()));
        }
        if server.split && *world == server.level_name {
            if rest.first().is_some_and(|first| split_case_variant(first)) {
                return Err(conflict());
            }
            let joined = |folder: String, rest: &[&str]| {
                std::iter::once(folder.as_str())
                    .chain(rest.iter().copied())
                    .collect::<Vec<_>>()
                    .join("/")
            };
            return match rest {
                [NETHER, ..] => Ok(joined(server.nether_folder(), rest)),
                [END, ..] => Ok(joined(server.end_folder(), rest)),
                [BUKKIT_NETHER, more @ ..] => match more {
                    [] | [NETHER, ..] => Err(conflict()),
                    _ => Ok(joined(server.nether_folder(), more)),
                },
                [BUKKIT_END, more @ ..] => match more {
                    [] | [END, ..] => Err(conflict()),
                    _ => Ok(joined(server.end_folder(), more)),
                },
                _ => Ok(joined(world.to_string(), rest)),
            };
        }
        return Ok(std::iter::once(*world)
            .chain(rest.iter().copied())
            .collect::<Vec<_>>()
            .join("/"));
    }
    // Not a world file: it keeps its path, unless that is a world's folder, or inside one,
    // on this disk (ignoring letter case), where reading it back would take it for the
    // world's, or a file couldn't be placed beside the folder.
    if let [first, ..] = parts {
        if server.is_world_like(first) {
            return Err(conflict());
        }
    }
    Ok(path.to_string())
}

fn server_from_disk(server: &ServerLayout, parts: &[&str]) -> Result<Vec<String>, PathError> {
    let conflict = || PathError::LayoutConflict(parts.join("/"));
    let owned = |items: &[&str]| -> Vec<String> { items.iter().map(|s| s.to_string()).collect() };
    let in_world = |world: &str, inner: &[&str]| -> Vec<String> {
        let mut out = vec!["saves".to_string(), world.to_string()];
        out.extend(inner.iter().map(|s| s.to_string()));
        out
    };
    let [first, rest @ ..] = parts else {
        return Ok(owned(parts));
    };
    if rest.is_empty() {
        // A top-level file keeps its name, unless it is named like a world (which
        // `server_to_disk` refuses to place).
        if server.is_world_like(first) {
            return Err(conflict());
        }
        return Ok(owned(parts));
    }
    let level = server.level_name.as_str();
    if server.split {
        if *first == level
            && (matches!(rest[0], NETHER | END | BUKKIT_NETHER | BUKKIT_END)
                || split_case_variant(rest[0]))
        {
            // A vanilla dimension (or our own folder) inside a split level: Paper moves
            // these out when it starts, and they would read back in its other folders.
            return Err(conflict());
        }
        if *first == server.nether_folder() {
            return Ok(match rest {
                [NETHER, ..] => in_world(level, rest),
                _ => {
                    let mut inner = vec![BUKKIT_NETHER];
                    inner.extend_from_slice(rest);
                    in_world(level, &inner)
                }
            });
        }
        if *first == server.end_folder() {
            return Ok(match rest {
                [END, ..] => in_world(level, rest),
                _ => {
                    let mut inner = vec![BUKKIT_END];
                    inner.extend_from_slice(rest);
                    in_world(level, &inner)
                }
            });
        }
    }
    if server.is_world(first) {
        return Ok(in_world(first, rest));
    }
    if server.is_world_like(first) {
        // A world's folder in another letter case: the same folder on most disks, but
        // not a name this layout reads as the world.
        return Err(conflict());
    }
    if same_name(first, "saves") {
        // Canonical `saves/` is where worlds are; a server folder by that name would read
        // back as worlds.
        return Err(conflict());
    }
    Ok(owned(parts))
}

/// `relative` (a `/`-separated disk path from [`DiskLayout::to_disk`]) under `base`.
pub fn join_disk(base: &Path, relative: &str) -> PathBuf {
    let mut joined = base.to_path_buf();
    for part in relative.split('/') {
        joined.push(part);
    }
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(path: &str) -> CanonicalPath {
        CanonicalPath::new(path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    fn vanilla() -> DiskLayout {
        DiskLayout::Server(ServerLayout::new("world", ["old_world".to_string()], false).unwrap())
    }

    fn paper() -> DiskLayout {
        DiskLayout::Server(ServerLayout::new("world", ["creative".to_string()], true).unwrap())
    }

    #[test]
    fn client_mapping() {
        let client = DiskLayout::Client;
        assert_eq!(
            client.to_disk(&p("mods/a.jar")).unwrap(),
            "minecraft/mods/a.jar"
        );
        assert_eq!(
            client
                .to_disk(&p("saves/w/DIM-1/region/r.0.0.mca"))
                .unwrap(),
            "minecraft/saves/w/DIM-1/region/r.0.0.mca"
        );
        assert_eq!(
            client.to_disk(&p("options.txt")).unwrap(),
            "minecraft/options.txt"
        );
        assert_eq!(
            client.to_disk(&p("server.properties")),
            Err(PathError::WrongSide(Side::Client))
        );
        assert_eq!(
            client.to_disk(&p("logs/latest.log")),
            Err(PathError::Excluded)
        );

        assert_eq!(
            client.from_disk("minecraft/mods/a.jar").unwrap(),
            Some(p("mods/a.jar"))
        );
        assert_eq!(client.from_disk("instance.json").unwrap(), None);
        assert_eq!(client.from_disk("icon.png").unwrap(), None);
        assert_eq!(client.from_disk("minecraft").unwrap(), None);
        assert_eq!(client.from_disk("minecraft/logs/latest.log").unwrap(), None);
        assert_eq!(
            client.from_disk("minecraft/logs/bad:name.log").unwrap(),
            None
        );
        assert_eq!(
            client.from_disk("minecraft/saves/w/session.lock").unwrap(),
            None
        );
        assert_eq!(
            client.from_disk("minecraft/server.properties"),
            Err(PathError::WrongSide(Side::Client))
        );
        assert_eq!(
            client.from_disk("minecraft/mods/a:b.jar"),
            Err(PathError::BadCharacter(':'))
        );
        assert_eq!(
            client.from_disk("minecraft/../etc/passwd"),
            Err(PathError::DotComponent)
        );
    }

    #[test]
    fn vanilla_server_mapping() {
        let server = vanilla();
        assert_eq!(server.to_disk(&p("mods/a.jar")).unwrap(), "mods/a.jar");
        assert_eq!(
            server.to_disk(&p("server.properties")).unwrap(),
            "server.properties"
        );
        assert_eq!(
            server
                .to_disk(&p("saves/world/DIM-1/region/r.0.0.mca"))
                .unwrap(),
            "world/DIM-1/region/r.0.0.mca"
        );
        assert_eq!(
            server.to_disk(&p("saves/old_world/level.dat")).unwrap(),
            "old_world/level.dat"
        );
        assert_eq!(
            server.to_disk(&p("saves/elsewhere/level.dat")),
            Err(PathError::UnknownWorld("elsewhere".into()))
        );
        assert_eq!(
            server.to_disk(&p("options.txt")),
            Err(PathError::WrongSide(Side::Server))
        );
        // A non-world file inside a world folder would read back as the world's.
        assert!(matches!(
            server.to_disk(&p("world/level.dat")),
            Err(PathError::LayoutConflict(_))
        ));
        // A top-level file named like a world can't sit beside the world's folder.
        assert!(matches!(
            server.to_disk(&p("world")),
            Err(PathError::LayoutConflict(_))
        ));

        assert_eq!(
            server.from_disk("world/DIM1/region/r.0.0.mca").unwrap(),
            Some(p("saves/world/DIM1/region/r.0.0.mca"))
        );
        assert!(matches!(
            server.from_disk("world"),
            Err(PathError::LayoutConflict(_))
        ));
        assert_eq!(server.from_disk("eula.txt").unwrap(), Some(p("eula.txt")));
        assert_eq!(server.from_disk(".lodestone_config").unwrap(), None);
        assert_eq!(server.from_disk("logs/latest.log").unwrap(), None);
        assert_eq!(server.from_disk("world/session.lock").unwrap(), None);
        assert!(matches!(
            server.from_disk("saves/x/level.dat"),
            Err(PathError::LayoutConflict(_))
        ));
        assert_eq!(
            server.from_disk("options.txt"),
            Err(PathError::WrongSide(Side::Server))
        );
        assert_eq!(server.from_disk(""), Err(PathError::Empty));
        assert_eq!(server.from_disk("/abs"), Err(PathError::EmptyComponent));
    }

    #[test]
    fn paper_split_worlds() {
        let server = paper();
        let cases = [
            ("saves/world/level.dat", "world/level.dat"),
            ("saves/world/region/r.0.0.mca", "world/region/r.0.0.mca"),
            (
                "saves/world/DIM-1/region/r.0.0.mca",
                "world_nether/DIM-1/region/r.0.0.mca",
            ),
            (
                "saves/world/DIM1/region/r.0.0.mca",
                "world_the_end/DIM1/region/r.0.0.mca",
            ),
            (
                "saves/world/.bukkit_nether/level.dat",
                "world_nether/level.dat",
            ),
            ("saves/world/.bukkit_nether/uid.dat", "world_nether/uid.dat"),
            (
                "saves/world/.bukkit_the_end/paper-world.yml",
                "world_the_end/paper-world.yml",
            ),
            (
                "saves/creative/DIM-1/region/r.0.0.mca",
                "creative/DIM-1/region/r.0.0.mca",
            ),
            ("plugins/x.jar", "plugins/x.jar"),
        ];
        for (canonical, disk) in cases {
            assert_eq!(server.to_disk(&p(canonical)).unwrap(), disk, "{canonical}");
            assert_eq!(
                server.from_disk(disk).unwrap(),
                Some(p(canonical)),
                "{disk}"
            );
        }
        // Leftover vanilla dimensions inside a split level can't be recorded.
        for disk in [
            "world/DIM-1/region/r.0.0.mca",
            "world/DIM1/x",
            "world/.bukkit_nether/x",
        ] {
            assert!(
                matches!(server.from_disk(disk), Err(PathError::LayoutConflict(_))),
                "{disk}"
            );
        }
        // Canonical paths that would read back as something else.
        for canonical in [
            "saves/world/.bukkit_nether",
            "saves/world/.bukkit_nether/DIM-1/x",
            "saves/world/.bukkit_the_end/DIM1/x",
            "world_nether/x",
            "world_the_end/x",
        ] {
            assert!(
                matches!(
                    server.to_disk(&p(canonical)),
                    Err(PathError::LayoutConflict(_))
                ),
                "{canonical}"
            );
        }
    }

    #[test]
    fn world_names_match_whatever_their_letter_case() {
        let vanilla = vanilla();
        let paper = paper();
        // Non-world paths in a world's folder (another case) or named like one.
        for (layout, canonical) in [
            (&vanilla, "World/level.dat"),
            (&vanilla, "WORLD"),
            (&vanilla, "Old_World/region/r.0.0.mca"),
            (&paper, "World_Nether/x"),
            (&paper, "world_THE_END"),
            (&paper, "saves/world/dim-1/region/r.0.0.mca"),
            (&paper, "saves/world/.BUKKIT_NETHER/level.dat"),
        ] {
            assert!(
                matches!(
                    layout.to_disk(&p(canonical)),
                    Err(PathError::LayoutConflict(_))
                ),
                "{canonical}"
            );
        }
        // And from the disk, both ways.
        for (layout, disk) in [
            (&vanilla, "World/level.dat"),
            (&vanilla, "WORLD"),
            (&vanilla, "Saves/x/level.dat"),
            (&paper, "World_nether/DIM-1/x"),
            (&paper, "world/dim1/x"),
            (&paper, "world/.Bukkit_The_End/x"),
        ] {
            assert!(
                matches!(layout.from_disk(disk), Err(PathError::LayoutConflict(_))),
                "{disk}"
            );
        }
        // Worlds that differ only in letter case, or clash with the split folders.
        assert!(ServerLayout::new("world", ["World".to_string()], false).is_err());
        assert!(ServerLayout::new("world", ["WORLD_nether".to_string()], true).is_err());
        assert!(ServerLayout::new("Mods", [], false).is_err());
        assert!(ServerLayout::new(".LODESTONE_x", [], false).is_err());
        // Other names in other cases are ordinary.
        assert_eq!(vanilla.to_disk(&p("Mods/a.jar")).unwrap(), "Mods/a.jar");
        assert_eq!(
            vanilla.from_disk("Mods/a.jar").unwrap(),
            Some(p("Mods/a.jar"))
        );
    }

    #[test]
    fn paper_world_converts_to_vanilla_and_client() {
        // A Paper server's level, read from its disk, placed on a vanilla server and a
        // client, and read back from each: the same canonical paths every time.
        let paper_disk = [
            "world/level.dat",
            "world/region/r.0.0.mca",
            "world/datapacks/x.zip",
            "world_nether/DIM-1/region/r.-1.0.mca",
            "world_nether/level.dat",
            "world_nether/uid.dat",
            "world_the_end/DIM1/region/r.0.0.mca",
            "world_the_end/level.dat",
        ];
        let paper = paper();
        let vanilla = vanilla();
        let client = DiskLayout::Client;
        let canonical: Vec<CanonicalPath> = paper_disk
            .iter()
            .map(|d| paper.from_disk(d).unwrap().unwrap())
            .collect();
        let vanilla_disk: Vec<String> = canonical
            .iter()
            .map(|c| vanilla.to_disk(c).unwrap())
            .collect();
        assert_eq!(
            vanilla_disk,
            [
                "world/level.dat",
                "world/region/r.0.0.mca",
                "world/datapacks/x.zip",
                "world/DIM-1/region/r.-1.0.mca",
                "world/.bukkit_nether/level.dat",
                "world/.bukkit_nether/uid.dat",
                "world/DIM1/region/r.0.0.mca",
                "world/.bukkit_the_end/level.dat",
            ]
        );
        for (c, d) in canonical.iter().zip(&vanilla_disk) {
            assert_eq!(vanilla.from_disk(d).unwrap().as_ref(), Some(c));
            let on_client = client.to_disk(c).unwrap();
            assert_eq!(on_client, format!("minecraft/{c}"));
            assert_eq!(client.from_disk(&on_client).unwrap().as_ref(), Some(c));
            // And back to Paper.
            let back = paper.to_disk(c).unwrap();
            assert_eq!(paper.from_disk(&back).unwrap().as_ref(), Some(c));
        }
    }

    #[test]
    fn world_names_checked() {
        for bad in [
            "mods",
            "saves",
            "config",
            "plugins",
            ".lodestone_x",
            "a/b",
            "con",
            "",
        ] {
            assert!(ServerLayout::new(bad, Vec::new(), false).is_err(), "{bad}");
        }
        assert!(ServerLayout::new("world", ["world_nether".to_string()], true).is_err());
        // Without the split, `world_nether` is an ordinary world name.
        assert!(ServerLayout::new("world", ["world_nether".to_string()], false).is_ok());
        let layout = ServerLayout::new("w", ["b".to_string(), "a".to_string()], true).unwrap();
        assert_eq!(layout.worlds().collect::<Vec<_>>(), ["a", "b", "w"]);
        assert_eq!(layout.level_name(), "w");
        assert!(layout.is_split());
    }

    #[test]
    fn windows_names_and_case_on_disk() {
        // Windows names on a disk are refused, not mapped.
        let client = DiskLayout::Client;
        assert!(matches!(
            client.from_disk("minecraft/mods/CON.jar"),
            Err(PathError::ReservedName(_))
        ));
        assert_eq!(
            vanilla().from_disk("mods/x.jar."),
            Err(PathError::TrailingDotOrSpace)
        );
        // `World/…` is the level `world` on a disk that ignores case, and not a name this
        // layout reads as it: a conflict, withheld. Other names keep their case.
        assert!(matches!(
            vanilla().from_disk("World/level.dat"),
            Err(PathError::LayoutConflict(_))
        ));
        assert_eq!(
            vanilla().from_disk("Config/a.toml").unwrap(),
            Some(p("Config/a.toml"))
        );
    }

    #[test]
    fn join_disk_parts() {
        let joined = join_disk(Path::new("/srv/server"), "world_nether/DIM-1/x.mca");
        assert_eq!(
            joined,
            Path::new("/srv/server")
                .join("world_nether")
                .join("DIM-1")
                .join("x.mca")
        );
    }

    /// A small deterministic generator (SplitMix64), so the property tests below need no
    /// dependency and run the same everywhere.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
            items[(self.next() % items.len() as u64) as usize]
        }
    }

    const PARTS: [&str; 24] = [
        "saves",
        "world",
        "creative",
        "old_world",
        "world_nether",
        "world_the_end",
        "DIM-1",
        "DIM1",
        ".bukkit_nether",
        ".bukkit_the_end",
        "mods",
        "config",
        "minecraft",
        "logs",
        "session.lock",
        "region",
        "r.0.0.mca",
        "level.dat",
        "options.txt",
        "server.properties",
        "World",
        "libraries",
        "x",
        "plugins",
    ];

    fn random_path(rng: &mut Rng) -> String {
        let len = 1 + (rng.next() % 5) as usize;
        (0..len)
            .map(|_| rng.pick(&PARTS))
            .collect::<Vec<_>>()
            .join("/")
    }

    #[test]
    fn mapping_is_two_way_on_every_layout() {
        let layouts = [DiskLayout::Client, vanilla(), paper()];
        let mut rng = Rng(0x5EED_1234);
        let mut mapped = [0usize; 3];
        for _ in 0..20_000 {
            let text = random_path(&mut rng);
            for (i, layout) in layouts.iter().enumerate() {
                // Canonical → disk → canonical.
                if let Ok(canonical) = CanonicalPath::new(text.clone()) {
                    if let Ok(disk) = layout.to_disk(&canonical) {
                        mapped[i] += 1;
                        assert_eq!(
                            layout.from_disk(&disk),
                            Ok(Some(canonical.clone())),
                            "{layout:?}: {canonical} → {disk}"
                        );
                    }
                }
                // Disk → canonical → disk.
                if let Ok(Some(canonical)) = layout.from_disk(&text) {
                    assert_eq!(
                        layout.to_disk(&canonical).as_deref(),
                        Ok(text.as_str()),
                        "{layout:?}: {text} → {canonical}"
                    );
                }
            }
        }
        // The generator reaches every layout's mapped paths, not just its errors.
        assert!(mapped.iter().all(|&n| n > 1000), "{mapped:?}");
    }

    #[test]
    fn mapping_is_injective() {
        // No two canonical paths land on one disk path, on any layout.
        let layouts = [DiskLayout::Client, vanilla(), paper()];
        let mut rng = Rng(42);
        for layout in &layouts {
            let mut seen = std::collections::HashMap::new();
            for _ in 0..20_000 {
                let Ok(canonical) = CanonicalPath::new(random_path(&mut rng)) else {
                    continue;
                };
                if let Ok(disk) = layout.to_disk(&canonical) {
                    if let Some(previous) = seen.insert(disk.clone(), canonical.clone()) {
                        assert_eq!(previous, canonical, "{layout:?}: both map to {disk}");
                    }
                }
            }
        }
    }
}
