use color_eyre::eyre::Context;
use indexmap::IndexMap;
use lodestone_minecraft::versions::forge::minecraft_versions;
use lodestone_minecraft::versions::neoforge::{
    neoforge_minecraft_versions, NeoForgeVersions, NEOFORGE_VERSIONS_URL,
};
use serde_json::Value;

use crate::error::Error;

/// All Minecraft versions Forge has builds for, newest first.
pub async fn get_forge_minecraft_versions() -> Result<Vec<String>, Error> {
    // IndexMap keeps the order of the metadata file
    let url = "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
    let response: IndexMap<String, Value> = super::util::api_get(url)
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

/// NeoForge's version list, which names each version after its Minecraft version.
pub async fn get_neoforge_versions() -> Result<NeoForgeVersions, Error> {
    Ok(super::util::api_get(NEOFORGE_VERSIONS_URL)
        .send()
        .await
        .context("Failed to get NeoForge versions, http request failed")?
        .error_for_status()
        .context("Failed to get NeoForge versions")?
        .json()
        .await
        .context("Failed to get NeoForge versions, unexpected response")?)
}

/// All Minecraft versions NeoForge has builds for, newest first.
pub async fn get_neoforge_minecraft_versions() -> Result<Vec<String>, Error> {
    Ok(neoforge_minecraft_versions(&get_neoforge_versions().await?))
}
