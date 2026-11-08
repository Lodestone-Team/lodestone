//! Turning other launchers' version ids and loader versions into this launcher's.
//!
//! The official launcher names a modded version after the loader's installer
//! (`fabric-loader-0.16.14-1.20.1`, `1.20.1-forge-47.4.28`, `neoforge-21.1.209`, ...);
//! [`parse_version_id`] reads those. The other launchers store the loader version
//! separately, in their own form; [`resolve_loader`] finds it in the loader's version
//! list (M4's), or the closest one when it isn't there.

use std::cmp::Ordering;

use serde_json::Value;

use super::{WantedLoader, WantedVersion};
use crate::game::loaders::{
    LoaderChoice, LoaderKind, LoaderVersion,
    forge::{forge_display, neoforge_game_version},
    meta::compare_versions,
};

/// What a version id (and its version JSON, when installed) says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedVersion {
    pub game_version: WantedVersion,
    pub loader: Option<WantedLoader>,
    pub notes: Vec<String>,
}

/// Reads an official launcher version id. `version_json` is
/// `versions/<id>/<id>.json` when it exists: its `inheritsFrom` is the Minecraft version,
/// whatever the id looks like.
pub fn parse_version_id(id: &str, version_json: Option<&Value>) -> ParsedVersion {
    let inherits = version_json
        .and_then(|v| v.get("inheritsFrom"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let neoforged_libraries = version_json
        .and_then(|v| v.get("libraries"))
        .and_then(Value::as_array)
        .is_some_and(|libs| {
            libs.iter().any(|l| {
                l.get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|n| n.starts_with("net.neoforged"))
            })
        });
    let mut notes = Vec::new();
    let lower = id.to_ascii_lowercase();
    let exact = |version: String| WantedVersion::Exact(version);

    // Fabric and Quilt: `<prefix><loader>-<mc>`.
    for (prefix, kind) in [
        ("fabric-loader-", LoaderKind::Fabric),
        ("quilt-loader-", LoaderKind::Quilt),
    ] {
        if lower.starts_with(prefix) {
            let rest = &id[prefix.len()..];
            let (loader, game) = match &inherits {
                Some(game) => (
                    rest.strip_suffix(game.as_str())
                        .and_then(|l| l.strip_suffix('-'))
                        .unwrap_or(rest)
                        .to_string(),
                    game.clone(),
                ),
                None => split_loader_game(rest),
            };
            return ParsedVersion {
                game_version: exact(game),
                loader: Some(WantedLoader::new(kind, Some(&loader))),
                notes,
            };
        }
    }
    if lower.starts_with("neoforge-") {
        let version = &id["neoforge-".len()..];
        let game = inherits
            .clone()
            .or_else(|| neoforge_game_version(version))
            .unwrap_or_default();
        return ParsedVersion {
            game_version: exact(game),
            loader: Some(WantedLoader::new(LoaderKind::NeoForge, Some(version))),
            notes,
        };
    }
    if let Some(at) = lower.find("-forge") {
        let game = inherits.clone().unwrap_or_else(|| id[..at].to_string());
        let rest = &id[at + "-forge".len()..];
        let rest = rest.strip_prefix('-').unwrap_or(rest);
        let rest = rest
            .strip_prefix(game.as_str())
            .and_then(|r| r.strip_prefix('-'))
            .unwrap_or(rest);
        let build = rest
            .strip_suffix(game.as_str())
            .and_then(|r| r.strip_suffix('-'))
            .unwrap_or(rest);
        if neoforged_libraries {
            notes.push(format!(
                "This is NeoForge for Minecraft {game}, which Lodestone Launcher runs as Forge {build}: NeoForge's 1.20.1 builds are Forge 47.1 under another name."
            ));
        }
        return ParsedVersion {
            game_version: exact(game),
            loader: Some(WantedLoader::new(LoaderKind::Forge, Some(build))),
            notes,
        };
    }
    for (marker, what) in [("-optifine", "OptiFine"), ("-liteloader", "LiteLoader")] {
        if let Some(at) = lower.find(marker) {
            let game = inherits.clone().unwrap_or_else(|| id[..at].to_string());
            notes.push(format!(
                "{what} isn't a loader Lodestone Launcher installs, so the instance is vanilla {game}. {}",
                if what == "OptiFine" {
                    "Add OptiFine's jar to its mods with a loader, or use Iris for shaders."
                } else {
                    "LiteLoader mods won't load."
                }
            ));
            return ParsedVersion {
                game_version: exact(game),
                loader: None,
                notes,
            };
        }
    }
    match inherits {
        Some(game) if game != id => {
            notes.push(format!(
                "{id} is a version Lodestone Launcher doesn't know; the instance runs vanilla {game}."
            ));
            ParsedVersion {
                game_version: exact(game),
                loader: None,
                notes,
            }
        }
        _ => ParsedVersion {
            game_version: exact(id.to_string()),
            loader: None,
            notes,
        },
    }
}

/// `0.16.14-1.20.1` → (`0.16.14`, `1.20.1`); `0.26.0-beta.1-1.20.1` → (`0.26.0-beta.1`,
/// `1.20.1`): the split is at the first `-` followed by a digit.
fn split_loader_game(rest: &str) -> (String, String) {
    for (at, _) in rest.match_indices('-') {
        if rest[at + 1..].starts_with(|c: char| c.is_ascii_digit()) {
            return (rest[..at].to_string(), rest[at + 1..].to_string());
        }
    }
    (rest.to_string(), String::new())
}

/// The loader version picked for an instance, and what to say about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoaderResolution {
    pub choice: Option<LoaderChoice>,
    pub note: Option<String>,
}

