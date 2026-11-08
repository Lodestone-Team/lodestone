//! Mod loaders: Fabric, Quilt, Forge and NeoForge.
//!
//! A loader version is installed as a *profile*: a version file in Mojang's format
//! that names the vanilla version it adds to (`inheritsFrom`), with its own main
//! class, arguments and libraries. Each profile is normalised once, when it is first
//! installed, so that every library has a path, a URL (or none, for files the
//! loader's installer provides) and a SHA-1, and is kept at
//! `versions/<id>/<id>.json`. Later launches read it without the network.
//!
//! - [`meta`]: Fabric's and Quilt's meta servers, which serve profiles directly;
//! - [`forge`]: Forge's and NeoForge's installers: the version lists, the installer
//!   jar's `install_profile.json` and `version.json`, and old Forge's single-file
//!   profiles;
//! - [`processors`]: the installer's processors (Forge 1.13 and later, NeoForge),
//!   which patch the Minecraft jar, run with the instance's Java;
//! - [`merge`]: a profile merged onto its vanilla version, as the official launcher
//!   does with `inheritsFrom`.

pub mod forge;
pub mod meta;
pub mod processors;

use serde::{Deserialize, Serialize};

use super::version::{Arguments, Library, VersionJson};

/// A mod loader.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LoaderKind {
    Fabric,
    Quilt,
    Forge,
    #[serde(rename = "neoforge")]
    NeoForge,
}

impl LoaderKind {
    pub const ALL: [LoaderKind; 4] = [
        LoaderKind::Fabric,
        LoaderKind::Quilt,
        LoaderKind::Forge,
        LoaderKind::NeoForge,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LoaderKind::Fabric => "Fabric",
            LoaderKind::Quilt => "Quilt",
            LoaderKind::Forge => "Forge",
            LoaderKind::NeoForge => "NeoForge",
        }
    }

    /// The key in a `.mrpack`'s `dependencies`.
    pub fn mrpack_key(self) -> &'static str {
        match self {
            LoaderKind::Fabric => "fabric-loader",
            LoaderKind::Quilt => "quilt-loader",
            LoaderKind::Forge => "forge",
            LoaderKind::NeoForge => "neoforge",
        }
    }

    /// Whether the loader is installed by running its installer's processors.
    pub fn has_installer(self) -> bool {
        matches!(self, LoaderKind::Forge | LoaderKind::NeoForge)
    }
}

/// A loader and its version, as an instance stores them.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LoaderChoice {
    pub kind: LoaderKind,
    /// Fabric and Quilt: the loader version (`0.16.14`). Forge: the full Maven version
    /// (`1.20.1-47.4.28`, or `1.8.9-11.15.1.2318-1.8.9` for old builds). NeoForge:
    /// its version (`21.1.209`).
    pub version: String,
}

/// One entry of a loader's version list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoaderVersion {
    /// What the instance stores, [`LoaderChoice::version`].
    pub id: String,
    /// What the list shows: `47.4.28` rather than `1.20.1-47.4.28`.
    pub display: String,
    /// Not a beta or pre-release.
    pub stable: bool,
    /// The loader's own recommendation for this Minecraft version (Forge's
    /// "recommended" promotion), or the newest stable build.
    pub recommended: bool,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LoaderError {
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    Invalid(String),
    #[error("{loader} {version} is for Minecraft {wanted}, not {game}")]
    WrongGame {
        loader: &'static str,
        version: String,
        wanted: String,
        game: String,
    },
}

/// The folder name (and `${version_name}`) of a loader version's profile for
/// `game_version`. It is derived from the choice, so an installed profile can be found
/// without the network.
pub fn profile_id(choice: &LoaderChoice, game_version: &str) -> Result<String, LoaderError> {
    let version = &choice.version;
    let valid = !version.is_empty()
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_'));
    if !valid {
        return Err(LoaderError::Invalid(format!(
            "{version:?} is not a valid {} version",
            choice.kind.label()
        )));
    }
    Ok(match choice.kind {
        LoaderKind::Fabric => format!("fabric-loader-{version}-{game_version}"),
        LoaderKind::Quilt => format!("quilt-loader-{version}-{game_version}"),
        LoaderKind::Forge => {
            let build = version
                .strip_prefix(game_version)
                .and_then(|rest| rest.strip_prefix('-'))
                .filter(|rest| !rest.is_empty())
                .ok_or_else(|| LoaderError::WrongGame {
                    loader: "Forge",
                    version: version.clone(),
                    wanted: version.split('-').next().unwrap_or_default().to_string(),
                    game: game_version.to_string(),
                })?;
            format!("{game_version}-forge-{build}")
        }
        LoaderKind::NeoForge => {
            match forge::neoforge_game_version(version) {
                Some(wanted) if wanted == game_version => {}
                wanted => {
                    return Err(LoaderError::WrongGame {
                        loader: "NeoForge",
                        version: version.clone(),
                        wanted: wanted.unwrap_or_else(|| "another version".into()),
                        game: game_version.to_string(),
                    });
                }
            }
            format!("neoforge-{version}")
        }
    })
}

