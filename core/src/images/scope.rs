//! Which files of a server are content: what a content image records.
//!
//! The spec splits an instance into content (mods, configs, versions, settings), recorded
//! at every change, and data ("worlds and everything else"), recorded by backups. So
//! content is an allowlist: a file is content only when a rule here names it, and anything
//! a server or a plugin writes elsewhere stays data. Getting this wrong never loses a file
//! (backups take everything); it only decides what goes into content images.
//!
//! Content is:
//!
//! - everything in `mods/`, `config/`, `defaultconfigs/`, `kubejs/`, `libraries/` and
//!   `versions/`;
//! - plugin jars (`plugins/*.jar`, `plugins/*.jar.disabled`) and the config files directly
//!   inside a plugin's own folder (`plugins/<name>/*.yml` and the like). Deeper plugin
//!   folders hold what plugins write as the game runs (CoreProtect's database, Essentials'
//!   player files), which would churn the lineage and bloat the store;
//! - the server's own top-level files: jars, `server.properties`, ops and whitelist,
//!   Forge's start scripts and JVM arguments, and the Bukkit family's settings. Not
//!   `eula.txt`: accepting the EULA is server state, not a content choice.
//!
//! Never content: Lodestone's own files (any name starting with `.lodestone`, at any depth:
//! the modpack update's working files in `.lodestone_modpack/`, the instance's local
//! settings, the "For players" marks,
//! which a content image carries as each file's `owner_mark` instead) and world files,
//! including those of a level that `level-name` puts inside a content folder
//! (`level-name=mods/survival`): the disk layout can't take such a level for a world, so
//! [`ContentFilter`] keeps it out by name (worlds never go to players).
//!
//! Names match ignoring ASCII letter case, as Windows and macOS's default disks do; the
//! `.lodestone` prefix is folded fully, so `.LodeſTone…` is Lodestone's too (NTFS
//! upper-cases `ſ` to `S`).

use lodestone_image::store::snapshot::Candidate;
use lodestone_image::CanonicalPath;

/// The version of these rules. A baseline taken under other rules is out of date (see
/// [`super::state::ContentState::scope_rules`]).
pub const SCOPE_RULES: u32 = 2;

/// Folders whose every file is content.
const CONTENT_FOLDERS: [&str; 6] = [
    "mods",
    "config",
    "defaultconfigs",
    "kubejs",
    "libraries",
    "versions",
];

/// Top-level files that are content (besides any `*.jar`).
const CONTENT_FILES: [&str; 14] = [
    "server.properties",
    "ops.json",
    "whitelist.json",
    "run.sh",
    "run.bat",
    "user_jvm_args.txt",
    "bukkit.yml",
    "spigot.yml",
    "paper.yml",
    "purpur.yml",
    "pufferfish.yml",
    "commands.yml",
    "help.yml",
    "permissions.yml",
];

const PLUGINS: &str = "plugins";

/// The server's settings and admin lists among the content files: Lodestone's settings and
/// the server's own commands (`/op`, `/whitelist`) change them, and a server's own writes
/// never make a content image, so a content image may hold older copies than a backup
/// does. Restoring mods never brings them back (an op removed since must stay removed).
const SERVER_SETTINGS_FILES: [&str; 3] = ["server.properties", "ops.json", "whitelist.json"];

/// Whether `path` is one of the server's settings and admin lists
/// ([`SERVER_SETTINGS_FILES`]), any letter case.
pub fn is_server_settings_file(path: &CanonicalPath) -> bool {
    let mut parts = path.components();
    match (parts.next(), parts.next()) {
        (Some(name), None) => SERVER_SETTINGS_FILES.iter().any(|file| same(name, file)),
        _ => false,
    }
}

/// The extensions of a plugin's config files, directly in its folder.
const PLUGIN_CONFIG_EXTENSIONS: [&str; 9] = [
    "yml",
    "yaml",
    "toml",
    "json",
    "json5",
    "properties",
    "conf",
    "cfg",
    "txt",
];

/// Whether the file (or, with `is_dir`, anything in the folder) at `path` is content.
/// False for a folder means nothing in it is, so a walk can skip it.
pub fn is_content(path: &CanonicalPath, is_dir: bool) -> bool {
    let parts: Vec<&str> = path.components().collect();
    is_content_parts(&parts, is_dir)
}

