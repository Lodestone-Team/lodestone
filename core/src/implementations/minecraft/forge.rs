use color_eyre::eyre::Context;
use indexmap::IndexMap;
use serde_json::Value;

use crate::error::Error;

/// All Minecraft versions Forge has builds for, newest first.
pub async fn get_forge_minecraft_versions() -> Result<Vec<String>, Error> {
    // IndexMap keeps the order of the metadata file
    let response: IndexMap<String, Value> = super::util::http_client()
        .get("https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json")
        .send()
        .await
        .context("Failed to get forge versions, http request failed")?
        .error_for_status()
        .context("Failed to get forge versions")?
        .json()
        .await
        .context("Failed to get forge versions, json is not a map")?;

    Ok(minecraft_versions(response))
}

/// The Minecraft versions in Forge's maven metadata, which lists them oldest first.
fn minecraft_versions(metadata: IndexMap<String, Value>) -> Vec<String> {
    metadata.into_iter().map(|(k, _)| k).rev().collect()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn lists_versions_newest_first() {
        let metadata: IndexMap<String, Value> =
            serde_json::from_str(include_str!("testdata/forge_maven_metadata.json")).unwrap();
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
