//! Fabric meta (<https://meta.fabricmc.net>): game versions, loaders and installers.

use serde::Deserialize;

use super::compare_versions;
use crate::Error;

#[derive(Debug, Clone, Deserialize)]
pub struct FabricGameVersion {
    pub version: String,
}

/// The answer of `GET /versions`.
#[derive(Debug, Clone, Deserialize)]
pub struct FabricVersions {
    pub game: Vec<FabricGameVersion>,
}

/// A loader, intermediary or installer version.
#[derive(Debug, Clone, Deserialize)]
pub struct FabricStable {
    pub version: String,
    #[serde(default)]
    pub stable: bool,
}

/// An entry of `GET /versions/loader/{game_version}`.
#[derive(Debug, Clone, Deserialize)]
pub struct FabricLoaderEntry {
    pub loader: FabricStable,
    pub intermediary: Option<FabricStable>,
}

/// The Minecraft versions Fabric supports, in the order Fabric lists them (newest first).
pub fn game_versions(response: FabricVersions) -> Vec<String> {
    response.game.into_iter().map(|g| g.version).collect()
}

/// The newest loader whose loader and intermediary are both stable, or else the newest
/// stable loader.
pub fn select_fabric_loader(entries: &[FabricLoaderEntry], version: &str) -> Result<String, Error> {
    let newest = |entries: Vec<&FabricLoaderEntry>| {
        entries
            .into_iter()
            .max_by(|a, b| compare_versions(&a.loader.version, &b.loader.version))
            .map(|e| e.loader.version.clone())
    };
    let fully_stable = entries
        .iter()
        .filter(|e| e.loader.stable && e.intermediary.as_ref().is_some_and(|i| i.stable))
        .collect();
    let loader_stable = entries.iter().filter(|e| e.loader.stable).collect();
    newest(fully_stable)
        .or_else(|| newest(loader_stable))
        .ok_or_else(|| {
            Error::NotFound(format!(
                "Fabric has no stable loader for Minecraft {version}"
            ))
        })
}

/// The newest stable installer of `GET /versions/installer`.
pub fn select_fabric_installer(entries: Vec<FabricStable>) -> Result<String, Error> {
    entries
        .into_iter()
        .filter(|e| e.stable)
        .max_by(|a, b| compare_versions(&a.version, &b.version))
        .map(|e| e.version)
        .ok_or_else(|| Error::NotFound("Fabric has no stable installer".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::versions::fixture;

    #[test]
    fn newest_stable_fabric_loader_and_installer() {
        let loaders: Vec<FabricLoaderEntry> =
            fixture(include_str!("../../testdata/fabric_loader_1.19.json"));
        // 0.20.0 and the older ones are not stable
        assert_eq!(select_fabric_loader(&loaders, "1.19").unwrap(), "0.20.1");

        // a loader with an unstable intermediary is only used when nothing better exists
        let loaders: Vec<FabricLoaderEntry> = fixture(
            r#"[
                { "loader": { "version": "0.16.0", "stable": true }, "intermediary": { "version": "x", "stable": false } },
                { "loader": { "version": "0.15.9", "stable": true }, "intermediary": { "version": "x", "stable": true } },
                { "loader": { "version": "0.17.0", "stable": false }, "intermediary": { "version": "x", "stable": true } }
            ]"#,
        );
        assert_eq!(select_fabric_loader(&loaders, "x").unwrap(), "0.15.9");
        assert_eq!(select_fabric_loader(&loaders[..1], "x").unwrap(), "0.16.0");
        let err = select_fabric_loader(&loaders[2..], "x").unwrap_err();
        assert!(matches!(err, Error::NotFound(_)));
        assert!(select_fabric_loader(&[], "x").is_err());

        let installers: Vec<FabricStable> =
            fixture(include_str!("../../testdata/fabric_installer.json"));
        assert_eq!(select_fabric_installer(installers).unwrap(), "1.1.2");
        let unstable: Vec<FabricStable> = fixture(r#"[{ "version": "1.0.0", "stable": false }]"#);
        assert!(select_fabric_installer(unstable).is_err());
    }

    #[test]
    fn lists_game_versions_in_order() {
        let response: FabricVersions = fixture(include_str!("../../testdata/fabric_versions.json"));
        let versions = game_versions(response);
        assert_eq!(versions[0], "26.4-snapshot-7");
        assert!(versions.contains(&"1.17.1".to_string()));
        assert!(versions.contains(&"21w19a".to_string()));
    }
}
