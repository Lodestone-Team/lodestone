use color_eyre::eyre::Context;
use serde::Deserialize;

use crate::error::Error;

#[derive(Deserialize)]
struct GameVersion {
    version: String,
}

#[derive(Deserialize)]
struct Versions {
    game: Vec<GameVersion>,
}

fn game_versions(response: Versions) -> Vec<String> {
    response.game.into_iter().map(|g| g.version).collect()
}

/// All Minecraft versions Fabric supports, newest first.
pub async fn get_fabric_minecraft_versions() -> Result<Vec<String>, Error> {
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
    Ok(game_versions(response))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_game_versions_in_order() {
        let response: Versions =
            serde_json::from_str(include_str!("testdata/fabric_versions.json")).unwrap();
        let versions = game_versions(response);
        assert_eq!(versions[0], "26.4-snapshot-7");
        assert!(versions.contains(&"1.17.1".to_string()));
        assert!(versions.contains(&"21w19a".to_string()));
    }
}
