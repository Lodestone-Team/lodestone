//! Rules for images that move between computers (`docs/instance-images.md`, "Sharing"),
//! which the program that writes an export and the one that reads it must agree on: what
//! an export places, what a reader takes as a version, how a sender takes files out, and
//! which names a disk would take for another.
//!
//! Lodestone Core and Lodestone Launcher both write and read `.lodeimg` files, so these
//! live here rather than in each program.

use std::collections::BTreeMap;

use crate::manifest::{Entry, Image};
use crate::paths::{ends_with_name, in_names, starts_with_name, CanonicalPath};
use crate::store::excludes::Excludes;

/// The most downloads an import makes: far more files than any real instance or server
/// has, so a file listing a million references is refused before anything is asked of
/// Modrinth or CurseForge.
pub const MAX_DOWNLOADS: usize = 20_000;

/// Every file an export places: its image's and its content link's (`linked`), the image
/// winning at a path both have. Sorted by path.
pub fn placed_entries(image: &Image, linked: &[Image]) -> Vec<Entry> {
    let mut by_path = BTreeMap::new();
    for entry in linked.iter().flat_map(|c| c.entries.iter()) {
        by_path.insert(entry.path.to_string(), entry.clone());
    }
    for entry in &image.entries {
        by_path.insert(entry.path.to_string(), entry.clone());
    }
    by_path.into_values().collect()
}

/// Whether `text` can be a Minecraft or loader version id: 1 to 64 bytes of ASCII letters,
/// digits and ` .-_+`, as every Minecraft and loader version has them. The sender wrote
/// them, a dialog shows them and what is made from the file keeps them, so they mustn't
/// hold prose or invisible characters (`1.21.1 (checked by Lodestone)`).
pub fn plausible_version(text: &str) -> bool {
    (1..=64).contains(&text.len())
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b" .-_+".contains(&b))
}

/// Takes out of `image` the entries an exclude pattern in `patterns` matches (the store's
/// pattern syntax, case-sensitive) or `also` picks (for names matched in any letter case),
/// adds the patterns to its `excludes` so the image says what it left out, and normalizes
/// it. Returns the paths taken out, sorted. The image isn't sealed: the caller seals it once every
/// change is made.
pub fn strip(
    image: &mut Image,
    patterns: &[&str],
    also: &dyn Fn(&CanonicalPath) -> bool,
) -> Vec<String> {
    let owned: Vec<String> = patterns.iter().map(|p| p.to_string()).collect();
    let excludes = Excludes::new(&owned);
    let mut cursor = excludes.cursor();
    let mut taken = Vec::new();
    image.entries.retain(|entry| {
        let out = also(&entry.path) || cursor.excludes(entry.path.as_str(), false);
        if out {
            taken.push(entry.path.to_string());
        }
        !out
    });
    for pattern in owned {
        if !image.excludes.contains(&pattern) {
            image.excludes.push(pattern);
        }
    }
    image.normalize();
    taken.sort();
    taken
}

