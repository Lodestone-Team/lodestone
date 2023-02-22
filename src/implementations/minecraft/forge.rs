use std::collections::BTreeMap;

use color_eyre::eyre::Context;
use serde_json::Value;

use crate::error::Error;

pub async fn get_forge_minecraft_versions() -> Result<Vec<String>, Error> {
    let http = reqwest::Client::new();

    let response: BTreeMap<String, Value> = serde_json::from_str(
        http.get("https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json")
            .send()
            .await
            .context("Failed to get forge versions, http request failed")?
            .text()
            .await
            .context("Failed to get forge versions, text conversion failed")?
            .as_str(),
    )
    .context("Failed to get forge versions, json is not a map")?;

    Ok(response.keys().map(|version| version.to_string()).collect())
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_get_forge_minecraft_versions() {
        let versions = get_forge_minecraft_versions().await.unwrap();
        assert!(versions.contains(&"1.16.5".to_string()));
        assert!(versions.contains(&"1.16.4".to_string()));
        assert!(versions.contains(&"1.16.3".to_string()));
        assert!(versions.contains(&"1.16.2".to_string()));
        assert!(versions.contains(&"1.16.1".to_string()));
    }
}
