//! Trusting a core for files that run as code on this Mac.
//!
//! Mods are Java programs with the same access to the user's files and accounts as the
//! user, and a pack's configs and script folders (KubeJS, CraftTweaker) can drive code
//! too. So before the launcher installs such files from a core the user hasn't trusted
//! ([`SavedCore::is_trusted`]), it shows an advisory: what will be installed, grouped by
//! whether Modrinth knows the file ([`advisory`]). World downloads and resource packs are
//! data and need none.
//!
//! Every pull from a core asks [`needs_advisory`] first, so new kinds of pulls follow the
//! same rule.

use super::store::SavedCore;
use crate::content::Modrinth;

/// What the launcher takes from a core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PullKind {
    /// A server's client pack (`.mrpack`): mods, and `overrides/` with configs and
    /// scripts. "Create matching instance".
    ClientPack,
    /// Bringing an instance's mods in step with its server's pack (before Play, or a
    /// re-sync).
    ModSync,
    /// Any other jars.
    Jars,
    /// Script folders or other files that drive code (KubeJS, CraftTweaker).
    Scripts,
    /// A world: data. Datapacks run inside the game's sandbox.
    World,
    /// A resource pack: textures, sounds and models.
    ResourcePack,
    /// Status, players, console: nothing is installed.
    Status,
}

impl PullKind {
    /// Whether the pull installs files that can run as code.
    pub fn is_executable(self) -> bool {
        match self {
            PullKind::ClientPack | PullKind::ModSync | PullKind::Jars | PullKind::Scripts => true,
            PullKind::World | PullKind::ResourcePack | PullKind::Status => false,
        }
    }
}

/// Whether to show the advisory before pulling `kind` from `core`: for code, unless the
/// user trusts the core.
pub fn needs_advisory(core: &SavedCore, kind: PullKind) -> bool {
    kind.is_executable() && !core.is_trusted()
}

/// The same for a friend's server (Lodestone Connect): for code, unless the user
/// trusts it under its current certificate.
pub fn friend_needs_advisory(friend: &super::friends::FriendServer, kind: PullKind) -> bool {
    kind.is_executable() && !friend.is_trusted()
}

/// A file the launcher is about to install, by its path in the game folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PulledFile {
    pub path: String,
    /// Lowercase hex.
    pub sha1: String,
    /// Known for files of the pack's index; overrides only have their SHA-1.
    pub sha512: Option<String>,
}

/// What a file is, for the advisory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileRole {
    /// A jar, or anything in `mods/`.
    Code,
    /// Script folders that drive code: `kubejs/`, `scripts/` (CraftTweaker).
    Script,
    /// `config/`, `defaultconfigs/`, and settings files.
    Config,
    Other,
}

pub const SCRIPT_DIRS: [&str; 2] = ["kubejs", "scripts"];

pub fn file_role(path: &str) -> FileRole {
    let lower = path.to_ascii_lowercase();
    let first = lower.split('/').next().unwrap_or_default();
    if lower.ends_with(".jar") || first == "mods" {
        FileRole::Code
    } else if SCRIPT_DIRS.contains(&first) {
        FileRole::Script
    } else if matches!(first, "config" | "defaultconfigs")
        || [
            ".toml",
            ".json",
            ".json5",
            ".cfg",
            ".properties",
            ".yml",
            ".yaml",
            ".txt",
        ]
        .iter()
        .any(|ext| lower.ends_with(ext))
    {
        FileRole::Config
    } else {
        FileRole::Other
    }
}

/// A file Modrinth has, by hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModrinthMatch {
    pub path: String,
    /// The project's title, or its id when Modrinth didn't send the project.
    pub project: String,
    pub version: String,
}

/// What the advisory lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Advisory {
    /// Jars Modrinth has, by SHA-1 (and SHA-512 where both are known).
    pub on_modrinth: Vec<ModrinthMatch>,
    /// Jars Modrinth doesn't have: uploaded to the server, and not checked by anyone.
    pub uploaded: Vec<String>,
    pub scripts: usize,
    pub configs: usize,
    pub other: usize,
    /// Modrinth couldn't be asked, so every jar is in `uploaded`.
    pub unchecked: bool,
}

impl Advisory {
    /// Whether nothing that runs as code would be installed.
    pub fn is_empty(&self) -> bool {
        self.on_modrinth.is_empty() && self.uploaded.is_empty() && self.scripts == 0
    }

    /// "3 config files, 2 script files, 1 other file", or `None`.
    pub fn other_files_label(&self) -> Option<String> {
        let plural =
            |n: usize, word: &str| format!("{n} {word} file{}", if n == 1 { "" } else { "s" });
        let parts: Vec<String> = [
            (self.configs, "config"),
            (self.scripts, "script"),
            (self.other, "other"),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, word)| plural(n, word))
        .collect();
        (!parts.is_empty()).then(|| parts.join(", "))
    }
}

