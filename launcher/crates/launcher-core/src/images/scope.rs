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

use lodestone_image::CanonicalPath;
use lodestone_image::layout::CLIENT_GAME_FOLDER;
use lodestone_image::store::snapshot::Candidate;

/// The version of these rules. A baseline taken under other rules is out of date (see
/// [`super::state::ContentState::scope_rules`]).
pub const SCOPE_RULES: u32 = 1;

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
    fn lodestone_names_are_folded() {
        assert!(is_lodestone_name(".lodestone_config"));
        assert!(is_lodestone_name(".LODESTONE-restore"));
        assert!(is_lodestone_name(".lodeſtone_client_mods.json"));
        assert!(!is_lodestone_name("lodestone.txt"));
    }
}
