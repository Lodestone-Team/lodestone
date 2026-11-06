use color_eyre::eyre::Context;
use indexmap::IndexMap;
use serde_json::Value;

use crate::error::Error;

/// All Minecraft versions Forge has builds for, newest first.
pub async fn get_forge_minecraft_versions() -> Result<Vec<String>, Error> {
    // IndexMap keeps the order of the metadata file (oldest first)
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

    Ok(response.into_iter().map(|(k, _)| k).rev().collect())
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_get_forge_minecraft_versions() {
        let versions = get_forge_minecraft_versions().await.unwrap();
        for v in ["1.16.5", "1.16.4", "1.16.3", "1.16.2", "1.16.1"] {
            assert!(versions.contains(&v.to_string()), "missing {v}");
        }
    }
}
