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

/// The manifest entry of `version`, which may use Forge's '_' naming.
fn find_mojang_version<'a>(
    versions: &'a [MojangVersion],
    version: &str,
) -> Result<&'a MojangVersion, Error> {
    let id = mojang_version_id(version);
    versions
        .iter()
        .find(|v| v.id == id)
        .ok_or_else(|| not_found(format!("Minecraft version {version} does not exist")))
}

async fn mojang_version_info(version: &str) -> Result<MojangVersionInfo, Error> {
    let versions = mojang_versions().await?;
    let entry = find_mojang_version(&versions, version)?;
    get_json(&entry.url, &format!("the details of Minecraft {version}")).await
}

fn vanilla_server_url(info: MojangVersionInfo, version: &str) -> Result<String, Error> {
    info.downloads
        .and_then(|d| d.server)
        .map(|s| s.url)
        .ok_or_else(|| not_found(format!("Minecraft {version} has no server download")))
}

fn jre_major_version(info: &MojangVersionInfo) -> u64 {
    let major = info.java_version.as_ref().map_or(8, |j| j.major_version);
    // Adoptium doesn't provide Java 16; 17 is compatible and an LTS release
    if major == 16 {
        17
    } else {
        major
    }
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
    Ok((vanilla_server_url(info, version)?, Flavour::Vanilla))
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
    select_fabric_loader(&entries, version)
}

/// The newest loader whose loader and intermediary are both stable, or else the newest
/// stable loader.
fn select_fabric_loader(entries: &[FabricLoaderEntry], version: &str) -> Result<String, Error> {
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
    select_fabric_installer(entries)
}

