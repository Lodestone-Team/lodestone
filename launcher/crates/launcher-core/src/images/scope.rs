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
/// The longest pack path a scope takes: no real path is longer, and one that is (a hostile
/// `modpack.json`) is skipped rather than using up [`MAX_PACK_PATH_BYTES`].
pub const MAX_PACK_PATH_LEN: usize = 4096;

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
/// `command_history.txt` holds the commands the player typed, passwords given to a
/// server's `/login` among them.
const GAME_FILES: [&str; 10] = [
    "options.txt",
    "optionsof.txt",
    "optionsshaders.txt",
    "servers.dat",
    "servers.dat_old",
    "realms_persistence.json",
    "usercache.json",
    "usernamecache.json",
    "command_history.txt",
    "hotbar.nbt",
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
    /// [`never_pack_content`] keeps out, that are longer than [`MAX_PACK_PATH_LEN`], or that
    /// aren't plain relative paths are left out.
    pub fn new<'a>(paths: impl IntoIterator<Item = &'a str>) -> Self {
        let mut scope = ContentScope::default();
        for path in paths {
            if path.len() > MAX_PACK_PATH_LEN || path.contains('\\') {
                continue;
            }
            if scope.files.len() >= MAX_PACK_FILES
                || scope.bytes.saturating_add(path.len()) > MAX_PACK_PATH_BYTES
            {
                break;
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
                .range::<str, _>((
                    std::ops::Bound::Included(prefix.as_str()),
                    std::ops::Bound::Unbounded,
                ))
                .next()
                .is_some_and(|file| file.starts_with(&prefix))
        } else {
            self.files.contains(&folded)
        }
    }
}

impl ContentScope {
    /// The other half of the instance: a snapshot's filter for a data image, which takes
    /// everything in the game folder that isn't content ("worlds and everything else":
    /// `saves/`, `options.txt`, `servers.dat`, screenshots, a script no pack lists). A file
    /// is taken when [`Self::accepts`] turns it down; a folder is turned down only when
    /// everything in it is content (a content folder, or something in one), so a folder
    /// that holds some pack files is still walked for its other files. Lodestone's own
    /// names are never taken. Content and data never overlap, and together they are
    /// everything a walk without a filter takes.
    pub fn data_accepts(&self, candidate: &Candidate) -> bool {
        let Some(parts) = game_parts(candidate) else {
            return false;
        };
        if parts.iter().any(|part| is_lodestone_name(part)) {
            return false;
        }
        if candidate.is_dir {
            !is_content_parts(&parts)
        } else {
            !self.accepts(candidate)
        }
    }
}

/// A snapshot's filter for a full image: everything in the game folder but Lodestone's own
/// names (which no image holds anyway).
pub fn full_filter(candidate: &Candidate) -> bool {
    game_parts(candidate).is_some_and(|parts| !parts.iter().any(|part| is_lodestone_name(part)))
}

