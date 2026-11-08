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

/// PaperMC asks API clients to identify themselves; the shared client sends a Lodestone
/// user agent and has a timeout.
fn client() -> Result<&'static reqwest::Client, Error> {
    Ok(super::util::http_client())
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
    Ok(version_ids(response))
}

fn version_ids(response: VersionsResponse) -> Vec<String> {
    response
        .versions
        .into_iter()
        .map(|entry| entry.version.id)
        .collect()
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
    select_download(builds, build)
}

/// The given build, or the newest stable one. `Ok(None)` when there is no such build.
fn select_download(builds: Vec<Build>, build: Option<i64>) -> Result<Option<PaperDownload>, Error> {
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

    #[test]
    fn versions_are_listed_newest_first() {
        let response: VersionsResponse =
            serde_json::from_str(include_str!("testdata/paper_versions.json")).unwrap();
        let versions = version_ids(response);
        for v in ["1.16.5", "1.16.4", "1.16.3", "1.16.2", "1.16.1"] {
            assert!(versions.contains(&v.to_string()), "missing {v}");
        }
        assert_eq!(versions[0], "26.3");
        let pos = |v: &str| versions.iter().position(|x| x == v).unwrap();
        assert!(pos("1.20.1") < pos("1.16.5"));
    }

    #[test]
    fn selects_the_requested_or_newest_stable_build() {
        let builds = || -> Vec<Build> {
            serde_json::from_str(include_str!("testdata/paper_builds_1.19.json")).unwrap()
        };
        let newest = select_download(builds(), None).unwrap().unwrap();
        assert_eq!(newest.build, 81);
        assert_eq!(
            newest.url,
            "https://fill-data.papermc.io/v1/objects/0d39cacc51a77b2b071e1ce862fcbf0b4a4bd668cc7e8b313598d84fa09fabac/paper-1.19-81.jar"
        );
        // an explicitly requested build may be on any channel
        let alpha = select_download(builds(), Some(5)).unwrap().unwrap();
        assert_eq!(alpha.build, 5);
        assert!(alpha.url.ends_with("/paper-1.19-5.jar"), "{}", alpha.url);
        assert_eq!(select_download(builds(), Some(999_999)).unwrap(), None);

        // a newer build on another channel is not "the newest stable"
        let mixed: Vec<Build> = serde_json::from_str(
            r#"[
                { "id": 12, "channel": "ALPHA", "downloads": { "server:default": { "url": "https://x/12.jar" } } },
                { "id": 11, "channel": "stable", "downloads": { "server:default": { "url": "https://x/11.jar" } } },
                { "id": 10, "channel": "STABLE", "downloads": { "server:default": { "url": "https://x/10.jar" } } }
            ]"#,
        )
        .unwrap();
        assert_eq!(
            select_download(mixed, None).unwrap(),
            Some(PaperDownload {
                build: 11,
                url: "https://x/11.jar".into()
            })
        );
        let unstable: Vec<Build> = serde_json::from_str(
            r#"[{ "id": 1, "channel": "BETA", "downloads": { "server:default": { "url": "https://x/1.jar" } } }]"#,
        )
        .unwrap();
        assert_eq!(select_download(unstable, None).unwrap(), None);
        // a build without a server jar is an error, not a missing build
        let no_jar: Vec<Build> =
            serde_json::from_str(r#"[{ "id": 1, "channel": "STABLE", "downloads": {} }]"#).unwrap();
        assert!(select_download(no_jar, None).is_err());
    }
}