/// `version` without the forms other launchers wrap Forge builds in: `forge-47.4.0`,
/// `1.20.1-47.4.0`, `1.7.10-10.13.4.1614-1.7.10`.
fn bare(kind: LoaderKind, version: &str, game_version: &str) -> String {
    let mut v = version.trim();
    for prefix in [
        "forge-",
        "neoforge-",
        "fabric-loader-",
        "quilt-loader-",
        "fabric-",
        "quilt-",
    ] {
        if let Some(rest) = v.strip_prefix(prefix) {
            v = rest;
        }
    }
    if matches!(kind, LoaderKind::Fabric | LoaderKind::Quilt) {
        if let Some(rest) = v
            .strip_suffix(game_version)
            .and_then(|r| r.strip_suffix('-'))
        {
            v = rest;
        }
        return v.to_string();
    }
    if let Some(rest) = v
        .strip_prefix(game_version)
        .and_then(|r| r.strip_prefix('-'))
    {
        v = rest;
    }
    if let Some(rest) = v
        .strip_suffix(game_version)
        .and_then(|r| r.strip_suffix('-'))
    {
        v = rest;
    }
    v.to_string()
}

/// The entry's version as other launchers write it (Forge's build without the Minecraft
/// version).
fn display_key(kind: LoaderKind, entry: &LoaderVersion, game_version: &str) -> String {
    match kind {
        LoaderKind::Forge => forge_display(&entry.id, game_version),
        _ => entry.id.clone(),
    }
}