/// Groups `files` for the advisory. Jars are looked up on Modrinth by SHA-1 (the content
/// tab's hash matching); a file counts as on Modrinth when Modrinth has a version file with
/// its SHA-1 and, when both are known, the same SHA-512. Without `api`, or when Modrinth
/// can't be reached, every jar is listed as uploaded.
pub async fn advisory(api: Option<&Modrinth>, files: &[PulledFile]) -> Advisory {
    let mut advisory = Advisory::default();
    let mut code: Vec<&PulledFile> = Vec::new();
    for file in files {
        match file_role(&file.path) {
            FileRole::Code => code.push(file),
            FileRole::Script => advisory.scripts += 1,
            FileRole::Config => advisory.configs += 1,
            FileRole::Other => advisory.other += 1,
        }
    }
    let hashes: Vec<String> = code
        .iter()
        .map(|f| f.sha1.to_ascii_lowercase())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let found = match api {
        Some(api) if !hashes.is_empty() => crate::content::listing::lookup(api, &hashes).await.ok(),
        Some(_) => Some(Default::default()),
        None => None,
    };
    advisory.unchecked = found.is_none() && !code.is_empty();
    for file in code {
        let sha1 = file.sha1.to_ascii_lowercase();
        let matched = found
            .as_ref()
            .and_then(|found| found.get(&sha1))
            .filter(|(version, _)| {
                let listed = version.files.iter().find(|f| {
                    f.hashes
                        .sha1
                        .as_deref()
                        .is_some_and(|h| h.eq_ignore_ascii_case(&sha1))
                });
                match (
                    listed.and_then(|f| f.hashes.sha512.as_deref()),
                    &file.sha512,
                ) {
                    (Some(theirs), Some(ours)) => theirs.eq_ignore_ascii_case(ours),
                    _ => listed.is_some(),
                }
            });
        match matched {
            Some((version, project)) => advisory.on_modrinth.push(ModrinthMatch {
                path: file.path.clone(),
                project: project
                    .as_ref()
                    .map_or_else(|| version.project_id.clone(), |p| p.title.clone()),
                version: version.version_number.clone(),
            }),
            None => advisory.uploaded.push(file.path.clone()),
        }
    }
    advisory.on_modrinth.sort_by(|a, b| a.path.cmp(&b.path));
    advisory.uploaded.sort();
    advisory
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_code_needs_the_advisory_and_only_from_untrusted_cores() {
        let mut core = SavedCore {
            uuid: "c".into(),
            url: "http://localhost:16662".into(),
            ..SavedCore::default()
        };
        for kind in [
            PullKind::ClientPack,
            PullKind::ModSync,
            PullKind::Jars,
            PullKind::Scripts,
        ] {
            assert!(needs_advisory(&core, kind), "{kind:?}");
        }
        for kind in [PullKind::World, PullKind::ResourcePack, PullKind::Status] {
            assert!(!needs_advisory(&core, kind), "{kind:?}");
        }
        core.set_trusted(true);
        for kind in [
            PullKind::ClientPack,
            PullKind::ModSync,
            PullKind::Jars,
            PullKind::Scripts,
            PullKind::World,
            PullKind::ResourcePack,
        ] {
            assert!(!needs_advisory(&core, kind), "{kind:?}");
        }
    }

    #[test]
    fn roles() {
        assert_eq!(file_role("mods/sodium.jar"), FileRole::Code);
        assert_eq!(file_role("mods/sodium.jar.disabled"), FileRole::Code);
        assert_eq!(file_role("config/nested/lib.JAR"), FileRole::Code);
        assert_eq!(file_role("kubejs/server_scripts/a.js"), FileRole::Script);
        assert_eq!(file_role("scripts/recipes.zs"), FileRole::Script);
        assert_eq!(file_role("config/sodium.json"), FileRole::Config);
        assert_eq!(file_role("options.txt"), FileRole::Config);
        assert_eq!(file_role("resourcepacks/pack.zip"), FileRole::Other);
    }

    #[test]
    fn without_modrinth_every_jar_is_unchecked() {
        let files = [
            PulledFile {
                path: "mods/a.jar".into(),
                sha1: "00".into(),
                sha512: None,
            },
            PulledFile {
                path: "config/a.toml".into(),
                sha1: "01".into(),
                sha512: None,
            },
        ];
        let advisory = futures::executor::block_on(advisory(None, &files));
        assert!(advisory.unchecked);
        assert_eq!(advisory.uploaded, ["mods/a.jar"]);
        assert_eq!(advisory.configs, 1);
        assert_eq!(
            advisory.other_files_label().as_deref(),
            Some("1 config file")
        );
        assert!(!advisory.is_empty());
    }
}