/// `group:artifact[:classifier][@extension]` without the version: two libraries with
/// the same key are the same library at different versions.
pub fn library_key(name: &str) -> String {
    let (coordinate, extension) = match name.split_once('@') {
        Some((coordinate, extension)) => (coordinate, Some(extension)),
        None => (name, None),
    };
    let parts: Vec<&str> = coordinate.split(':').collect();
    let mut key = match parts.as_slice() {
        [group, artifact, _version, classifier] => format!("{group}:{artifact}:{classifier}"),
        [group, artifact, ..] => format!("{group}:{artifact}"),
        _ => coordinate.to_string(),
    };
    if let Some(extension) = extension {
        key.push('@');
        key.push_str(extension);
    }
    key
}

/// `child` (a loader profile) on top of `parent` (its vanilla version), as the official
/// launcher merges `inheritsFrom`:
/// - libraries: the child's first, then the parent's, without parent libraries the
///   child also lists (at another version);
/// - arguments: the parent's, then the child's; `minecraftArguments`, the main class
///   and the id from the child;
/// - the asset index, downloads, Java version and logging config from the parent,
///   unless the child has its own. Forge's profiles have an empty `logging`, so
///   Mojang's patched Log4j config stays in use.
///
/// The merged version keeps the child's SHA-1, so its install record is the loader's.
pub fn merge(child: &VersionJson, parent: &VersionJson) -> VersionJson {
    let child_keys: std::collections::HashSet<String> = child
        .libraries
        .iter()
        .map(|library| library_key(&library.name))
        .collect();
    let libraries: Vec<Library> = child
        .libraries
        .iter()
        .cloned()
        .chain(
            parent
                .libraries
                .iter()
                .filter(|library| !child_keys.contains(&library_key(&library.name)))
                .cloned(),
        )
        .collect();
    let arguments = match (&parent.arguments, &child.arguments) {
        (Some(parent), Some(child)) => Some(Arguments {
            game: parent.game.iter().chain(&child.game).cloned().collect(),
            jvm: parent.jvm.iter().chain(&child.jvm).cloned().collect(),
            default_user_jvm: if child.default_user_jvm.is_empty() {
                parent.default_user_jvm.clone()
            } else {
                child.default_user_jvm.clone()
            },
        }),
        (parent, child) => child.clone().or_else(|| parent.clone()),
    };
    let logging = match child.client_logging() {
        Some(_) => child.logging.clone(),
        None => parent.logging.clone(),
    };
    VersionJson {
        id: child.id.clone(),
        kind: parent.kind,
        main_class: child.main_class.clone(),
        inherits_from: None,
        arguments,
        minecraft_arguments: child
            .minecraft_arguments
            .clone()
            .or_else(|| parent.minecraft_arguments.clone()),
        libraries,
        asset_index: child
            .asset_index
            .clone()
            .or_else(|| parent.asset_index.clone()),
        assets: parent.assets.clone().or_else(|| child.assets.clone()),
        downloads: if child.downloads.is_empty() {
            parent.downloads.clone()
        } else {
            child.downloads.clone()
        },
        java_version: child
            .java_version
            .clone()
            .or_else(|| parent.java_version.clone()),
        logging,
        sha1: child.sha1.clone(),
    }
}

/// The SHA-1 in a Maven `.sha1` file: the first word, if it is 40 hex digits.
pub fn parse_sha1_file(text: &str) -> Option<String> {
    let word = text.split_whitespace().next()?;
    (word.len() == 40 && word.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| word.to_ascii_lowercase())
}

