use color_eyre::eyre::{eyre, Context, ContextCompat};
use serde_json::Value;

use crate::error::Error;

pub async fn get_paper_minecraft_versions() -> Result<Vec<String>, Error> {
    let http = reqwest::Client::new();

    let response: Value = serde_json::from_str(
        http.get("https://api.papermc.io/v2/projects/paper")
            .send()
            .await
            .context("Failed to get paper versions")?
            .text()
            .await
            .context("Failed to get paper versions")?
            .as_str(),
    )
    .context("Failed to get paper versions, response is not valid json")?;

    let mut versions = response
        .get("versions")
        .context("Failed to get paper versions, response does not contain versions")?
        .as_array()
        .context("Failed to get paper versions Response is not an array")?
        .iter()
        .map(|version| {
            version
                .as_str()
                .ok_or_else(|| {
                    eyre!("Failed to get paper versions. Version string is not a string").into()
                })
                .map(|version| version.to_string())
        })
        .collect::<Result<Vec<String>, Error>>()?;

    versions.reverse();

    Ok(versions)
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_get_paper_minecraft_versions() {
        let versions = get_paper_minecraft_versions().await.unwrap();
        assert!(versions.contains(&"1.16.5".to_string()));
        assert!(versions.contains(&"1.16.4".to_string()));
        assert!(versions.contains(&"1.16.3".to_string()));
        assert!(versions.contains(&"1.16.2".to_string()));
        assert!(versions.contains(&"1.16.1".to_string()));
    }
}
