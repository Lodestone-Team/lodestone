use color_eyre::eyre::Context;
use serde::Deserialize;

use crate::error::Error;

/// All Minecraft versions Fabric supports, newest first.
pub async fn get_fabric_minecraft_versions() -> Result<Vec<String>, Error> {
    #[derive(Deserialize)]
    struct GameVersion {
        version: String,
    }
    #[derive(Deserialize)]
    struct Versions {
        game: Vec<GameVersion>,
    }
    let response: Versions = super::util::http_client()
        .get("https://meta.fabricmc.net/v2/versions")
        .send()
        .await
        .context("Failed to get fabric versions")?
        .error_for_status()
        .context("Failed to get fabric versions")?
        .json()
        .await
        .context("Failed to get fabric versions, unexpected response")?;
    Ok(response.game.into_iter().map(|g| g.version).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_fabric_minecraft_versions() {
        let versions = get_fabric_minecraft_versions().await.unwrap();
        assert!(!versions.is_empty());
        assert!(versions.contains(&"1.17.1".to_string()));
        assert!(versions.contains(&"21w19a".to_string()));
    }
}
