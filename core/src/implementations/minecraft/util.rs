//! Version and download lookups for Minecraft servers (Mojang, Fabric, Forge, NeoForge,
//! Paper, Adoptium).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use color_eyre::eyre::{eyre, Context};
use lodestone_minecraft::versions::fabric::{
    select_fabric_installer, select_fabric_loader, FabricLoaderEntry, FabricStable,
};
use lodestone_minecraft::versions::forge::{forge_build_for, newest_forge_build};
use lodestone_minecraft::versions::mojang::{
    find_mojang_version, jre_major_version, vanilla_server_url, MojangVersion, MojangVersionInfo,
    MojangVersionManifest, VERSION_MANIFEST_URL,
};
use lodestone_minecraft::versions::neoforge::{neoforge_installer_url, newest_neoforge_build};
use serde::Deserialize;

use super::{
    FabricInstallerVersion, FabricLoaderVersion, Flavour, ForgeBuildVersion, NeoForgeBuildVersion,
    PaperBuildVersion,
};
use crate::error::{Error, ErrorKind};

/// How long version lists are cached.
pub(super) const VERSION_CACHE_TTL: Duration = Duration::from_secs(10 * 60);

/// Minecraft API calls time out after this long.
const API_TIMEOUT: Duration = Duration::from_secs(30);

/// A GET request for a Minecraft API, on the core's shared client (which sends Lodestone's
/// user agent, as PaperMC asks) with [`API_TIMEOUT`], since that client has no overall
/// timeout of its own.
pub(super) fn api_get(url: impl reqwest::IntoUrl) -> reqwest::RequestBuilder {
    crate::util::http_client().get(url).timeout(API_TIMEOUT)
}

fn not_found(message: String) -> Error {
    Error {
        kind: ErrorKind::NotFound,
        source: eyre!(message),
    }
}

async fn get_json<T: serde::de::DeserializeOwned>(url: &str, what: &str) -> Result<T, Error> {
    Ok(api_get(url)
        .send()
        .await
        .with_context(|| format!("Failed to get {what}: the request to {url} failed"))?
        .error_for_status()
        .with_context(|| format!("Failed to get {what}"))?
        .json()
        .await
        .with_context(|| format!("Failed to get {what}: unexpected response from {url}"))?)
}

// ---- Mojang ----

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
        get_json(VERSION_MANIFEST_URL, "the Minecraft version list").await?;
    let versions = Arc::new(manifest.versions);
    *cache = Some((Instant::now(), versions.clone()));
    Ok(versions)
}

async fn mojang_version_info(version: &str) -> Result<MojangVersionInfo, Error> {
    let versions = mojang_versions().await?;
    let entry = find_mojang_version(&versions, version)?;
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
        Flavour::NeoForge { build_version } => get_neoforge_jar_url(version, build_version).await,
    }
}

pub async fn get_vanilla_jar_url(version: &str) -> Result<(String, Flavour), Error> {
    let info = mojang_version_info(version).await?;
    Ok((vanilla_server_url(info, version)?, Flavour::Vanilla))
}

async fn newest_fabric_loader(version: &str) -> Result<String, Error> {
    let entries: Vec<FabricLoaderEntry> = get_json(
        &format!("https://meta.fabricmc.net/v2/versions/loader/{version}"),
        "Fabric loader versions",
    )
    .await?;
    Ok(select_fabric_loader(&entries, version)?)
}

async fn newest_fabric_installer() -> Result<String, Error> {
    let entries: Vec<FabricStable> = get_json(
        "https://meta.fabricmc.net/v2/versions/installer",
        "Fabric installer versions",
    )
    .await?;
    Ok(select_fabric_installer(entries)?)
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

/// The Forge build a modpack names for `version` (`47.2.0` for `1.20.1-47.2.0`), checked
/// against Forge's version list.
pub async fn forge_build_for_pack(version: &str, forge_version: &str) -> Result<String, Error> {
    let metadata: BTreeMap<String, Vec<String>> = get_json(
        "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json",
        "Forge versions",
    )
    .await?;
    Ok(forge_build_for(&metadata, version, forge_version)?)
}

/// The installer of NeoForge `neoforge_build_version` (else the newest for `version`).
/// A version that is not for `version` is refused.
pub async fn get_neoforge_jar_url(
    version: &str,
    neoforge_build_version: &Option<NeoForgeBuildVersion>,
) -> Result<(String, Flavour), Error> {
    let build = match neoforge_build_version {
        Some(NeoForgeBuildVersion(b)) => {
            let game = lodestone_minecraft::versions::neoforge::neoforge_game_version(b);
            // the version goes into a URL
            let plain = b
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_'));
            if !plain || game.as_deref() != Some(version) {
                return Err(Error {
                    kind: ErrorKind::BadRequest,
                    source: eyre!("NeoForge {b} is not for Minecraft {version}"),
                });
            }
            b.clone()
        }
        None => newest_neoforge_build(&super::forge::get_neoforge_versions().await?, version)?,
    };
    Ok((
        neoforge_installer_url(&build),
        Flavour::NeoForge {
            build_version: Some(NeoForgeBuildVersion(build)),
        },
    ))
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
    let url = format!("https://api.mojang.com/users/profiles/minecraft/{name}");
    let profile: Profile = api_get(url)
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