/// Finds `wanted`'s version in `list` (the loader's versions for `game_version`, newest
/// first): the exact one, else the newest one that isn't newer, else the recommended one.
/// An empty list means the loader doesn't run on that version: the instance is vanilla.
pub fn resolve_loader(
    wanted: &WantedLoader,
    game_version: &str,
    list: &[LoaderVersion],
) -> LoaderResolution {
    let kind = wanted.kind;
    let label = kind.label();
    if list.is_empty() {
        return LoaderResolution {
            choice: None,
            note: Some(format!(
                "{label} has no build for Minecraft {game_version}, so the instance is vanilla and its mods won't load."
            )),
        };
    }
    let choose = |entry: &LoaderVersion| {
        Some(LoaderChoice {
            kind,
            version: entry.id.clone(),
        })
    };
    let default = list
        .iter()
        .find(|v| v.recommended)
        .or_else(|| list.iter().find(|v| v.stable))
        .unwrap_or(&list[0]);
    let Some(version) = &wanted.version else {
        return LoaderResolution {
            choice: choose(default),
            note: Some(format!(
                "The launcher didn't say which {label} version; using {} {}.",
                label, default.display
            )),
        };
    };
    let key = bare(kind, version, game_version);
    if let Some(entry) = list
        .iter()
        .find(|e| e.id == *version || e.id == key || display_key(kind, e, game_version) == key)
    {
        return LoaderResolution {
            choice: choose(entry),
            note: None,
        };
    }
    let closest = list
        .iter()
        .filter(|e| {
            compare_versions(&display_key(kind, e, game_version), &key) != Ordering::Greater
        })
        .max_by(|a, b| {
            compare_versions(
                &display_key(kind, a, game_version),
                &display_key(kind, b, game_version),
            )
        })
        .unwrap_or(default);
    LoaderResolution {
        choice: choose(closest),
        note: Some(format!(
            "{label} {key} isn't in {label}'s list for Minecraft {game_version}; using {}, the closest.",
            closest.display
        )),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn loader(kind: LoaderKind, version: &str) -> Option<WantedLoader> {
        Some(WantedLoader::new(kind, Some(version)))
    }

    fn parsed(id: &str) -> (String, Option<WantedLoader>) {
        let p = parse_version_id(id, None);
        let WantedVersion::Exact(game) = p.game_version else {
            panic!("{id}");
        };
        (game, p.loader)
    }

    #[test]
    fn vanilla_version_ids() {
        assert_eq!(parsed("1.20.1"), ("1.20.1".into(), None));
        assert_eq!(parsed("24w14a"), ("24w14a".into(), None));
        assert_eq!(
            parsed("fabric-loader-0.16.14-1.20.1"),
            ("1.20.1".into(), loader(LoaderKind::Fabric, "0.16.14"))
        );
        assert_eq!(
            parsed("fabric-loader-0.15.11-1.20.5-pre1"),
            ("1.20.5-pre1".into(), loader(LoaderKind::Fabric, "0.15.11"))
        );
        assert_eq!(
            parsed("quilt-loader-0.26.0-beta.1-1.20.1"),
            ("1.20.1".into(), loader(LoaderKind::Quilt, "0.26.0-beta.1"))
        );
        assert_eq!(
            parsed("1.20.1-forge-47.4.28"),
            ("1.20.1".into(), loader(LoaderKind::Forge, "47.4.28"))
        );
        assert_eq!(
            parsed("1.12.2-forge1.12.2-14.23.5.2847"),
            ("1.12.2".into(), loader(LoaderKind::Forge, "14.23.5.2847"))
        );
        assert_eq!(
            parsed("1.12.2-forge-14.23.5.2860"),
            ("1.12.2".into(), loader(LoaderKind::Forge, "14.23.5.2860"))
        );
        assert_eq!(
            parsed("1.7.10-Forge10.13.4.1614-1.7.10"),
            ("1.7.10".into(), loader(LoaderKind::Forge, "10.13.4.1614"))
        );
        assert_eq!(
            parsed("1.8.9-forge1.8.9-11.15.1.2318-1.8.9"),
            ("1.8.9".into(), loader(LoaderKind::Forge, "11.15.1.2318"))
        );
        assert_eq!(
            parsed("neoforge-21.1.209"),
            ("1.21.1".into(), loader(LoaderKind::NeoForge, "21.1.209"))
        );
        let optifine = parse_version_id("1.20.1-OptiFine_HD_U_I6", None);
        assert_eq!(optifine.game_version, WantedVersion::Exact("1.20.1".into()));
        assert_eq!(optifine.loader, None);
        assert!(optifine.notes[0].contains("OptiFine"));
    }

    /// The version JSON's `inheritsFrom` beats the id, and `net.neoforged` libraries mark
    /// a 1.20.1 NeoForge.
    #[test]
    fn version_json_wins() {
        let json = json!({"inheritsFrom": "1.20.1", "libraries": [{"name": "net.neoforged:mergetool:2.0.0"}]});
        let p = parse_version_id("1.20.1-forge-47.1.106", Some(&json));
        assert_eq!(p.loader, loader(LoaderKind::Forge, "47.1.106"));
        assert!(p.notes[0].contains("NeoForge"));
        let custom = parse_version_id("My Pack", Some(&json!({"inheritsFrom": "1.19.2"})));
        assert_eq!(custom.game_version, WantedVersion::Exact("1.19.2".into()));
        assert_eq!(custom.loader, None);
        assert_eq!(custom.notes.len(), 1);
        let fabric = parse_version_id(
            "fabric-loader-0.16.14-1.21",
            Some(&json!({"inheritsFrom": "1.21"})),
        );
        assert_eq!(fabric.loader, loader(LoaderKind::Fabric, "0.16.14"));
    }

    fn entry(id: &str, display: &str, recommended: bool) -> LoaderVersion {
        LoaderVersion {
            id: id.into(),
            display: display.into(),
            stable: true,
            recommended,
        }
    }

    #[test]
    fn resolves_loader_versions() {
        let forge = [
            entry("1.20.1-47.4.28", "47.4.28", false),
            entry("1.20.1-47.4.10", "47.4.10", true),
            entry("1.20.1-47.1.3", "47.1.3", false),
        ];
        for wanted in ["47.4.10", "1.20.1-47.4.10", "forge-47.4.10"] {
            let r = resolve_loader(
                &WantedLoader::new(LoaderKind::Forge, Some(wanted)),
                "1.20.1",
                &forge,
            );
            assert_eq!(r.choice.unwrap().version, "1.20.1-47.4.10", "{wanted}");
            assert_eq!(r.note, None);
        }
        // Not in the list: the newest one that isn't newer.
        let r = resolve_loader(
            &WantedLoader::new(LoaderKind::Forge, Some("47.1.106")),
            "1.20.1",
            &forge,
        );
        assert_eq!(r.choice.unwrap().version, "1.20.1-47.1.3");
        assert!(r.note.unwrap().contains("closest"));
        // Older than everything: the recommended one.
        let r = resolve_loader(
            &WantedLoader::new(LoaderKind::Forge, Some("46.0.1")),
            "1.20.1",
            &forge,
        );
        assert_eq!(r.choice.unwrap().version, "1.20.1-47.4.10");
        // No version given: the recommended one.
        let r = resolve_loader(
            &WantedLoader::new(LoaderKind::Forge, None),
            "1.20.1",
            &forge,
        );
        assert_eq!(r.choice.unwrap().version, "1.20.1-47.4.10");
        // Legacy Forge.
        let legacy = [entry("1.7.10-10.13.4.1614-1.7.10", "10.13.4.1614", true)];
        let r = resolve_loader(
            &WantedLoader::new(LoaderKind::Forge, Some("10.13.4.1614")),
            "1.7.10",
            &legacy,
        );
        assert_eq!(r.choice.unwrap().version, "1.7.10-10.13.4.1614-1.7.10");
        // Fabric, in GDLauncher's form.
        let fabric = [
            entry("0.16.14", "0.16.14", true),
            entry("0.16.9", "0.16.9", false),
        ];
        let r = resolve_loader(
            &WantedLoader::new(LoaderKind::Fabric, Some("fabric-0.16.9-1.20.1")),
            "1.20.1",
            &fabric,
        );
        assert_eq!(r.choice.unwrap().version, "0.16.9");
        // No build for that Minecraft version.
        let r = resolve_loader(
            &WantedLoader::new(LoaderKind::Fabric, Some("0.16.14")),
            "1.8.9",
            &[],
        );
        assert_eq!(r.choice, None);
        assert!(r.note.unwrap().contains("vanilla"));
    }
}
