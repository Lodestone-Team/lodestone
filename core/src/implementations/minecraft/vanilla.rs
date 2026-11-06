use crate::error::Error;

/// All Minecraft versions, newest first.
pub async fn get_vanilla_minecraft_versions() -> Result<Vec<String>, Error> {
    Ok(super::util::mojang_versions()
        .await?
        .iter()
        .map(|v| v.id.clone())
        .collect())
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_get_vanilla_minecraft_versions() {
        let versions = get_vanilla_minecraft_versions().await.unwrap();
        for v in [
            "1.16.5", "1.16.4", "1.16.3", "1.16.2", "1.16.1", "1.16", "1.15.2", "1.15.1", "1.15",
            "1.14.4", "1.14.3", "1.14.2", "1.14.1", "1.14", "1.13.2", "1.13.1", "1.13", "1.12.2",
            "1.12.1", "1.12", "1.11.2", "1.11.1", "1.11", "1.10.2", "1.10.1", "1.10", "1.9.4",
            "1.9.3", "1.9.2",
        ] {
            assert!(versions.contains(&v.to_string()), "missing {v}");
        }
    }
}
