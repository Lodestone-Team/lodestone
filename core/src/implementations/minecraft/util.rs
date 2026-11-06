//! Version and download lookups for Minecraft servers (Mojang, Fabric, Forge, Paper, Adoptium).

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use color_eyre::eyre::{eyre, Context};
use serde::Deserialize;

use super::{
    FabricInstallerVersion, FabricLoaderVersion, Flavour, ForgeBuildVersion, PaperBuildVersion,
};
use crate::error::{Error, ErrorKind};

/// Some APIs (PaperMC) ask clients to identify themselves.
pub(super) const USER_AGENT: &str = concat!(
    "Lodestone/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Lodestone-Team/lodestone)"
);

const MOJANG_VERSION_MANIFEST: &str =
    "https://launchermeta.mojang.com/mc/game/version_manifest.json";

/// How long version lists are cached.
pub(super) const VERSION_CACHE_TTL: Duration = Duration::from_secs(10 * 60);

/// One HTTP client for all Minecraft API calls, so connections are reused and every request
/// has a timeout.
pub(super) fn http_client() -> &'static reqwest::Client {
    static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
        reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    });
    &CLIENT
}

fn not_found(message: String) -> Error {
    Error {
        kind: ErrorKind::NotFound,
        source: eyre!(message),
    }
}

async fn get_json<T: serde::de::DeserializeOwned>(url: &str, what: &str) -> Result<T, Error> {
    Ok(http_client()
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to get {what}: the request to {url} failed"))?
        .error_for_status()
        .with_context(|| format!("Failed to get {what}"))?
        .json()
        .await
        .with_context(|| format!("Failed to get {what}: unexpected response from {url}"))?)
}

/// Compares version strings such as "0.14.21" or "1.0.0-beta.2" part by part, numerically
/// where both parts are numbers. If one is a prefix of the other, the longer one is newer.
pub(super) fn compare_versions(a: &str, b: &str) -> Ordering {
    let split = |s: &str| -> Vec<String> {
        s.split(['.', '-', '+', '_'])
            .map(|p| p.to_string())
            .collect()
    };
    let (a_parts, b_parts) = (split(a), split(b));
    for (a_part, b_part) in a_parts.iter().zip(b_parts.iter()) {
        let ordering = match (a_part.parse::<u64>(), b_part.parse::<u64>()) {
            (Ok(a_num), Ok(b_num)) => a_num.cmp(&b_num),
            _ => a_part.cmp(b_part),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    a_parts.len().cmp(&b_parts.len())
}

// ---- Mojang ----

#[derive(Debug, Clone, Deserialize)]
pub(super) struct MojangVersion {
    pub id: String,
    pub url: String,
}

#[derive(Deserialize)]
struct MojangVersionManifest {
    versions: Vec<MojangVersion>,
}

#[derive(Deserialize)]
struct MojangVersionInfo {
    downloads: Option<MojangDownloads>,
    #[serde(rename = "javaVersion")]
    java_version: Option<MojangJavaVersion>,
}

#[derive(Deserialize)]
struct MojangDownloads {
    server: Option<MojangDownload>,
}

#[derive(Deserialize)]
struct MojangDownload {
    url: String,
}

#[derive(Deserialize)]
struct MojangJavaVersion {
    #[serde(rename = "majorVersion")]
    major_version: u64,
}

/// All versions in Mojang's version manifest, newest first. Cached for a few minutes.
pub(super) async fn mojang_versions() -> Result<Arc<Vec<MojangVersion>>, Error> {
    static CACHE: LazyLock<tokio::sync::Mutex<Option<(Instant, Arc<Vec<MojangVersion>>)>>> =
        LazyLock::new(|| tokio::sync::Mutex::new(None));
    let mut cache = CACHE.lock().await;
    if let Some((fetched_at, versions)) = cache.as_ref() {
        if fetched_at.elapsed() < VERSION_CACHE_TTL {
            return Ok(versions.clone());
        }
    }
    let manifest: MojangVersionManifest =
        get_json(MOJANG_VERSION_MANIFEST, "the Minecraft version list").await?;
    let versions = Arc::new(manifest.versions);
    *cache = Some((Instant::now(), versions.clone()));
    Ok(versions)
}

/// Mojang uses '-' where Forge version names use '_' (for example 1.7.10_pre4).
fn mojang_version_id(version: &str) -> String {
    version.replace('_', "-")
}

async fn mojang_version_info(version: &str) -> Result<MojangVersionInfo, Error> {
    let id = mojang_version_id(version);
    let versions = mojang_versions().await?;
    let entry = versions
        .iter()
        .find(|v| v.id == id)
        .ok_or_else(|| not_found(format!("Minecraft version {version} does not exist")))?;
    get_json(&entry.url, &format!("the details of Minecraft {version}")).await
}

// ---- server jars ----

/// Returns the jar url and the updated flavour with version information
pub async fn get_server_jar_url(
    version: &str,
    flavour: &Flavour,
) -> Result<(String, Flavour), Error> {
    match flavour {
        Flavour::Vanilla => get_vanilla_jar_url(version).await,
        Flavour::Fabric {
            loader_version,
            installer_version,
        } => get_fabric_jar_url(version, loader_version, installer_version).await,
        Flavour::Paper { build_version } => get_paper_jar_url(version, build_version).await,
        Flavour::Spigot => Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("Spigot servers are not supported"),
        }),
        Flavour::Forge { build_version } => get_forge_jar_url(version, build_version).await,
    }
}