/// [`is_content`] for a snapshot's filter. A path the layout can't map (no canonical path)
/// is judged by its disk path's parts, so a mod with a name no image can hold
/// (`mods/a:b.jar`) is withheld from the content image rather than silently missing, while
/// a world's split folders (`world_nether/`) and a stray `saves/` folder are skipped.
pub fn content_filter(candidate: &Candidate) -> bool {
    match candidate.path {
        Some(path) => is_content(path, candidate.is_dir),
        None => {
            let parts: Vec<&str> = candidate.disk.split('/').collect();
            // A canonical `saves/…` is a world; on a server's disk, a folder by that name
            // is an odd one, but nothing in it is content either way.
            !parts.first().is_some_and(|first| same(first, "saves"))
                && is_content_parts(&parts, candidate.is_dir)
        }
    }
}

/// Folders and files a vanilla level holds directly in its folder: what is kept out of
/// content for a level whose folder is a content folder itself (`level-name=mods`), where
/// the rest of the folder stays content.
const LEVEL_PARTS: [&str; 16] = [
    "level.dat",
    "level.dat_old",
    "uid.dat",
    "icon.png",
    "region",
    "entities",
    "poi",
    "playerdata",
    "advancements",
    "stats",
    "data",
    "datapacks",
    "dimensions",
    "serverconfig",
    "DIM-1",
    "DIM1",
];

/// The content filter of one server: [`content_filter`], less the files of a level that
/// `level-name` puts inside a content folder (see the module). A record and the check
/// before a start use the same one, so they agree on what is content.
#[derive(Debug, Clone, Default)]
pub struct ContentFilter {
    /// The level's folder as disk path parts, when it is inside a content folder.
    level: Option<Vec<String>>,
}

impl ContentFilter {
    /// The filter for a server whose `level-name` is `level_name`.
    pub fn new(level_name: &str) -> ContentFilter {
        let parts: Vec<String> = level_name
            .split(['/', '\\'])
            .filter(|part| !part.is_empty() && *part != ".")
            .map(str::to_string)
            .collect();
        let inside = !parts.iter().any(|part| part == "..")
            && parts
                .first()
                .is_some_and(|first| CONTENT_FOLDERS.iter().any(|folder| same(first, folder)));
        ContentFilter {
            level: inside.then_some(parts),
        }
    }

    /// Whether the walk offers content (see [`SnapshotOptions::filter`]).
    ///
    /// [`SnapshotOptions::filter`]: lodestone_image::store::snapshot::SnapshotOptions::filter
    pub fn accepts(&self, candidate: &Candidate) -> bool {
        content_filter(candidate) && !self.in_level(candidate.disk)
    }

    /// Whether the disk path `disk` is the level's or in it.
    fn in_level(&self, disk: &str) -> bool {
        let Some(level) = &self.level else {
            return false;
        };
        let parts: Vec<&str> = disk.split('/').collect();
        let under = |n: usize| {
            parts.len() >= n
                && level
                    .iter()
                    .zip(&parts)
                    .take(n)
                    .all(|(level, part)| same(level, part))
        };
        if level.len() > 1 {
            under(level.len())
        } else {
            // the content folder itself: only what a level keeps there
            under(1)
                && parts
                    .get(1)
                    .is_some_and(|part| LEVEL_PARTS.iter().any(|name| same(part, name)))
        }
    }
}

/// Whether the canonical exclude `pattern` (`lodestone_image::store::excludes` syntax)
/// could match a content file or a folder that can hold one. False only for a pattern
/// anchored under a literal path where nothing is content (a world's `saves/<level>/…`,
/// `/logs/`, a plugin's data folder): a content image's excludes leave such patterns out, so
/// editing them in the backup policy changes neither a content image's id nor the check
/// before a start. Names match ignoring letter case here, so the answer may only err
/// towards true.
pub fn exclude_reaches_content(pattern: &str) -> bool {
    let body = pattern.trim_start_matches('/').trim_end_matches('/');
    let anchored = pattern.starts_with('/') || body.contains('/');
    if !anchored {
        // A name at any depth.
        return true;
    }
    let parts: Vec<&str> = body.split('/').collect();
    let literal = parts.iter().take_while(|part| !part.contains('*')).count();
    if literal == 0 {
        return true;
    }
    let prefix = &parts[..literal];
    if literal == parts.len() && !pattern.ends_with('/') && is_content_parts(prefix, false) {
        // The file itself.
        return true;
    }
    // The folder, or anything under it.
    is_content_parts(prefix, true)
}

