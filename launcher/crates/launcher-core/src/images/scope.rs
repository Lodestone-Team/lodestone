//! Which files of a Launcher instance are content: what a content image records.
//!
//! The spec splits an instance into content (mods, configs, versions, settings), recorded
//! at every change, and data ("worlds and everything else"), recorded by snapshots. So
//! content is an allowlist, as in the core: a file is content only when a rule here names
//! it, and anything the game writes elsewhere stays data. Getting this wrong never loses a
//! file (snapshots take everything); it only decides what goes into content images.
//!
//! Content is everything in the game folder's `mods/`, `config/`, `defaultconfigs/`,
//! `kubejs/`, `resourcepacks/` and `shaderpacks/` (disabled `*.disabled` files too: turning
//! a mod off is a content change). Those are the client's own content folders in the
//! spec's paths table; the game, the loader and the client settings that travel come from
//! `instance.json` and `options.txt` instead ([`super::context`]).
//!
//! Content is also every file the instance's pack or an added pack wrote elsewhere, as
//! `modpack.json` and the added packs' file list name it ([`ContentScope`]): packs put
//! CraftTweaker's `scripts/`, `global_packs/`, FancyMenu's or Patchouli's folders and the
//! like in their overrides, and the modpack updater's three-way merge changes them, so the
//! image before an update must hold them. Files the game writes are never taken this way,
//! even when a pack ships them (`options.txt`, `servers.dat`, worlds): see
//! [`never_pack_content`]. A file in such a folder that no pack wrote (a script added by
//! hand) isn't content.
//!
//! Never content: names starting with `.lodestone` at any depth (Lodestone's own files,
//! and downloads in flight, `.lodestone-<sha1>.download`), worlds (`saves/`), screenshots,
//! logs, crash reports, `options.txt` and the other files the game writes, and anything
//! outside the game folder (`instance.json`, `content.json`, `modpack.json`, the icon). The
//! game rewrites some configs at startup; those join the next image an operation records
//! (see [`super::check`]).
//!
//! Names match ignoring ASCII letter case, as Windows and macOS's default disks do; the
//! `.lodestone` prefix is folded fully, so `.LodeſTone…` is Lodestone's too (NTFS
//! upper-cases `ſ` to `S`).

use std::collections::BTreeSet;

use lodestone_image::CanonicalPath;
use lodestone_image::layout::CLIENT_GAME_FOLDER;
use lodestone_image::paths::always_excluded;
use lodestone_image::store::snapshot::Candidate;

/// The version of these rules. A baseline taken under other rules is out of date (see
/// [`super::state::ContentState::scope_rules`]).
pub const SCOPE_RULES: u32 = 2;

/// The most pack files a scope takes ([`ContentScope::new`]): far more than any pack
/// writes, and a bound on what a hostile `modpack.json` can make it hold.
pub const MAX_PACK_FILES: usize = 100_000;
/// The most bytes of paths a scope holds, whatever their number: a bound on memory when a
/// pack lists long paths.
pub const MAX_PACK_PATH_BYTES: usize = 16 << 20;

/// Top-level folders of the game folder never taken from a pack's file list: worlds, what
/// the game and the Launcher keep, and the spec's always-left-out folders.
const NEVER_PACK_FOLDERS: [&str; 13] = [
    "saves",
    "screenshots",
    "logs",
    "crash-reports",
    "cache",
    ".cache",
    "natives",
    ".fabric",
    ".mixin.out",
    "debug",
    "versions",
    "libraries",
    "assets",
];

/// Top-level files the game writes as it plays, never taken from a pack's file list.
const GAME_FILES: [&str; 8] = [
    "options.txt",
    "optionsof.txt",
    "optionsshaders.txt",
    "servers.dat",
    "servers.dat_old",
    "realms_persistence.json",
    "usercache.json",
    "usernamecache.json",
];

