use color_eyre::eyre::{eyre, Context, ContextCompat};
use serde_json::Value;

use crate::error::Error;

pub async fn get_vanilla_minecraft_versions() -> Result<Vec<String>, Error> {
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

    let mut versions = Vec::new();

    for version in response
        .get("versions")
        .context("Failed to get vanilla versions, response does not contain versions")?
        .as_array()
        .context("Failed to get vanilla versions")?
    {
        let version = version
            .as_object()
            .context("Failed to get vanilla versions")?
            .get("id")
            .context("Failed to get vanilla versions")?
            .as_str()
            .ok_or_else(|| -> Error {
                eyre!("Failed to get vanilla versions. Version string is not a string").into()
            })
            .map(|version| version.to_string())?;

        versions.push(version);
    }

    Ok(versions)
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_get_vanilla_minecraft_versions() {
        let versions = get_vanilla_minecraft_versions().await.unwrap();
        dbg!(&versions);
        assert!(versions.contains(&"1.16.5".to_string()));
        assert!(versions.contains(&"1.16.4".to_string()));
        assert!(versions.contains(&"1.16.3".to_string()));
        assert!(versions.contains(&"1.16.2".to_string()));
        assert!(versions.contains(&"1.16.1".to_string()));
        assert!(versions.contains(&"1.16".to_string()));
        assert!(versions.contains(&"1.15.2".to_string()));
        assert!(versions.contains(&"1.15.1".to_string()));
        assert!(versions.contains(&"1.15".to_string()));
        assert!(versions.contains(&"1.14.4".to_string()));
        assert!(versions.contains(&"1.14.3".to_string()));
        assert!(versions.contains(&"1.14.2".to_string()));
        assert!(versions.contains(&"1.14.1".to_string()));
        assert!(versions.contains(&"1.14".to_string()));
        assert!(versions.contains(&"1.13.2".to_string()));
        assert!(versions.contains(&"1.13.1".to_string()));
        assert!(versions.contains(&"1.13".to_string()));
        assert!(versions.contains(&"1.12.2".to_string()));
        assert!(versions.contains(&"1.12.1".to_string()));
        assert!(versions.contains(&"1.12".to_string()));
        assert!(versions.contains(&"1.11.2".to_string()));
        assert!(versions.contains(&"1.11.1".to_string()));
        assert!(versions.contains(&"1.11".to_string()));
        assert!(versions.contains(&"1.10.2".to_string()));
        assert!(versions.contains(&"1.10.1".to_string()));
        assert!(versions.contains(&"1.10".to_string()));
        assert!(versions.contains(&"1.9.4".to_string()));
        assert!(versions.contains(&"1.9.3".to_string()));
        assert!(versions.contains(&"1.9.2".to_string()));
    }
}