/// Whether the canonical exclude `pattern` could match the `mods/` folder or anything in
/// it: a name at any depth (`*.jar`, `cache/`), or a pattern from the game folder whose
/// first part is or may be `mods` (`mods/`, `/*/x.jar`, `**/x`). Content images leave such
/// patterns out (see [`super::recorder::content_scope`]): the client view players get is a
/// projection of the content image, so a backup exclude that reached `mods/` would take the
/// server's mods away from players. Ignores letter case, so it errs only towards true.
pub fn exclude_reaches_mods(pattern: &str) -> bool {
    let body = pattern.trim_start_matches('/').trim_end_matches('/');
    let anchored = pattern.starts_with('/') || body.contains('/');
    if !anchored {
        return true;
    }
    let first = body.split('/').next().unwrap_or_default();
    first == "**"
        || lodestone_image::store::excludes::glob_matches(&first.to_ascii_lowercase(), "mods")
}

fn is_content_parts(parts: &[&str], is_dir: bool) -> bool {
    if parts.is_empty() || parts.iter().any(|part| is_lodestone_name(part)) {
        return false;
    }
    let first = parts[0];
    if CONTENT_FOLDERS.iter().any(|folder| same(first, folder)) {
        // The folder itself, or anything in it.
        return is_dir || parts.len() > 1;
    }
    if same(first, PLUGINS) {
        return match (parts.len(), is_dir) {
            (1, true) => true,
            (1, false) => false,
            // A plugin's own folder, or a jar.
            (2, true) => true,
            (2, false) => is_plugin_jar(parts[1]),
            // Config files directly in a plugin's folder.
            (3, false) => is_plugin_config(parts[2]),
            _ => false,
        };
    }
    if parts.len() == 1 && !is_dir {
        return ends_with(first, ".jar") || CONTENT_FILES.iter().any(|name| same(first, name));
    }
    false
}

fn is_plugin_jar(name: &str) -> bool {
    ends_with(name, ".jar") || ends_with(name, ".jar.disabled")
}

fn is_plugin_config(name: &str) -> bool {
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => PLUGIN_CONFIG_EXTENSIONS
            .iter()
            .any(|known| same(extension, known)),
        _ => false,
    }
}