/// Folders of the game folder whose every file is content.
pub const CONTENT_FOLDERS: [&str; 6] = [
    "mods",
    "config",
    "defaultconfigs",
    "kubejs",
    "resourcepacks",
    "shaderpacks",
];

/// Whether the file or folder at the canonical `path` is content (for a folder: whether
/// anything in it can be). False for a folder means nothing in it is, so a walk can skip
/// it. The rules don't depend on which it is: a folder is content where its files are, and
/// something other than a folder at a content folder's name counts too, so a link there
/// (`config` pointing elsewhere) is withheld as a link rather than silently missing.
pub fn is_content(path: &CanonicalPath) -> bool {
    let parts: Vec<&str> = path.components().collect();
    is_content_parts(&parts)
}

/// [`is_content`] for a snapshot's filter. A path the client layout can't map (no
/// canonical path) is judged by its disk parts under the game folder, so a mod with a name
/// no image can hold (`mods/a:b.jar`) is withheld from the content image rather than
/// silently missing. Nothing outside the game folder is content.
pub fn content_filter(candidate: &Candidate) -> bool {
    match candidate.path {
        Some(path) => is_content(path),
        None => {
            let mut parts = candidate.disk.split('/');
            if parts.next() != Some(CLIENT_GAME_FOLDER) {
                return false;
            }
            let parts: Vec<&str> = parts.collect();
            is_content_parts(&parts)
        }
    }
}

fn is_content_parts(parts: &[&str]) -> bool {
    if parts.is_empty() || parts.iter().any(|part| is_lodestone_name(part)) {
        return false;
    }
    CONTENT_FOLDERS
        .iter()
        .any(|folder| parts[0].eq_ignore_ascii_case(folder))
}

/// Whether a pack file at `parts` (relative to the game folder) is kept out of the
/// content images whatever the pack says: Lodestone's own names, `session.lock`, files the
/// game writes, worlds and the other folders in [`NEVER_PACK_FOLDERS`].
pub fn never_pack_content(parts: &[&str]) -> bool {
    let Some(first) = parts.first() else {
        return true;
    };
    parts
        .iter()
        .any(|part| is_lodestone_name(part) || part.eq_ignore_ascii_case("session.lock"))
        || NEVER_PACK_FOLDERS
            .iter()
            .any(|folder| first.eq_ignore_ascii_case(folder))
        || (parts.len() == 1 && GAME_FILES.iter().any(|f| first.eq_ignore_ascii_case(f)))
}

/// One instance's content rules: the content folders, and the files its packs wrote
/// elsewhere (see the module).
#[derive(Debug, Clone, Default)]
pub struct ContentScope {
    /// Pack files outside the content folders, by path in the game folder with ASCII
    /// letters folded to lower case. Ordered, so a folder holding one is found without a
    /// set of every folder (a deep path would add a string per level).
    files: BTreeSet<String>,
    /// The bytes of `files`' paths.
    bytes: usize,
}

