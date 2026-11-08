use color_eyre::eyre::Context;
use lodestone_minecraft::versions::fabric::{game_versions, FabricVersions};

use crate::error::Error;

/// All Minecraft versions Fabric supports, newest first.
pub async fn get_fabric_minecraft_versions() -> Result<Vec<String>, Error> {
    let response: FabricVersions = super::util::http_client()
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