fn select_fabric_installer(entries: Vec<FabricStable>) -> Result<String, Error> {
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
            let metadata: BTreeMap<String, Vec<String>> = get_json(
                "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json",
                "Forge versions",
            )
            .await?;
            newest_forge_build(&metadata, version)?
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

/// The newest Forge build for `version` in Forge's maven metadata, which lists builds
/// oldest first.
fn newest_forge_build(
    metadata: &BTreeMap<String, Vec<String>>,
    version: &str,
) -> Result<String, Error> {
    metadata
        .get(version)
        .and_then(|builds| builds.last())
        .cloned()
        .ok_or_else(|| not_found(format!("Forge has no builds for Minecraft {version}")))
}

// ---- Java ----

/// The Java major version that a Minecraft version needs.
pub async fn get_jre_major_version(version: &str) -> Result<u64, Error> {
    Ok(jre_major_version(&mojang_version_info(version).await?))
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

    /// Recorded API responses, trimmed to what the tests need.
    fn fixture<T: serde::de::DeserializeOwned>(json: &str) -> T {
        serde_json::from_str(json).unwrap()
    }

    fn mojang_manifest() -> Vec<MojangVersion> {
        fixture::<MojangVersionManifest>(include_str!("testdata/mojang_version_manifest.json"))
            .versions
    }

    fn mojang_info(id: &str) -> MojangVersionInfo {
        fixture(match id {
            "1.18.2" => include_str!("testdata/mojang_1.18.2.json"),
            "21w44a" => include_str!("testdata/mojang_21w44a.json"),
            "1.8.4" => include_str!("testdata/mojang_1.8.4.json"),
            "1.7.10-pre4" => include_str!("testdata/mojang_1.7.10-pre4.json"),
            "a1.0.16" => include_str!("testdata/mojang_a1.0.16.json"),
            _ => panic!("no fixture for {id}"),
        })
    }

    /// What `mojang_version_info` returns, with the manifest and details read from fixtures.
    fn offline_version_info(version: &str) -> Result<MojangVersionInfo, Error> {
        let versions = mojang_manifest();
        Ok(mojang_info(&find_mojang_version(&versions, version)?.id))
    }

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

    #[test]
    fn vanilla_jar_urls_come_from_the_version_details() {
        let url = |version: &str| vanilla_server_url(offline_version_info(version)?, version);
        assert_eq!(url("1.18.2").unwrap(), "https://piston-data.mojang.com/v1/objects/c8f83c5655308435b3dcf03c06d9fe8740a77469/server.jar");
        assert_eq!(url("21w44a").unwrap(), "https://piston-data.mojang.com/v1/objects/ae583fd57a8c07f2d6fbadce1ce1e1379bf4b32d/server.jar");
        assert_eq!(url("1.8.4").unwrap(), "https://launcher.mojang.com/v1/objects/dd4b5eba1c79500390e0b0f45162fa70d38f8a3d/server.jar");

        assert!(matches!(
            url("1.8.4asdasd").unwrap_err().kind,
            ErrorKind::NotFound
        ));
        // very old versions have no server jar
        let err = url("a1.0.16").unwrap_err();
        assert!(matches!(err.kind, ErrorKind::NotFound));
        assert!(
            err.source.to_string().contains("no server download"),
            "{err:?}"
        );
    }

    #[test]
    fn java_versions_come_from_the_version_details() {
        let major = |version: &str| offline_version_info(version).map(|i| jre_major_version(&i));
        assert_eq!(major("1.8.4").unwrap(), 8);
        assert_eq!(major("1.18.2").unwrap(), 17);
        // Mojang says 16, which Adoptium does not provide
        assert_eq!(major("21w44a").unwrap(), 17);
        // Forge names pre-releases with '_'
        assert_eq!(major("1.7.10_pre4").unwrap(), 8);
        assert!(major("1.8.4asdasd").is_err());
        // no javaVersion at all means Java 8
        let info: MojangVersionInfo = fixture(r#"{ "downloads": null }"#);
        assert_eq!(jre_major_version(&info), 8);
    }

    #[test]
    fn newest_stable_fabric_loader_and_installer() {
        let loaders: Vec<FabricLoaderEntry> =
            fixture(include_str!("testdata/fabric_loader_1.19.json"));
        // 0.20.0 and the older ones are not stable
        assert_eq!(select_fabric_loader(&loaders, "1.19").unwrap(), "0.20.1");

        // a loader with an unstable intermediary is only used when nothing better exists
        let loaders: Vec<FabricLoaderEntry> = fixture(
            r#"[
                { "loader": { "version": "0.16.0", "stable": true }, "intermediary": { "version": "x", "stable": false } },
                { "loader": { "version": "0.15.9", "stable": true }, "intermediary": { "version": "x", "stable": true } },
                { "loader": { "version": "0.17.0", "stable": false }, "intermediary": { "version": "x", "stable": true } }
            ]"#,
        );
        assert_eq!(select_fabric_loader(&loaders, "x").unwrap(), "0.15.9");
        assert_eq!(select_fabric_loader(&loaders[..1], "x").unwrap(), "0.16.0");
        let err = select_fabric_loader(&loaders[2..], "x").unwrap_err();
        assert!(matches!(err.kind, ErrorKind::NotFound));
        assert!(select_fabric_loader(&[], "x").is_err());

        let installers: Vec<FabricStable> = fixture(include_str!("testdata/fabric_installer.json"));
        assert_eq!(select_fabric_installer(installers).unwrap(), "1.1.2");
        let unstable: Vec<FabricStable> = fixture(r#"[{ "version": "1.0.0", "stable": false }]"#);
        assert!(select_fabric_installer(unstable).is_err());
    }

    #[tokio::test]
    async fn fabric_jar_url_with_known_versions() {
        // with both versions given, nothing is looked up
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
    }

    #[test]
    fn newest_forge_build_is_the_last_one_listed() {
        let metadata: BTreeMap<String, Vec<String>> =
            fixture(include_str!("testdata/forge_maven_metadata.json"));
        assert_eq!(
            newest_forge_build(&metadata, "1.7.10").unwrap(),
            "1.7.10-10.13.4.1614-1.7.10"
        );
        assert_eq!(
            newest_forge_build(&metadata, "1.7.10_pre4").unwrap(),
            "1.7.10_pre4-10.12.2.1149-prerelease"
        );
        let err = newest_forge_build(&metadata, "1.19.3bruh").unwrap_err();
        assert!(matches!(err.kind, ErrorKind::NotFound));
        let empty: BTreeMap<String, Vec<String>> = fixture(r#"{ "1.20.1": [] }"#);
        assert!(newest_forge_build(&empty, "1.20.1").is_err());
    }

    #[tokio::test]
    async fn forge_jar_url_with_known_build() {
        let build = "1.7.10_pre4-10.12.2.1149-prerelease".to_string();
        assert_eq!(
            get_server_jar_url("1.7.10_pre4", &Flavour::Forge { build_version: Some(ForgeBuildVersion(build.clone())) }).await.unwrap(),
            (
                "https://maven.minecraftforge.net/net/minecraftforge/forge/1.7.10_pre4-10.12.2.1149-prerelease/forge-1.7.10_pre4-10.12.2.1149-prerelease-installer.jar".to_string(),
                Flavour::Forge { build_version: Some(ForgeBuildVersion(build)) }
            )
        );
    }

    #[tokio::test]
    async fn spigot_is_not_supported() {
        assert!(matches!(
            get_server_jar_url("1.20.1", &Flavour::Spigot)
                .await
                .unwrap_err()
                .kind,
            ErrorKind::UnsupportedOperation
        ));
    }

    // Contract checks against the live APIs: they only check that the services still
    // answer the way the code expects. Run them with `cargo test -- --ignored`.

    #[tokio::test]
    #[ignore = "network: Mojang's version manifest and version details still have the fields we read"]
    async fn network_mojang_contract() {
        let versions = crate::minecraft::vanilla::get_vanilla_minecraft_versions()
            .await
            .unwrap();
        for v in ["1.20.1", "1.16.5", "1.12.2", "1.8.9", "21w44a"] {
            assert!(versions.contains(&v.to_string()), "missing {v}");
        }
        assert_eq!(super::get_vanilla_jar_url("1.18.2").await.unwrap(), ("https://piston-data.mojang.com/v1/objects/c8f83c5655308435b3dcf03c06d9fe8740a77469/server.jar".to_string(), Flavour::Vanilla));
        assert_eq!(super::get_jre_major_version("1.18.2").await.unwrap(), 17);
        assert_eq!(
            super::get_jre_major_version("1.7.10_pre4").await.unwrap(),
            8
        );
        let err = super::get_vanilla_jar_url("1.8.4asdasd").await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::NotFound));
    }

    #[tokio::test]
    #[ignore = "network: Fabric meta still lists game versions, loaders and installers"]
    async fn network_fabric_contract() {
        let versions = crate::minecraft::fabric::get_fabric_minecraft_versions()
            .await
            .unwrap();
        assert!(versions.contains(&"1.17.1".to_string()));
        assert!(versions.contains(&"21w19a".to_string()));
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
        let (_, flavour) = super::get_fabric_jar_url("21w44a", &None, &None)
            .await
            .unwrap();
        assert!(matches!(
            flavour,
            Flavour::Fabric {
                loader_version: Some(_),
                installer_version: Some(_)
            }
        ));
    }

    #[tokio::test]
    #[ignore = "network: Forge's maven metadata still maps Minecraft versions to builds"]
    async fn network_forge_contract() {
        let versions = crate::minecraft::forge::get_forge_minecraft_versions()
            .await
            .unwrap();
        for v in ["1.16.5", "1.16.4", "1.16.1"] {
            assert!(versions.contains(&v.to_string()), "missing {v}");
        }
        // newest first
        let pos = |v: &str| versions.iter().position(|x| x == v).unwrap();
        assert!(pos("1.20.1") < pos("1.16.5"));
        assert_eq!(
            get_server_jar_url("1.7.10", &Flavour::Forge { build_version: None }).await.unwrap(),
            (
                "https://maven.minecraftforge.net/net/minecraftforge/forge/1.7.10-10.13.4.1614-1.7.10/forge-1.7.10-10.13.4.1614-1.7.10-installer.jar".to_string(),
                Flavour::Forge { build_version: Some(ForgeBuildVersion("1.7.10-10.13.4.1614-1.7.10".to_string())) }
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
    }

    #[tokio::test]
    #[ignore = "network: PaperMC's Fill v3 API still lists versions and builds with downloads"]
    async fn network_paper_contract() {
        let versions = crate::minecraft::paper::get_paper_minecraft_versions()
            .await
            .unwrap();
        let pos = |v: &str| versions.iter().position(|x| x == v).unwrap();
        assert!(pos("1.20.1") < pos("1.16.5"));
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
        // newest stable build of a version that no longer gets builds
        check("1.19", None, 81).await;
        assert!(super::get_paper_jar_url("1.19.3bruh", &None).await.is_err());
        assert!(
            super::get_paper_jar_url("1.19.3", &Some(PaperBuildVersion(999_999)))
                .await
                .is_err()
        );
    }
}