/// The URL of a file in a Maven repository at `base`.
pub fn maven_url(base: &str, path: &str) -> String {
    format!("{}/{path}", base.trim_end_matches('/'))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::game::version::tests::fixture;

    pub fn loader_fixture(path: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/testdata/loaders/{path}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn profile_ids() {
        let id = |kind, version: &str, game| {
            profile_id(
                &LoaderChoice {
                    kind,
                    version: version.into(),
                },
                game,
            )
        };
        assert_eq!(
            id(LoaderKind::Fabric, "0.16.14", "1.20.1").unwrap(),
            "fabric-loader-0.16.14-1.20.1"
        );
        assert_eq!(
            id(LoaderKind::Quilt, "0.29.2", "1.20.1").unwrap(),
            "quilt-loader-0.29.2-1.20.1"
        );
        assert_eq!(
            id(LoaderKind::Forge, "1.20.1-47.4.28", "1.20.1").unwrap(),
            "1.20.1-forge-47.4.28"
        );
        assert_eq!(
            id(LoaderKind::Forge, "1.8.9-11.15.1.2318-1.8.9", "1.8.9").unwrap(),
            "1.8.9-forge-11.15.1.2318-1.8.9"
        );
        assert_eq!(
            id(LoaderKind::NeoForge, "21.1.209", "1.21.1").unwrap(),
            "neoforge-21.1.209"
        );
        assert!(matches!(
            id(LoaderKind::Forge, "1.20.1-47.4.28", "1.21.1"),
            Err(LoaderError::WrongGame { .. })
        ));
        // 1.20 is not 1.20.1.
        assert!(id(LoaderKind::Forge, "1.20-46.0.14", "1.20.1").is_err());
        assert!(id(LoaderKind::NeoForge, "21.1.209", "1.21").is_err());
        for bad in ["", "../x", "1.0/2", "a b"] {
            assert!(id(LoaderKind::Fabric, bad, "1.20.1").is_err(), "{bad}");
        }
    }

    #[test]
    fn library_keys_ignore_the_version() {
        assert_eq!(
            library_key("org.apache.logging.log4j:log4j-core:2.15.0"),
            "org.apache.logging.log4j:log4j-core"
        );
        assert_eq!(
            library_key("org.lwjgl:lwjgl:3.3.1:natives-macos-arm64"),
            "org.lwjgl:lwjgl:natives-macos-arm64"
        );
        assert_eq!(
            library_key("de.oceanlabs.mcp:mcp_config:1.20.1-20230612.114412@zip"),
            "de.oceanlabs.mcp:mcp_config@zip"
        );
    }

    #[test]
    fn sha1_files() {
        assert_eq!(
            parse_sha1_file("97D0BFF94981E37BD7A4362DEEE53C9A84E3FB21  intermediary.jar\n"),
            Some("97d0bff94981e37bd7a4362deee53c9a84e3fb21".into())
        );
        assert_eq!(parse_sha1_file("<html>not found</html>"), None);
        assert_eq!(parse_sha1_file(""), None);
    }

    /// Forge 1.12.2 replaces vanilla's Log4j 2.8.1 with 2.15.0, keeps Mojang's logging
    /// config, and replaces the game arguments.
    #[test]
    fn merges_forge_1_12_2_onto_vanilla() {
        let vanilla = fixture("1.12.2");
        let forge = forge::tests::forge_profile("1.12.2-14.23.5.2864", "1.12.2");
        let merged = merge(&forge, &vanilla);
        assert_eq!(merged.id, "1.12.2-forge-14.23.5.2864");
        assert_eq!(merged.main_class, "net.minecraft.launchwrapper.Launch");
        assert!(
            merged
                .minecraft_arguments
                .as_deref()
                .unwrap()
                .contains("--tweakClass net.minecraftforge.fml.common.launcher.FMLTweaker")
        );
        let log4j: Vec<&str> = merged
            .libraries
            .iter()
            .map(|l| l.name.as_str())
            .filter(|name| name.starts_with("org.apache.logging.log4j:log4j-core"))
            .collect();
        assert_eq!(log4j, ["org.apache.logging.log4j:log4j-core:2.15.0"]);
        assert_eq!(
            merged.libraries[0].name,
            "net.minecraftforge:forge:1.12.2-14.23.5.2864"
        );
        assert_eq!(merged.client_logging().unwrap().file.id, "client-1.12.xml");
        assert_eq!(merged.assets_id(), "1.12");
        assert_eq!(merged.runtime_component(), "jre-legacy");
        assert!(merged.client_download().is_some());
        assert!(merged.uses_log4j());
    }

    /// Fabric adds its JVM argument after vanilla's, and keeps vanilla's game ones.
    #[test]
    fn merges_fabric_onto_vanilla() {
        let vanilla = fixture("1.20.1");
        let fabric = meta::tests::fabric_profile();
        let merged = merge(&fabric, &vanilla);
        let arguments = merged.arguments.as_ref().unwrap();
        assert_eq!(
            arguments.jvm.len(),
            vanilla.arguments.as_ref().unwrap().jvm.len() + 1
        );
        assert_eq!(
            arguments.game.len(),
            vanilla.arguments.as_ref().unwrap().game.len()
        );
        assert_eq!(
            merged.main_class,
            "net.fabricmc.loader.impl.launch.knot.KnotClient"
        );
        assert_eq!(
            merged.libraries.len(),
            fabric.libraries.len() + vanilla.libraries.len()
        );
        assert_eq!(merged.runtime_component(), "java-runtime-gamma");
        assert_eq!(merged.sha1, fabric.sha1);
    }
}
