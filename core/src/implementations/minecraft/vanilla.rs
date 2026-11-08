use lodestone_minecraft::versions::mojang::version_ids;

use crate::error::Error;

/// All Minecraft versions, newest first.
pub async fn get_vanilla_minecraft_versions() -> Result<Vec<String>, Error> {
    Ok(version_ids(&super::util::mojang_versions().await?))
}
