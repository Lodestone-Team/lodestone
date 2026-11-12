//! NeoForge's version list (`maven.neoforged.net`), which names versions after the
//! Minecraft version they are for.
//!
//! Only NeoForge's own artifact (`net.neoforged:neoforge`, Minecraft 1.20.2 and later) is
//! read. NeoForge for 1.20.1 was a Forge fork published as `net.neoforged:forge`, and is
//! not included.

use serde::Deserialize;

use super::compare_versions;
use crate::Error;

/// NeoForge's releases, oldest first.
pub const NEOFORGE_VERSIONS_URL: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";

/// The answer of [`NEOFORGE_VERSIONS_URL`].
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NeoForgeVersions {
    #[serde(default)]
    pub versions: Vec<String>,
}

/// The URL of NeoForge `version`'s installer.
pub fn neoforge_installer_url(version: &str) -> String {
    format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"
    )
}

/// The Minecraft version a NeoForge version is for: `21.1.209` is 1.21.1, `21.0.167`
/// is 1.21, `20.2.3-beta` is 1.20.2, and since Minecraft's 2026 numbering
/// `26.3.0.101` is 26.3 and `26.1.1.4` is 26.1.1. Snapshot builds have none.
pub fn neoforge_game_version(version: &str) -> Option<String> {
    let numbers: Vec<u32> = version
        .split(['-', '+'])
        .next()?
        .split('.')
        .map(|part| part.parse().ok())
        .collect::<Option<_>>()?;
    match numbers.as_slice() {
        [major, minor, patch, _build] if *major >= 26 => Some(if *patch == 0 {
            format!("{major}.{minor}")
        } else {
            format!("{major}.{minor}.{patch}")
        }),
        [major, minor, _build] if (20..26).contains(major) => Some(if *minor == 0 {
            format!("1.{major}")
        } else {
            format!("1.{major}.{minor}")
        }),
        _ => None,
    }
}

/// Whether a NeoForge version is a stable release (not a beta or alpha).
pub fn is_stable_neoforge(version: &str) -> bool {
    !version.contains("beta") && !version.contains("alpha")
}

/// NeoForge's versions for `game_version`, newest first.
pub fn neoforge_versions_for(list: &NeoForgeVersions, game_version: &str) -> Vec<String> {
    let mut versions: Vec<String> = list
        .versions
        .iter()
        .filter(|v| neoforge_game_version(v).as_deref() == Some(game_version))
        .cloned()
        .collect();
    versions.sort_by(|a, b| compare_versions(b, a));
    versions
}

/// The newest NeoForge version for `game_version`: the newest stable one, else the
/// newest beta (a new Minecraft version only has betas at first).
pub fn newest_neoforge_build(list: &NeoForgeVersions, game_version: &str) -> Result<String, Error> {
    let versions = neoforge_versions_for(list, game_version);
    versions
        .iter()
        .find(|v| is_stable_neoforge(v))
        .or_else(|| versions.first())
        .cloned()
        .ok_or_else(|| {
            Error::NotFound(format!(
                "NeoForge has no builds for Minecraft {game_version}"
            ))
        })
}

/// The Minecraft versions NeoForge has builds for, newest first.
pub fn neoforge_minecraft_versions(list: &NeoForgeVersions) -> Vec<String> {
    let mut versions: Vec<String> = Vec::new();
    for version in &list.versions {
        if let Some(game) = neoforge_game_version(version) {
            if !versions.contains(&game) {
                versions.push(game);
            }
        }
    }
    versions.sort_by(|a, b| compare_versions(b, a));
    versions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(versions: &[&str]) -> NeoForgeVersions {
        NeoForgeVersions {
            versions: versions.iter().map(|v| v.to_string()).collect(),
        }
    }

    #[test]
    fn game_versions() {
        for (neo, game) in [
            ("21.1.209", Some("1.21.1")),
            ("21.0.167", Some("1.21")),
            ("20.2.3-beta", Some("1.20.2")),
            ("20.4.237", Some("1.20.4")),
            ("26.3.0.101", Some("26.3")),
            ("26.1.1.4", Some("26.1.1")),
            ("0.25w14craftmine.3-beta", None),
            ("47.1.0", None),
        ] {
            assert_eq!(neoforge_game_version(neo).as_deref(), game, "{neo}");
        }
    }

    #[test]
    fn newest_build_prefers_stable() {
        let list = list(&[
            "20.2.3-beta",
            "21.1.1-beta",
            "21.1.9",
            "21.1.10",
            "21.1.11-beta",
            "21.2.0-beta",
        ]);
        assert_eq!(newest_neoforge_build(&list, "1.21.1").unwrap(), "21.1.10");
        // only betas: the newest beta
        assert_eq!(
            newest_neoforge_build(&list, "1.21.2").unwrap(),
            "21.2.0-beta"
        );
        assert_eq!(
            newest_neoforge_build(&list, "1.20.2").unwrap(),
            "20.2.3-beta"
        );
        assert!(matches!(
            newest_neoforge_build(&list, "1.19.2"),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn minecraft_versions_newest_first() {
        let list = list(&[
            "20.2.3-beta",
            "20.4.1",
            "21.0.1",
            "21.1.1",
            "21.1.2",
            "26.1.0.3",
        ]);
        assert_eq!(
            neoforge_minecraft_versions(&list),
            ["26.1", "1.21.1", "1.21", "1.20.4", "1.20.2"]
        );
    }

    #[test]
    fn installer_url() {
        assert_eq!(
            neoforge_installer_url("21.1.209"),
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.209/neoforge-21.1.209-installer.jar"
        );
    }
}