impl ContentScope {
    /// The scope with the pack files at `paths` (relative to the game folder, as
    /// `modpack.json` keys them), at most [`MAX_PACK_FILES`] of them and
    /// [`MAX_PACK_PATH_BYTES`] of paths. Paths that are content already, that
    /// [`never_pack_content`] keeps out, or that aren't plain relative paths are left out.
    pub fn new<'a>(paths: impl IntoIterator<Item = &'a str>) -> Self {
        let mut scope = ContentScope::default();
        for path in paths {
            if scope.files.len() >= MAX_PACK_FILES
                || scope.bytes.saturating_add(path.len()) > MAX_PACK_PATH_BYTES
            {
                break;
            }
            if path.contains('\\') {
                continue;
            }
            let parts: Vec<&str> = path.split('/').collect();
            if parts
                .iter()
                .any(|part| part.is_empty() || *part == "." || *part == "..")
                || is_content_parts(&parts)
                || never_pack_content(&parts)
            {
                continue;
            }
            if scope.files.insert(path.to_ascii_lowercase()) {
                scope.bytes += path.len();
            }
        }
        scope
    }

    /// Whether the scope has pack files outside the content folders.
    pub fn has_pack_files(&self) -> bool {
        !self.files.is_empty()
    }

    /// [`content_filter`], and the pack files: a snapshot's filter for this instance.
    pub fn accepts(&self, candidate: &Candidate) -> bool {
        content_filter(candidate) || self.lists(candidate)
    }

    /// Whether `candidate` is one of the pack files, or a folder holding one.
    fn lists(&self, candidate: &Candidate) -> bool {
        if self.files.is_empty() {
            return false;
        }
        let relative = match candidate.path {
            Some(path) if always_excluded(path) => return false,
            Some(path) => path.as_str().to_string(),
            None => match candidate.disk.split_once('/') {
                Some((CLIENT_GAME_FOLDER, rest)) => rest.to_string(),
                _ => return false,
            },
        };
        let parts: Vec<&str> = relative.split('/').collect();
        if never_pack_content(&parts) {
            return false;
        }
        let folded = relative.to_ascii_lowercase();
        if candidate.is_dir {
            // A folder holding a pack file: the first file at or after `folder/` is in it.
            let prefix = format!("{folded}/");
            self.files
                .range::<str, _>((std::ops::Bound::Included(prefix.as_str()), std::ops::Bound::Unbounded))
                .next()
                .is_some_and(|file| file.starts_with(&prefix))
        } else {
            self.files.contains(&folded)
        }
    }
}

