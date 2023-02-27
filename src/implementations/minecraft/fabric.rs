use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::error::Error;

#[derive(Debug, Clone, TS, Serialize, Deserialize, PartialEq)]
#[ts(export)]
#[serde(transparent)]
pub struct FabricLoaderVersion(String);

impl AsRef<str> for FabricLoaderVersion {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<FabricLoaderVersion> for String {
    fn from(version: FabricLoaderVersion) -> Self {
        version.0
    }
}

#[derive(Debug, Clone, TS, Serialize, Deserialize, PartialEq)]
#[ts(export)]
#[serde(transparent)]
pub struct FabricInstallerVersion(String);

impl AsRef<str> for FabricInstallerVersion {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<FabricInstallerVersion> for String {
    fn from(version: FabricInstallerVersion) -> Self {
        version.0
    }
}

pub async fn get_fabric_minecraft_versions() -> Result<Vec<String>, Error> {
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

    response["game"]
        .as_array()
        .ok_or_else(|| eyre!("Failed to get fabric versions. Game array is not an array"))?
        .iter()
        .map(|item| {
            item["version"]
                .as_str()
                .ok_or_else(|| {
                    eyre!("Failed to get fabric versions. Version string is not a string").into()
                })
                .map(|version| version.to_string())
        })
        .collect::<Result<Vec<String>, Error>>() // Rust converts Vec<Result<&str, Error>> to Result<Vec<&str>, Error>
}

pub async fn get_fabric_installer_versions() -> Result<Vec<String>, Error> {
    let http = reqwest::Client::new();

    let response: Value = serde_json::from_str(
        http.get("https://meta.fabricmc.net/v2/versions/installer")
            .send()
            .await
            .context("Failed to get fabric installer versions")?
            .text()
            .await
            .context("Failed to get fabric installer versions")?
            .as_str(),
    )
    .context("Failed to get fabric installer versions")?;

    let versions = response
        .as_array()
        .ok_or_else(|| eyre!("Failed to get fabric installer versions. Response is not an array"))?
        .iter()
        .map(|item| {
            item["version"].as_str().ok_or_else(|| {
                eyre!("Failed to get fabric installer versions. Version string is not a string")
                    .into()
            })
        })
        .collect::<Result<Vec<&str>, Error>>()?; // Rust converts Vec<Result<&str, Error>> to Result<Vec<&str>, Error>

    Ok(versions.iter().map(|version| version.to_string()).collect())
}

pub async fn get_fabric_loader_versions() -> Result<Vec<String>, Error> {
    let http = reqwest::Client::new();

    let response: Value = serde_json::from_str(
        http.get("https://meta.fabricmc.net/v2/versions/loader")
            .send()
            .await
            .context("Failed to get fabric loader versions")?
            .text()
            .await
            .context("Failed to get fabric loader versions")?
            .as_str(),
    )
    .context("Failed to get fabric loader versions")?;

    let versions = response
        .as_array()
        .ok_or_else(|| eyre!("Failed to get fabric loader versions. Response is not an array"))?
        .iter()
        .map(|item| {
            item["version"].as_str().ok_or_else(|| {
                eyre!("Failed to get fabric loader versions. Version string is not a string").into()
            })
        })
        .collect::<Result<Vec<&str>, Error>>()?; // Rust converts Vec<Result<&str, Error>> to Result<Vec<&str>, Error>

    Ok(versions.iter().map(|version| version.to_string()).collect())
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

    #[tokio::test]
    async fn test_get_fabric_installer_versions() {
        let versions = get_fabric_installer_versions().await.unwrap();
        assert!(!versions.is_empty());
        assert!(versions.contains(&"0.7.4".to_string()));
    }

    #[tokio::test]
    async fn test_get_fabric_loader_versions() {
        let versions = get_fabric_loader_versions().await.unwrap();
        assert!(!versions.is_empty());
        assert!(versions.contains(&"0.11.6".to_string()));
    }
}