/// Whether `name` is one of Lodestone's own (`.lodestone…`), folded the way a disk that
/// ignores letter case might.
pub fn is_lodestone_name(name: &str) -> bool {
    name.to_uppercase().to_lowercase().starts_with(".lodestone")
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn ends_with(name: &str, suffix: &str) -> bool {
    name.len() > suffix.len()
        && name
            .get(name.len() - suffix.len()..)
            .is_some_and(|end| end.eq_ignore_ascii_case(suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(path: &str) -> CanonicalPath {
        CanonicalPath::new(path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    #[test]
    fn content_is_an_allowlist() {
        for yes in [
            "mods/jei.jar",
            "mods/jei.jar.disabled",
            "mods/sub/x.jar",
            "config/jei/settings.toml",
            "defaultconfigs/x.toml",
            "kubejs/server_scripts/a.js",
            "libraries/net/minecraft/server.jar",
            "versions/1.21.1/server-1.21.1.jar",
            "plugins/WorldEdit.jar",
            "plugins/Old.JAR.disabled",
            "plugins/Essentials/config.yml",
            "plugins/LuckPerms/config.YAML",
            "server.jar",
            "forge-1.20.1-47.2.0.jar",
            "server.properties",
            "ops.json",
            "whitelist.json",
            "run.sh",
            "user_jvm_args.txt",
            "bukkit.yml",
            "Spigot.yml",
            "paper.yml",
        ] {
            assert!(is_content(&p(yes), false), "{yes}");
        }
        for no in [
            "eula.txt",
            "saves/world/level.dat",
            "saves/world/datapacks/x.zip",
            "plugins/Essentials/userdata/abc.yml",
            "plugins/CoreProtect/database.db",
            "plugins/readme",
            "plugins/x.zip",
            "plugins/.lodestone_x.jar",
            "usercache.json",
            "banned-players.json",
            "dynmap/web/tiles/a.png",
            "mods/.lodestone_client_mods.json",
            "mods/.LODESTONE_client_mods.json",
            ".lodestone_minecraft_config.json",
            ".lodestone_modpack/update.json",
            "world-backup.zip",
            "server.jar.bak",
        ] {
            assert!(!is_content(&p(no), false), "{no}");
        }
        // Folders: false only when nothing inside can be content.
        for (folder, content) in [
            ("mods", true),
            ("plugins", true),
            ("plugins/Essentials", true),
            ("plugins/Essentials/userdata", false),
            ("saves", false),
            ("saves/world", false),
            ("dynmap", false),
            (".lodestone_modpack", false),
            ("config", true),
        ] {
            assert_eq!(is_content(&p(folder), true), content, "{folder}/");
        }
    }

    #[test]
    fn excludes_that_can_reach_content() {
        for yes in [
            "dynmap",
            "*.log",
            "**/cache/",
            "**",
            // A name at any depth: `config/logs/` too.
            "logs/",
            "/server.jar",
            "mods/",
            "mods/*.disabled",
            "config/dynmap/",
            "/config",
            "plugins/",
            "plugins/*/config.yml",
            "plugins/Essentials/config.yml",
            "plugins/Essentials",
            "*/x.toml",
        ] {
            assert!(exclude_reaches_content(yes), "{yes}");
        }
        for no in [
            "saves/world/playerdata/",
            "saves/world/**",
            "saves/*/region",
            "/logs",
            "/logs/",
            "dynmap/web/tiles/",
            "plugins/Essentials/userdata/",
            "plugins/CoreProtect/database.db",
            "/server.jar/",
            "/.lodestone_modpack/",
            "/eula.txt",
        ] {
            assert!(!exclude_reaches_content(no), "{no}");
        }
    }

    #[test]
    fn excludes_that_can_reach_mods() {
        for yes in [
            "*.jar",
            "dynmap",
            "cache/",
            "**",
            "mods",
            "/mods/",
            "mods/*.disabled",
            "MODS/x.jar",
            "/m*/x.jar",
            "*/x.jar",
            "**/x.jar",
        ] {
            assert!(exclude_reaches_mods(yes), "{yes}");
        }
        for no in [
            "config/dynmap/",
            "/config",
            "config/*.txt",
            "/server.jar",
            "plugins/*.jar",
            "world/playerdata/",
            "/logs/",
            "/modsx/",
            "/c*/x",
        ] {
            assert!(!exclude_reaches_mods(no), "{no}");
        }
    }

    #[test]
    fn unmapped_paths_are_judged_by_their_disk_path() {
        let candidate = |disk: &'static str, is_dir: bool| Candidate {
            disk,
            path: None,
            is_dir,
        };
        assert!(content_filter(&candidate("mods/a:b.jar", false)));
        assert!(!content_filter(&candidate("world_nether", true)));
        assert!(!content_filter(&candidate("world/DIM-1", true)));
        assert!(!content_filter(&candidate("saves", true)));
        assert!(!content_filter(&candidate("saves/mods/x.jar", false)));
        assert!(!content_filter(&candidate("options.txt", false)));
    }

    #[test]
    fn a_level_inside_a_content_folder_is_not_content() {
        let candidate = |disk: &'static str, is_dir: bool| Candidate {
            disk,
            path: None,
            is_dir,
        };
        let nested = ContentFilter::new("mods/survival");
        assert!(!nested.accepts(&candidate("mods/survival", true)));
        assert!(!nested.accepts(&candidate("mods/survival/level.dat", false)));
        assert!(!nested.accepts(&candidate("MODS/Survival/playerdata/a.dat", false)));
        assert!(nested.accepts(&candidate("mods/a.jar", false)));
        assert!(nested.accepts(&candidate("mods/survivalist.jar", false)));
        let folder = ContentFilter::new("config");
        assert!(!folder.accepts(&candidate("config/level.dat", false)));
        assert!(!folder.accepts(&candidate("config/region", true)));
        assert!(!folder.accepts(&candidate("config/DIM-1/region/r.0.0.mca", false)));
        assert!(folder.accepts(&candidate("config/sodium.json", false)));
        assert!(folder.accepts(&candidate("config/jei", true)));
        // a level elsewhere changes nothing
        for level in ["world", "worlds/main", "../mods/x", ""] {
            let filter = ContentFilter::new(level);
            assert!(
                filter.accepts(&candidate("mods/x/level.dat", false)),
                "{level}"
            );
            assert!(
                filter.accepts(&candidate("config/level.dat", false)),
                "{level}"
            );
        }
    }

    #[test]
    fn lodestone_names_are_folded() {
        assert!(is_lodestone_name(".lodestone_config"));
        assert!(is_lodestone_name(".LODESTONE-restore"));
        assert!(is_lodestone_name(".lodeſtone_client_mods.json"));
        assert!(!is_lodestone_name("lodestone.txt"));
    }
}