pub async fn get_vanilla_jar_url(version: &str) -> Result<(String, Flavour), Error> {
    let info = mojang_version_info(version).await?;
    let url = info
        .downloads
        .and_then(|d| d.server)
        .map(|s| s.url)
        .ok_or_else(|| not_found(format!("Minecraft {version} has no server download")))?;
    Ok((url, Flavour::Vanilla))
}

#[derive(Deserialize)]
struct FabricStable {
    version: String,
    #[serde(default)]
    stable: bool,
}

#[derive(Deserialize)]
struct FabricLoaderEntry {
    loader: FabricStable,
    intermediary: Option<FabricStable>,
}

async fn newest_fabric_loader(version: &str) -> Result<String, Error> {
    let entries: Vec<FabricLoaderEntry> = get_json(
        &format!("https://meta.fabricmc.net/v2/versions/loader/{version}"),
        "Fabric loader versions",
    )
    .await?;
    let newest = |entries: Vec<&FabricLoaderEntry>| {
        entries
            .into_iter()
            .max_by(|a, b| compare_versions(&a.loader.version, &b.loader.version))
            .map(|e| e.loader.version.clone())
    };
    let fully_stable = entries
        .iter()
        .filter(|e| e.loader.stable && e.intermediary.as_ref().is_some_and(|i| i.stable))
        .collect();
    let loader_stable = entries.iter().filter(|e| e.loader.stable).collect();
    newest(fully_stable)
        .or_else(|| newest(loader_stable))
        .ok_or_else(|| {
            not_found(format!(
                "Fabric has no stable loader for Minecraft {version}"
            ))
        })
}

async fn newest_fabric_installer() -> Result<String, Error> {
    let entries: Vec<FabricStable> = get_json(
        "https://meta.fabricmc.net/v2/versions/installer",
        "Fabric installer versions",
    )
    .await?;
    entries
        .into_iter()
        .filter(|e| e.stable)
        .max_by(|a, b| compare_versions(&a.version, &b.version))
        .map(|e| e.version)
        .ok_or_else(|| not_found("Fabric has no stable installer".to_string()))
}

pub async fn get_fabric_jar_url(
    version: &str,
    fabric_loader_version: &Option<FabricLoaderVersion>,
    fabric_installer_version: &Option<FabricInstallerVersion>,
) -> Result<(String, Flavour), Error> {
    let loader_version = match fabric_loader_version {
        Some(FabricLoaderVersion(l)) => l.clone(),
        None => newest_fabric_loader(version).await?,
    };
    let installer_version = match fabric_installer_version {
        Some(FabricInstallerVersion(i)) => i.clone(),
        None => newest_fabric_installer().await?,
    };
    Ok((
        format!(
            "https://meta.fabricmc.net/v2/versions/loader/{version}/{loader_version}/{installer_version}/server/jar"
        ),
        Flavour::Fabric {
            loader_version: Some(FabricLoaderVersion(loader_version)),
            installer_version: Some(FabricInstallerVersion(installer_version)),
        },
    ))
}

pub async fn get_paper_jar_url(
    version: &str,
    paper_build_version: &Option<PaperBuildVersion>,
) -> Result<(String, Flavour), Error> {
    let build = paper_build_version.as_ref().map(|PaperBuildVersion(b)| *b);
    let download = super::paper::get_paper_download(version, build)
        .await?
        .ok_or_else(|| match build {
            Some(b) => not_found(format!("Paper has no build {b} for Minecraft {version}")),
            None => not_found(format!("Paper has no stable build for Minecraft {version}")),
        })?;
    Ok((
        download.url,
        Flavour::Paper {
            build_version: Some(PaperBuildVersion(download.build)),
        },
    ))
}