/// Whether `name` is one of Lodestone's own (`.lodestone…`), folded the way a disk that
/// ignores letter case might.
pub fn is_lodestone_name(name: &str) -> bool {
    name.to_uppercase().to_lowercase().starts_with(".lodestone")
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
            "mods/sodium.jar",
            "mods/sodium.jar.disabled",
            "MODS/Sodium.JAR",
            "mods/sub/x.jar",
            "config/sodium-options.json",
            "Config/x/y.toml",
            "defaultconfigs/x.toml",
            "kubejs/client_scripts/a.js",
            "resourcepacks/Faithful.zip",
            "resourcepacks/folder/pack.mcmeta",
            "shaderpacks/BSL.zip.disabled",
        ] {
            assert!(is_content(&p(yes)), "{yes}");
        }
        for no in [
            "options.txt",
            "optionsof.txt",
            "optionsshaders.txt",
            "servers.dat",
            "saves/world/level.dat",
            "saves/world/datapacks/x.zip",
            "screenshots/a.png",
            "usercache.json",
            "mods/.lodestone-abc.download",
            "mods/.LODESTONE_client_mods.json",
            "config/.lodeſtone_x",
            ".lodestone_modpack/update.json",
            "modsx/a.jar",
        ] {
            assert!(!is_content(&p(no)), "{no}");
        }
        for (folder, content) in [
            ("mods", true),
            ("config", true),
            ("resourcepacks", true),
            ("saves", false),
            ("saves/world", false),
            ("screenshots", false),
            ("mods/.lodestone-staging", false),
        ] {
            assert_eq!(is_content(&p(folder)), content, "{folder}/");
        }
        // A link or file where a content folder goes is offered, so it is withheld.
        assert!(is_content(&p("config")));
    }

    #[test]
    fn unmapped_paths_are_judged_by_their_parts_in_the_game_folder() {
        let candidate = |disk: &'static str, is_dir: bool| Candidate {
            disk,
            path: None,
            is_dir,
        };
        // A mod whose name no image can hold is offered (and then withheld).
        assert!(content_filter(&candidate("minecraft/mods/a:b.jar", false)));
        assert!(content_filter(&candidate("minecraft/Mods/CON.jar", false)));
        assert!(!content_filter(&candidate(
            "minecraft/mods/.lodestone-x",
            false
        )));
        assert!(!content_filter(&candidate("minecraft/saves/a:b", true)));
        assert!(!content_filter(&candidate("minecraft/options.txt", false)));
        // Outside the game folder: the instance's own files.
        assert!(!content_filter(&candidate("instance.json", false)));
        assert!(!content_filter(&candidate("content.json", false)));
        assert!(!content_filter(&candidate("mods/x.jar", false)));
    }

    #[test]
    fn pack_files_outside_the_content_folders_are_content() {
        let scope = ContentScope::new([
            "scripts/recipes.zs",
            "global_packs/required/pack.zip",
            "Patchouli_Books/guide/book.json",
            "mods/a.jar",
            "options.txt",
            "servers.dat",
            "saves/World/level.dat",
            "logs/latest.log",
            ".lodestone_modpack/update.json",
            "fancymenu_data/../escape.txt",
            "fancymenu_data//x.txt",
            "a\\b.txt",
            "/rooted.txt",
        ]);
        let candidate = |disk: &'static str, path: Option<&CanonicalPath>, is_dir: bool| {
            scope.accepts(&Candidate { disk, path, is_dir })
        };
        for (path, is_dir) in [
            ("scripts/recipes.zs", false),
            ("SCRIPTS/Recipes.zs", false),
            ("scripts", true),
            ("global_packs", true),
            ("global_packs/required", true),
            ("global_packs/required/pack.zip", false),
            ("patchouli_books/guide/book.json", false),
            // The content folders as always.
            ("mods/b.jar", false),
        ] {
            assert!(candidate("", Some(&p(path)), is_dir), "{path}");
        }
        for (path, is_dir) in [
            ("scripts/other.zs", false),
            ("scripts/sub", true),
            ("options.txt", false),
            ("servers.dat", false),
            ("saves/World/level.dat", false),
            ("saves", true),
            ("logs/latest.log", false),
            ("escape.txt", false),
            ("rooted.txt", false),
        ] {
            assert!(!candidate("", Some(&p(path)), is_dir), "{path}");
        }
        // A pack file whose name no image can hold is offered, so it is withheld.
        let odd = ContentScope::new(["scripts/a:b.zs"]);
        assert!(odd.accepts(&Candidate {
            disk: "minecraft/scripts/a:b.zs",
            path: None,
            is_dir: false,
        }));
        assert!(!odd.accepts(&Candidate {
            disk: "scripts/a:b.zs",
            path: None,
            is_dir: false,
        }));
        assert!(!ContentScope::default().has_pack_files());
        assert!(scope.has_pack_files());
        // Bounded, however long the pack's list.
        let many: Vec<String> = (0..MAX_PACK_FILES + 10)
            .map(|i| format!("scripts/{i}.zs"))
            .collect();
        let big = ContentScope::new(many.iter().map(String::as_str));
        assert_eq!(big.files.len(), MAX_PACK_FILES);
        // Deep paths hold only themselves (no string per folder level), and the bytes of
        // paths are bounded too.
        let deep_tail = "/a".repeat(250);
        let deep: Vec<String> = (0..40_000).map(|i| format!("x{i:05}{deep_tail}/f")).collect();
        let deep_scope = ContentScope::new(deep.iter().map(String::as_str));
        assert!(deep_scope.bytes <= MAX_PACK_PATH_BYTES);
        let held: usize = deep_scope.files.iter().map(String::len).sum();
        assert_eq!(held, deep_scope.bytes);
        assert!(deep_scope.files.len() < deep.len(), "the byte bound cut the list");
        let folder = format!("x00000{}", "/a".repeat(100));
        assert!(deep_scope.accepts(&Candidate {
            disk: "",
            path: Some(&p(&folder)),
            is_dir: true,
        }));
    }

    #[test]
    fn lodestone_names_are_folded() {
        assert!(is_lodestone_name(".lodestone_config"));
        assert!(is_lodestone_name(".LODESTONE-restore"));
        assert!(is_lodestone_name(".lodeſtone_client_mods.json"));
        assert!(!is_lodestone_name("lodestone.txt"));
    }
}
