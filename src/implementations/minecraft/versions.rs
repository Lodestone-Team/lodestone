use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::error::Error;

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export)]
pub struct MinecraftVersions {
    old_alpha: Vec<String>,
    snapshot: Vec<String>,
    release: Vec<String>,
}

pub async fn get_vanilla_versions() -> Result<MinecraftVersions, Error> {
    let http = reqwest::Client::new();
    let response: Value = serde_json::from_str(
        http.get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
            .send()
            .await
            .context("Failed to get vanilla versions")?
            .text()
            .await
            .context("Failed to get vanilla versions")?
            .as_str(),
    )
    .context("Failed to get vanilla versions")?;

    let versions = response["versions"]
        .as_array()
        .ok_or_else(|| eyre!("Failed to get vanilla versions. Mojang API changed?"))?;

    #[derive(Serialize, Deserialize, Debug)]
    struct Version {
        id: String,
        r#type: String,
    }

    let mut ret = MinecraftVersions {
        old_alpha: Vec::new(),
        snapshot: Vec::new(),
        release: Vec::new(),
    };

    for version in versions.iter() {
        let version: Version = serde_json::from_value(version.to_owned())
            .context("Failed to get vanilla versions. Mojang API changed?")?;
        match version.r#type.as_str() {
            "old_alpha" => ret.old_alpha.push(version.id),
            "snapshot" => ret.snapshot.push(version.id),
            "release" => ret.release.push(version.id),
            _ => {}
        }
    }
    Ok(ret)
}

pub async fn get_fabric_versions() -> Result<MinecraftVersions, Error> {
    let http = reqwest::Client::new();

    let response: Value = serde_json::from_str(
        http.get("https://meta.fabricmc.net/v2/versions")
            .send()
            .await
            .context("Failed to get fabric versions")?
            .text()
            .await
            .context("Failed to get fabric versions")?
            .as_str(),
    )
    .context("Failed to get fabric versions")?;

    let vanilla_versions = get_vanilla_versions().await?;
    let mut ret = MinecraftVersions {
        release: Vec::new(),
        snapshot: Vec::new(),
        old_alpha: Vec::new(),
    };
    for item in response["game"]
        .as_array()
        .ok_or_else(|| eyre!("Failed to get fabric versions. Game array is not an array"))?
    {
        let version_str = item["version"].as_str().ok_or_else(|| {
            eyre!("Failed to get fabric versions. Version string is not a string")
        })?;
        if vanilla_versions.release.contains(&version_str.to_string()) {
            ret.release.push(version_str.to_string());
        }
        if vanilla_versions.snapshot.contains(&version_str.to_string()) {
            ret.snapshot.push(version_str.to_string());
        }
        if vanilla_versions
            .old_alpha
            .contains(&version_str.to_string())
        {
            ret.old_alpha.push(version_str.to_string());
        }
    }

    Ok(ret)
}