/// Whether the top-level name `name` is one of the files the JVM writes into its working
/// folder when it crashes or dumps memory (`hs_err_pid*`, `replay_pid*`,
/// `java_pid*.hprof`, `*.mdmp`, `core`, `core.<digits>`), in any letter case. A crash log
/// holds the whole command line (a client's access token among it) and a dump the process's
/// memory, so no image that leaves the computer carries them.
pub fn is_jvm_crash_name(name: &str) -> bool {
    if starts_with_name(name, "hs_err_pid")
        || starts_with_name(name, "replay_pid")
        || (starts_with_name(name, "java_pid") && ends_with_name(name, ".hprof"))
        || ends_with_name(name, ".mdmp")
        || in_names(name, &["core"])
    {
        return true;
    }
    let folded = name.to_uppercase().to_lowercase();
    folded
        .strip_prefix("core.")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether the top-level name `name` is one of the official Minecraft launcher's account
/// files, in any letter case: `launcher_profiles.json`, `launcher_accounts*.json` and
/// `launcher_msa_credentials*.bin` (Microsoft sign-in tokens). A game folder pasted into
/// an instance or imported as a server can hold them, and no image that leaves the
/// computer carries them.
pub fn is_launcher_account_file(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name == "launcher_profiles.json"
        || (name.starts_with("launcher_accounts") && name.ends_with(".json"))
        || (name.starts_with("launcher_msa_credentials") && name.ends_with(".bin"))
}

/// A player's files no export carries, as exclude patterns (a name at any depth, a path
/// with `/` from the folder, `*` within a name): the commands a player typed (a server's
/// `/login` password among them), the creative hotbars, account switchers' files and the
/// official launcher's account files (Microsoft sign-in tokens). The Launcher's export
/// leaves them out of instances, the core's out of a server made from a game folder;
/// [`is_player_private`] matches them in any letter case.
pub const PLAYER_PRIVATE: &[&str] = &[
    "command_history.txt",
    "hotbar.nbt",
    "ias.json",
    "essential/microsoft_accounts.json",
    "essential/mojang_accounts.json",
    "/launcher_profiles.json",
    "/launcher_accounts*.json",
    "/launcher_msa_credentials*.bin",
];

/// Whether `path` is one of [`PLAYER_PRIVATE`]'s files, whatever its letter case.
pub fn is_player_private(path: &CanonicalPath) -> bool {
    let parts: Vec<&str> = path.components().collect();
    match parts.as_slice() {
        [name] if is_launcher_account_file(name) => true,
        [.., dir, name]
            if in_names(dir, &["essential"])
                && in_names(name, &["microsoft_accounts.json", "mojang_accounts.json"]) =>
        {
            true
        }
        [.., name] => in_names(name, &["command_history.txt", "hotbar.nbt", "ias.json"]),
        [] => false,
    }
}

/// Whether `path` is a file of a server's world that only a server reads, which a client's
/// save never takes: what a Paper or Bukkit server's split Nether and End folders keep
/// beside their dimension (`saves/<w>/.bukkit_nether/…`, `saves/<w>/.bukkit_the_end/…`,
/// their own `level.dat`, `uid.dat` and settings), and Paper's settings of a world
/// (`saves/<w>/paper-world.yml`), in any letter case. The dimensions themselves come back
/// as a client's `DIM-1` and `DIM1` through the canonical paths.
pub fn is_server_world_file(path: &CanonicalPath) -> bool {
    let parts: Vec<&str> = path.components().collect();
    match parts.as_slice() {
        [saves, _world, name] if in_names(saves, &["saves"]) => {
            in_names(name, &["paper-world.yml"])
                || in_names(
                    name,
                    &[crate::layout::BUKKIT_NETHER, crate::layout::BUKKIT_END],
                )
        }
        [saves, _world, folder, _, ..] if in_names(saves, &["saves"]) => in_names(
            folder,
            &[crate::layout::BUKKIT_NETHER, crate::layout::BUKKIT_END],
        ),
        _ => false,
    }
}

/// Whether `part` is one of `names` (lower-case ASCII) spelled with other than ASCII
/// letters, as a disk that ignores letter case would still take it for that name: `modſ`
/// (a long s) is `mods` on APFS, and a loader loads from it, while a check comparing
/// letters would miss it. An import refuses such top-level folders.
pub fn spelled_like(part: &str, names: &[&str]) -> bool {
    !part.is_ascii() && in_names(part, names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::tests::{fabric, object};
    use crate::manifest::Kind;

    #[test]
    fn the_image_wins_over_its_content_link() {
        let mut content = Image::new(Kind::Content, fabric());
        let mut old = object("config/a.toml");
        old.size = 99;
        content.entries = vec![old, object("mods/b.jar")];
        let mut data = Image::new(Kind::Data, fabric());
        data.entries = vec![object("config/a.toml"), object("saves/w/level.dat")];
        let placed = placed_entries(&data, &[content]);
        let paths: Vec<&str> = placed.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["config/a.toml", "mods/b.jar", "saves/w/level.dat"]);
        assert_eq!(placed[0].size, "config/a.toml".len() as u64);
    }

    #[test]
    fn a_players_private_files_are_known_in_any_letter_case() {
        let private = |path: &str| is_player_private(&CanonicalPath::new(path).unwrap());
        for path in [
            "command_history.txt",
            "config/Command_History.TXT",
            "hotbar.nbt",
            "config/ias.json",
            "essential/microsoft_accounts.json",
            "config/Essential/Mojang_Accounts.json",
            "launcher_profiles.json",
            "LAUNCHER_ACCOUNTS_microsoft_store.json",
            "launcher_msa_credentials.bin",
        ] {
            assert!(private(path), "{path}");
        }
        for path in [
            "options.txt",
            "config/launcher_profiles.json",
            "microsoft_accounts.json",
            "saves/w/level.dat",
        ] {
            assert!(!private(path), "{path}");
        }
        // every pattern is one the matcher knows
        let excludes = Excludes::new(
            &PLAYER_PRIVATE
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>(),
        );
        let mut cursor = excludes.cursor();
        assert!(cursor.excludes("config/hotbar.nbt", false));
        assert!(cursor.excludes("launcher_accounts.json", false));
    }

    #[test]
    fn only_plausible_versions_pass() {
        for ok in [
            "1.21.1",
            "24w14a",
            "1.14 Pre-Release 2",
            "1.20.1-47.2.0",
            "0.26.0-beta.1+build.3",
        ] {
            assert!(plausible_version(ok), "{ok}");
        }
        for bad in [
            "",
            "1.21.1 (checked by Lodestone: no mods)",
            "1.21.1\u{202E}",
            "0.16\n10",
            &"1".repeat(65),
            "1.21/../x",
        ] {
            assert!(!plausible_version(bad), "{bad:?}");
        }
    }

    #[test]
    fn strip_takes_files_out_and_says_so() {
        let mut image = Image::new(Kind::Full, fabric());
        image.entries = vec![
            object("config/a.toml"),
            object("logsx/b.txt"),
            object("plugins/X.jar"),
            object("plugins/X/config.yml"),
            object("HS_ERR_PID12.log"),
            object("eula.txt"),
        ];
        image.excludes = vec!["dynmap/".into()];
        let taken = strip(&mut image, &["plugins/*/", "/eula.txt"], &|p| {
            p.components().count() == 1 && is_jvm_crash_name(p.as_str())
        });
        assert_eq!(
            taken,
            ["HS_ERR_PID12.log", "eula.txt", "plugins/X/config.yml"]
        );
        let left: Vec<&str> = image.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(left, ["config/a.toml", "logsx/b.txt", "plugins/X.jar"]);
        assert_eq!(image.excludes, ["/eula.txt", "dynmap/", "plugins/*/"]);
        image.seal();
        image.validate().unwrap();
        image.verify().unwrap();
        // again: nothing more, no pattern twice
        let mut again = image.clone();
        assert!(strip(&mut again, &["/eula.txt"], &|_| false).is_empty());
        assert_eq!(again.excludes, image.excludes);
    }

    #[test]
    fn crash_and_dump_files_are_known_in_any_case() {
        for name in [
            "hs_err_pid123.log",
            "HS_ERR_PID1.LOG",
            "replay_pid9.log",
            "java_pid4.hprof",
            "Java_Pid4.HPROF",
            "minecraft.mdmp",
            "core",
            "CORE",
            "core.1234",
        ] {
            assert!(is_jvm_crash_name(name), "{name}");
        }
        for name in [
            "core.txt",
            "core.",
            "cores",
            "java_pid4.log",
            "server.properties",
        ] {
            assert!(!is_jvm_crash_name(name), "{name}");
        }
    }

    #[test]
    fn the_official_launchers_account_files_are_known_in_any_case() {
        for name in [
            "launcher_profiles.json",
            "Launcher_Accounts.json",
            "launcher_accounts_microsoft_store.json",
            "LAUNCHER_MSA_CREDENTIALS.BIN",
            "launcher_msa_credentials_microsoft_store.bin",
        ] {
            assert!(is_launcher_account_file(name), "{name}");
        }
        for name in [
            "launcher_settings.json",
            "launcher_accounts.txt",
            "options.txt",
        ] {
            assert!(!is_launcher_account_file(name), "{name}");
        }
    }

    #[test]
    fn a_servers_own_world_files_are_known_in_any_case() {
        let server_only = |path: &str| is_server_world_file(&CanonicalPath::new(path).unwrap());
        for path in [
            "saves/world/paper-world.yml",
            "saves/world/Paper-World.YML",
            "saves/world/.bukkit_nether/level.dat",
            "saves/world/.bukkit_nether/paper-world.yml",
            "saves/world/.BUKKIT_THE_END/uid.dat",
            "saves/world/.bukkit_the_end/data/x.dat",
        ] {
            assert!(server_only(path), "{path}");
        }
        for path in [
            "saves/world/level.dat",
            "saves/world/DIM-1/region/r.0.0.mca",
            "saves/world/DIM1/region/r.0.0.mca",
            // only at a world's top
            "saves/world/sub/paper-world.yml",
            "saves/world/region/.bukkit_nether",
            "paper-world.yml",
            "config/paper-world.yml",
            "world/paper-world.yml",
        ] {
            assert!(!server_only(path), "{path}");
        }
    }

    #[test]
    fn folders_spelled_otherwise_are_found() {
        let names = ["mods", "config"];
        assert!(spelled_like("modſ", &names));
        // the Kelvin sign upper-cases to K
        assert!(spelled_like("\u{212A}ubejs", &["kubejs"]));
        assert!(
            !spelled_like("mods", &names),
            "plain spelling is the folder itself"
        );
        assert!(
            !spelled_like("MODS", &names),
            "ASCII case is the plain spelling"
        );
        assert!(!spelled_like("modzé", &names));
    }
}
