//! Forge's maven metadata, which maps Minecraft versions to Forge builds.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde_json::Value;

use crate::Error;

/// The Minecraft versions in Forge's maven metadata, newest first. The metadata lists them
/// oldest first, so parse it into an [`IndexMap`] to keep that order.
pub fn minecraft_versions(metadata: IndexMap<String, Value>) -> Vec<String> {
    metadata.into_iter().map(|(k, _)| k).rev().collect()
}

/// The newest Forge build for `version` in Forge's maven metadata, which lists builds
/// oldest first.
pub fn newest_forge_build(
    metadata: &BTreeMap<String, Vec<String>>,
    version: &str,
) -> Result<String, Error> {
    metadata
        .get(version)
        .and_then(|builds| builds.last())
        .cloned()
        .ok_or_else(|| Error::NotFound(format!("Forge has no builds for Minecraft {version}")))
}

/// The Forge build in Forge's maven metadata that a modpack's Forge version names: a pack
/// says `47.2.0` for the build `1.20.1-47.2.0`, and `10.13.4.1614` for
/// `1.7.10-10.13.4.1614-1.7.10`. A full build id is found as it is.
pub fn forge_build_for(
    metadata: &BTreeMap<String, Vec<String>>,
    game_version: &str,
    forge_version: &str,
) -> Result<String, Error> {
    let builds = metadata.get(game_version).map(Vec::as_slice).unwrap_or(&[]);
    let candidates = [
        forge_version.to_string(),
        format!("{game_version}-{forge_version}"),
        format!("{game_version}-{forge_version}-{game_version}"),
    ];
    candidates
        .iter()
        .find(|c| builds.contains(c))
        .cloned()
        .ok_or_else(|| {
            Error::NotFound(format!(
                "Forge has no build {forge_version} for Minecraft {game_version}"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::versions::fixture;

    #[test]
    fn finds_the_build_a_pack_names() {
        let metadata: BTreeMap<String, Vec<String>> = fixture(
            r#"{ "1.20.1": ["1.20.1-47.1.0", "1.20.1-47.2.0"],
                 "1.7.10": ["1.7.10-10.13.4.1614-1.7.10"] }"#,
        );
        assert_eq!(
            forge_build_for(&metadata, "1.20.1", "47.2.0").unwrap(),
            "1.20.1-47.2.0"
        );
        assert_eq!(
            forge_build_for(&metadata, "1.20.1", "1.20.1-47.1.0").unwrap(),
            "1.20.1-47.1.0"
        );
        assert_eq!(
            forge_build_for(&metadata, "1.7.10", "10.13.4.1614").unwrap(),
            "1.7.10-10.13.4.1614-1.7.10"
        );
        assert!(forge_build_for(&metadata, "1.20.1", "47.9.9").is_err());
        assert!(forge_build_for(&metadata, "1.19.2", "47.2.0").is_err());
    }

    const METADATA: &str = include_str!("../../testdata/forge_maven_metadata.json");

    #[test]
    fn newest_forge_build_is_the_last_one_listed() {
        let metadata: BTreeMap<String, Vec<String>> = fixture(METADATA);
        assert_eq!(
            newest_forge_build(&metadata, "1.7.10").unwrap(),
            "1.7.10-10.13.4.1614-1.7.10"
        );
        assert_eq!(
            newest_forge_build(&metadata, "1.7.10_pre4").unwrap(),
            "1.7.10_pre4-10.12.2.1149-prerelease"
        );
        let err = newest_forge_build(&metadata, "1.19.3bruh").unwrap_err();
        assert!(matches!(err, Error::NotFound(_)));
        let empty: BTreeMap<String, Vec<String>> = fixture(r#"{ "1.20.1": [] }"#);
        assert!(newest_forge_build(&empty, "1.20.1").is_err());
    }

    #[test]
    fn lists_versions_newest_first() {
        let metadata: IndexMap<String, Value> = fixture(METADATA);
        let versions = minecraft_versions(metadata);
        for v in ["1.16.5", "1.16.4", "1.16.3", "1.16.2", "1.16.1"] {
            assert!(versions.contains(&v.to_string()), "missing {v}");
        }
        assert_eq!(versions.first().unwrap(), "1.18.2");
        assert_eq!(versions.last().unwrap(), "1.1");
        let pos = |v: &str| versions.iter().position(|x| x == v).unwrap();
        assert!(pos("1.16.5") < pos("1.16.1"));
        assert!(pos("1.7.10") < pos("1.7.10_pre4"));
    }
}