/// The parts of `candidate`'s path in the game folder: its canonical path, or, for a name
/// the layout can't map, its disk path under the game folder. `None` outside the game
/// folder.
fn game_parts<'c>(candidate: &Candidate<'c>) -> Option<Vec<&'c str>> {
    match candidate.path {
        Some(path) => Some(path.components().collect()),
        None => {
            let mut parts = candidate.disk.split('/');
            if parts.next() != Some(CLIENT_GAME_FOLDER) {
                return None;
            }
            Some(parts.collect())
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
            "command_history.txt",
            "Hotbar.nbt",
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
            // What the player typed (a server's /login password among it).
            ("command_history.txt", false),
            ("hotbar.nbt", false),
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
        let deep: Vec<String> = (0..40_000)
            .map(|i| format!("x{i:05}{deep_tail}/f"))
            .collect();
        let deep_scope = ContentScope::new(deep.iter().map(String::as_str));
        assert!(deep_scope.bytes <= MAX_PACK_PATH_BYTES);
        let held: usize = deep_scope.files.iter().map(String::len).sum();
        assert_eq!(held, deep_scope.bytes);
        assert!(
            deep_scope.files.len() < deep.len(),
            "the byte bound cut the list"
        );
        // One path too long to be real is skipped, not the end of the list.
        let huge = format!("0{}/x", "a".repeat(MAX_PACK_PATH_LEN));
        let after_huge = ContentScope::new([huge.as_str(), "scripts/after.zs"]);
        assert!(after_huge.lists(&Candidate {
            disk: "",
            path: Some(&p("scripts/after.zs")),
            is_dir: false,
        }));
        assert_eq!(after_huge.files.len(), 1);
        let folder = format!("x00000{}", "/a".repeat(100));
        assert!(deep_scope.accepts(&Candidate {
            disk: "",
            path: Some(&p(&folder)),
            is_dir: true,
        }));
    }

    /// The disk paths a walk of `root` takes with `filter` (all of them with `None`).
    fn walked(
        root: &std::path::Path,
        filter: Option<&(dyn Fn(&Candidate) -> bool + Sync)>,
    ) -> BTreeSet<String> {
        use lodestone_image::DiskLayout;
        use lodestone_image::store::snapshot::{SnapshotOptions, plan};
        use lodestone_image::store::{Encoding, Store};
        let store_dir = tempfile::tempdir().unwrap();
        let store = Store::new(store_dir.path(), Encoding::Raw);
        let options = SnapshotOptions {
            filter,
            ..SnapshotOptions::default()
        };
        plan(&store, root, &DiskLayout::Client, &options)
            .unwrap()
            .to_read
            .into_iter()
            .map(|file| file.disk)
            .collect()
    }

    #[test]
    fn content_and_data_split_the_game_folder_without_overlap() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for file in [
            "instance.json",
            "minecraft/mods/a.jar",
            "minecraft/mods/b.jar.disabled",
            "minecraft/config/a.toml",
            "minecraft/resourcepacks/p.zip",
            "minecraft/scripts/pack.zs",
            "minecraft/scripts/mine.zs",
            "minecraft/scripts/deep/other.zs",
            "minecraft/saves/World/level.dat",
            "minecraft/saves/World/region/r.0.0.mca",
            "minecraft/options.txt",
            "minecraft/servers.dat",
            "minecraft/screenshots/a.png",
            "minecraft/.lodestone-tmp-x",
            "minecraft/mods/.lodestone-y.download",
            "minecraft/.lodestone_marker",
            "minecraft/saves/World/.lodestone_x",
            "minecraft/logs/latest.log",
        ] {
            let path = root.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, file).unwrap();
        }
        let scope = ContentScope::new(["scripts/pack.zs"]);
        let content_filter = |c: &Candidate| scope.accepts(c);
        let data_filter = |c: &Candidate| scope.data_accepts(c);
        let content = walked(root, Some(&content_filter));
        let data = walked(root, Some(&data_filter));
        let full = walked(root, Some(&full_filter));
        let everything = walked(root, None);
        assert!(content.is_disjoint(&data), "{content:?} {data:?}");
        let union: BTreeSet<String> = content.union(&data).cloned().collect();
        let lodestone_free: BTreeSet<String> = everything
            .iter()
            .filter(|p| !p.split('/').any(is_lodestone_name))
            .cloned()
            .collect();
        assert_eq!(union, lodestone_free);
        assert_eq!(full, lodestone_free);
        for yes in [
            "minecraft/saves/World/level.dat",
            "minecraft/saves/World/region/r.0.0.mca",
            "minecraft/options.txt",
            "minecraft/servers.dat",
            "minecraft/screenshots/a.png",
            // A script no pack lists, beside one a pack does.
            "minecraft/scripts/mine.zs",
            "minecraft/scripts/deep/other.zs",
        ] {
            assert!(data.contains(yes), "{yes}");
        }
        for no in [
            "minecraft/scripts/pack.zs",
            "minecraft/mods/a.jar",
            "minecraft/mods/b.jar.disabled",
            "minecraft/config/a.toml",
            "minecraft/.lodestone_marker",
            "minecraft/saves/World/.lodestone_x",
            "minecraft/logs/latest.log",
            "instance.json",
        ] {
            assert!(!data.contains(no), "{no}");
        }
        // Lodestone's own names are in no image.
        assert!(!full.iter().any(|p| p.contains(".lodestone")));
    }

    #[test]
    fn lodestone_names_are_folded() {
        assert!(is_lodestone_name(".lodestone_config"));
        assert!(is_lodestone_name(".LODESTONE-restore"));
        assert!(is_lodestone_name(".lodeſtone_client_mods.json"));
        assert!(!is_lodestone_name("lodestone.txt"));
    }
}
