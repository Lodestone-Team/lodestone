use color_eyre::eyre::Context;
use indexmap::IndexMap;
use lodestone_minecraft::versions::forge::minecraft_versions;
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
