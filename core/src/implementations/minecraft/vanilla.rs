use super::util::MojangVersion;
use crate::error::Error;

/// All Minecraft versions, newest first.
pub async fn get_vanilla_minecraft_versions() -> Result<Vec<String>, Error> {
    Ok(version_ids(&super::util::mojang_versions().await?))
}

fn version_ids(versions: &[MojangVersion]) -> Vec<String> {
    versions.iter().map(|v| v.id.clone()).collect()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn lists_manifest_versions_in_order() {
        #[derive(serde::Deserialize)]
        struct Manifest {
            versions: Vec<MojangVersion>,
        }
        let manifest: Manifest =
            serde_json::from_str(include_str!("testdata/mojang_version_manifest.json")).unwrap();
        assert_eq!(
            version_ids(&manifest.versions),
            [
                "26.4-snapshot-7",
                "1.18.2",
                "21w44a",
                "1.16.5",
                "1.8.4",
                "1.7.10-pre4",
                "a1.0.16"
            ]
        );
    }
}