pub async fn get_forge_jar_url(
    version: &str,
    forge_build_version: &Option<ForgeBuildVersion>,
) -> Result<(String, Flavour), Error> {
    let build = match forge_build_version {
        Some(ForgeBuildVersion(b)) => b.clone(),
        None => {
            let response: BTreeMap<String, Vec<String>> = get_json(
                "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json",
                "Forge versions",
            )
            .await?;
            response
                .get(version)
                .ok_or_else(|| not_found(format!("Forge has no builds for Minecraft {version}")))?
                .last()
                .ok_or_else(|| not_found(format!("Forge has no builds for Minecraft {version}")))?
                .clone()
        }
    };

    Ok((
        format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{build}/forge-{build}-installer.jar"
        ),
        Flavour::Forge {
            build_version: Some(ForgeBuildVersion(build)),
        },
    ))
}

// ---- Java ----

/// The Java major version that a Minecraft version needs.
pub async fn get_jre_major_version(version: &str) -> Result<u64, Error> {
    let info = mojang_version_info(version).await?;
    let major = info.java_version.map_or(8, |j| j.major_version);
    // Adoptium doesn't provide Java 16; 17 is compatible and an LTS release
    Ok(if major == 16 { 17 } else { major })
}

/// The Adoptium download URL of the JRE for a Java major version on this machine.
pub fn jre_download_url(major_java_version: u64) -> String {
    let os = if std::env::consts::OS == "macos" {
        "mac"
    } else {
        std::env::consts::OS
    };
    let arch = if std::env::consts::ARCH == "x86_64" {
        "x64"
    } else {
        std::env::consts::ARCH
    };
    format!(
        "https://api.adoptium.net/v3/binary/latest/{major_java_version}/ga/{os}/{arch}/jre/hotspot/normal/eclipse"
    )
}

/// The JRE download URL and Java major version for a Minecraft version.
pub async fn get_jre_url(version: &str) -> Result<(String, u64), Error> {
    let major = get_jre_major_version(version)
        .await
        .with_context(|| format!("Could not find the Java version for Minecraft {version}"))?;
    Ok((jre_download_url(major), major))
}

// ---- players ----

