//! PaperMC download API ("Fill" v3).
//!
//! PaperMC sunset the v2 API at api.papermc.io (it now answers 410 Gone). Fill v3 lists
//! versions newest first, returns builds with numeric ids and a `channel`
//! (STABLE/BETA/ALPHA/RECOMMENDED), and gives each build a direct download URL with a
//! SHA-256 checksum.

use std::collections::HashMap;

use color_eyre::eyre::{eyre, Context};
use serde::Deserialize;

use crate::error::Error;

const PAPER_API: &str = "https://fill.papermc.io/v3/projects/paper";

/// PaperMC asks API clients to identify themselves.
const USER_AGENT: &str = concat!(
    "Lodestone/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Lodestone-Team/lodestone)"
);

#[derive(Deserialize)]
struct VersionsResponse {
    versions: Vec<VersionEntry>,
}

#[derive(Deserialize)]
struct VersionEntry {
    version: VersionInfo,
}

#[derive(Deserialize)]
struct VersionInfo {
    id: String,
}

#[derive(Deserialize)]
struct Build {
    id: i64,
    channel: String,
    downloads: HashMap<String, Download>,
}

#[derive(Deserialize)]
struct Download {
    url: String,
}

/// A downloadable Paper server jar.
#[derive(Debug, Clone, PartialEq)]
pub struct PaperDownload {
    pub build: i64,
    pub url: String,
}

fn client() -> Result<reqwest::Client, Error> {
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("Failed to create HTTP client")?)
}

/// All Minecraft versions Paper has builds for, newest first.
pub async fn get_paper_minecraft_versions() -> Result<Vec<String>, Error> {
    let response: VersionsResponse = client()?
        .get(format!("{PAPER_API}/versions"))
        .send()
        .await
        .context("Failed to get paper versions")?
        .error_for_status()
        .context("Failed to get paper versions")?
        .json()
        .await
        .context("Failed to get paper versions, unexpected response")?;

    Ok(response
        .versions
        .into_iter()
        .map(|entry| entry.version.id)
        .collect())
}

/// The server jar for `version`: the given build, or the newest stable build.
/// Returns `Ok(None)` when the version or build does not exist.
pub async fn get_paper_download(
    version: &str,
    build: Option<i64>,
) -> Result<Option<PaperDownload>, Error> {
    let response = client()?
        .get(format!("{PAPER_API}/versions/{version}/builds"))
        .send()
        .await
        .context("Failed to get paper builds")?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let builds: Vec<Build> = response
        .error_for_status()
        .context("Failed to get paper builds")?
        .json()
        .await
        .context("Failed to get paper builds, unexpected response")?;

    let selected = match build {
        Some(id) => builds.into_iter().find(|b| b.id == id),
        None => builds
            .into_iter()
            .filter(|b| b.channel.eq_ignore_ascii_case("STABLE"))
            .max_by_key(|b| b.id),
    };
    let Some(mut selected) = selected else {
        return Ok(None);
    };
    let download = selected
        .downloads
        .remove("server:default")
        .ok_or_else(|| eyre!("Paper build {} has no server download", selected.id))?;

    Ok(Some(PaperDownload {
        build: selected.id,
        url: download.url,
    }))
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_get_paper_minecraft_versions() {
        let versions = get_paper_minecraft_versions().await.unwrap();
        for v in ["1.16.5", "1.16.4", "1.16.3", "1.16.2", "1.16.1"] {
            assert!(versions.contains(&v.to_string()), "missing {v}");
        }
        // newest first, like the v2-based code returned after reversing
        let pos = |v: &str| versions.iter().position(|x| x == v).unwrap();
        assert!(pos("1.20.1") < pos("1.16.5"));
    }
}