/// Looks up the UUID of an online-mode player name. Successful lookups are cached.
pub async fn name_to_uuid(name: &str) -> Option<String> {
    static CACHE: LazyLock<std::sync::Mutex<HashMap<String, String>>> =
        LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));
    if let Some(uuid) = CACHE.lock().ok()?.get(name) {
        return Some(uuid.clone());
    }

    #[derive(Deserialize)]
    struct Profile {
        id: String,
    }
    let profile: Profile = http_client()
        .get(format!(
            "https://api.mojang.com/users/profiles/minecraft/{name}"
        ))
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;

    if let Ok(mut cache) = CACHE.lock() {
        if cache.len() > 10_000 {
            cache.clear();
        }
        cache.insert(name.to_string(), profile.id.clone());
    }
    Some(profile.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::minecraft::{
        FabricInstallerVersion, FabricLoaderVersion, Flavour, ForgeBuildVersion, PaperBuildVersion,
    };
    use tokio;

    #[test]
    fn test_compare_versions() {
        assert_eq!(compare_versions("0.14.21", "0.14.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.2", "1.2.1"), Ordering::Less);
        assert_eq!(compare_versions("1.2.1", "1.2.1"), Ordering::Equal);
        assert_eq!(compare_versions("1.0.0", "0.11.2"), Ordering::Greater);
        assert_eq!(compare_versions("x.1", "1.1"), Ordering::Greater);
    }

    #[test]
    fn test_mojang_version_id() {
        assert_eq!(mojang_version_id("1.7.10_pre4"), "1.7.10-pre4");
        assert_eq!(mojang_version_id("1.20.1"), "1.20.1");
    }

    #[tokio::test]
    async fn test_get_vanilla_jar_url() {
        assert_eq!(super::get_vanilla_jar_url("1.18.2").await.unwrap(), ("https://piston-data.mojang.com/v1/objects/c8f83c5655308435b3dcf03c06d9fe8740a77469/server.jar".to_string(), Flavour::Vanilla));
        assert_eq!(super::get_vanilla_jar_url("21w44a").await.unwrap(), ("https://piston-data.mojang.com/v1/objects/ae583fd57a8c07f2d6fbadce1ce1e1379bf4b32d/server.jar".to_string(), Flavour::Vanilla));
        assert_eq!(super::get_vanilla_jar_url("1.8.4").await.unwrap(), ("https://launcher.mojang.com/v1/objects/dd4b5eba1c79500390e0b0f45162fa70d38f8a3d/server.jar".to_string(), Flavour::Vanilla));

        let err = super::get_vanilla_jar_url("1.8.4asdasd").await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::NotFound));
    }

    #[tokio::test]
    async fn test_get_jre_url() {
        assert_eq!(super::get_jre_major_version("1.8.4").await.unwrap(), 8);
        assert_eq!(super::get_jre_major_version("1.18.2").await.unwrap(), 17);
        // Forge names pre-releases with '_'
        assert_eq!(
            super::get_jre_major_version("1.7.10_pre4").await.unwrap(),
            8
        );
        assert!(super::get_jre_url("1.8.4asdasd").await.is_err());
    }

    /// Test subject to fail if fabric updates their installer or loader
    #[tokio::test]
    async fn test_get_fabric_jar_url() {
        assert_eq!(
            super::get_fabric_jar_url(
                "1.19",
                &Some(FabricLoaderVersion("0.14.8".to_string())),
                &Some(FabricInstallerVersion("0.11.0".to_string()))
            )
            .await
            .unwrap(),
            (
                "https://meta.fabricmc.net/v2/versions/loader/1.19/0.14.8/0.11.0/server/jar"
                    .to_string(),
                Flavour::Fabric {
                    loader_version: Some(FabricLoaderVersion("0.14.8".to_string())),
                    installer_version: Some(FabricInstallerVersion("0.11.0".to_string()))
                }
            )
        );
        assert!(super::get_fabric_jar_url("21w44a", &None, &None)
            .await
            .is_ok());
        // only one of the two versions given: the other one is looked up
        let (url, _) = super::get_fabric_jar_url(
            "1.19",
            &Some(FabricLoaderVersion("0.14.8".to_string())),
            &None,
        )
        .await
        .unwrap();
        assert!(url.starts_with("https://meta.fabricmc.net/v2/versions/loader/1.19/0.14.8/"));
        assert!(!url.contains("//server"), "{url}");
    }

    #[tokio::test]
    async fn test_get_paper_jar_url() {
        // Fill v3 serves jars from content-addressed URLs: check the host, file name and build.
        async fn check(version: &str, build: Option<i64>, expected_build: i64) {
            let (url, flavour) = super::get_paper_jar_url(version, &build.map(PaperBuildVersion))
                .await
                .unwrap_or_else(|e| panic!("no jar for paper {version}: {e}"));
            assert!(url.starts_with("https://fill-data.papermc.io/"), "{url}");
            assert!(
                url.ends_with(&format!("/paper-{version}-{expected_build}.jar")),
                "{url}"
            );
            assert_eq!(
                flavour,
                Flavour::Paper {
                    build_version: Some(PaperBuildVersion(expected_build))
                }
            );
        }
        check("1.19.3", Some(308), 308).await;
        check("1.13-pre7", Some(1), 1).await;
        // newest stable build of a version that no longer gets builds
        check("1.19", None, 81).await;

        assert!(super::get_paper_jar_url("1.19.3bruh", &None).await.is_err());
        assert!(
            super::get_paper_jar_url("1.19.3", &Some(PaperBuildVersion(999_999)))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_get_forge_jar_url() {
        get_forge_jar_url("1.18.2", &None).await.unwrap();
    }

    #[tokio::test]
    async fn test_get_server_jar_url() {
        assert_eq!(
            get_server_jar_url("1.7.10", &Flavour::Forge { build_version: None }).await.unwrap(),
            (
                "https://maven.minecraftforge.net/net/minecraftforge/forge/1.7.10-10.13.4.1614-1.7.10/forge-1.7.10-10.13.4.1614-1.7.10-installer.jar".to_string(),
                Flavour::Forge { build_version: Some(ForgeBuildVersion("1.7.10-10.13.4.1614-1.7.10".to_string())) }
            )
        );
        assert_eq!(
            get_server_jar_url("1.7.10_pre4", &Flavour::Forge { build_version: None }).await.unwrap(),
            (
                "https://maven.minecraftforge.net/net/minecraftforge/forge/1.7.10_pre4-10.12.2.1149-prerelease/forge-1.7.10_pre4-10.12.2.1149-prerelease-installer.jar".to_string(),
                Flavour::Forge { build_version: Some(ForgeBuildVersion("1.7.10_pre4-10.12.2.1149-prerelease".to_string())) }
            )
        );
        assert!(get_server_jar_url(
            "1.19.3bruh",
            &Flavour::Forge {
                build_version: None
            }
        )
        .await
        .is_err());
        assert!(matches!(
            get_server_jar_url("1.20.1", &Flavour::Spigot)
                .await
                .unwrap_err()
                .kind,
            ErrorKind::UnsupportedOperation
        ));
    }
}
