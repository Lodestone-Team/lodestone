//! Version and download lookups for Minecraft servers (Mojang, Fabric, Forge, NeoForge,
//! Paper, and Java from Adoptium and Azul).

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

/// The operating system and processor a JRE is for, as Rust names them
/// (`std::env::consts::OS` and `ARCH`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JrePlatform {
    pub os: &'static str,
    pub arch: &'static str,
}

impl JrePlatform {
    /// This computer.
    pub fn current() -> Self {
        Self {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        }
    }

    /// Whether a JRE for this computer has to be looked for vendor by vendor: on Apple
    /// silicon Macs and Windows on ARM, Temurin has no build of some Java versions that
    /// Minecraft needs (8 and 16 on both; 17 and 25 on Windows too). The x64 builds would run
    /// there, emulated (Rosetta, Windows' x64 emulation), but Lodestone never runs servers
    /// that way: they are slower, and the emulation can be missing or turned off.
    /// Everywhere else Temurin has every Java version Minecraft uses.
    fn needs_native_lookup(self) -> bool {
        self.arch == "aarch64" && matches!(self.os, "macos" | "windows")
    }

    /// The name Adoptium gives the operating system.
    fn adoptium_os(self) -> &'static str {
        if self.os == "macos" {
            "mac"
        } else {
            self.os
        }
    }

    /// The name Adoptium gives the processor.
    fn adoptium_arch(self) -> &'static str {
        if self.arch == "x86_64" {
            "x64"
        } else {
            self.arch
        }
    }

    /// The archive type the JRE comes in, which `unzip_file_async` can unpack.
    fn archive_type(self) -> &'static str {
        if self.os == "windows" {
            "zip"
        } else {
            "tar.gz"
        }
    }

    /// How a person would name this computer, for messages.
    fn description(self) -> String {
        let os = match self.os {
            "macos" => "macOS",
            "windows" => "Windows",
            "linux" => "Linux",
            other => other,
        };
        let arch = match (self.os, self.arch) {
            ("macos", "aarch64") => "Apple silicon",
            (_, "aarch64") => "ARM",
            (_, other) => other,
        };
        format!("{os} on {arch}")
    }
}

/// Who builds a JRE that Lodestone downloads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JreVendor {
    /// Eclipse Temurin, from Adoptium: Lodestone's usual Java.
    Temurin,
    /// Azul Zulu, which has native builds that Temurin lacks, such as Java 8 for Apple
    /// silicon.
    Zulu,
}

impl std::fmt::Display for JreVendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            JreVendor::Temurin => "Eclipse Temurin",
            JreVendor::Zulu => "Azul Zulu",
        })
    }
}

/// The kind of Java package. A JDK is a JRE with the developer tools added; it runs servers
/// the same way, so it is used when a vendor has no JRE for this computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JavaPackage {
    Jre,
    Jdk,
}

impl JavaPackage {
    fn name(self) -> &'static str {
        match self {
            JavaPackage::Jre => "jre",
            JavaPackage::Jdk => "jdk",
        }
    }
}

/// A Java runtime archive to download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JreDownload {
    pub url: String,
    /// The SHA-256 of the archive (in hex) that the vendor publishes, which the download
    /// must match. None only for the direct Temurin download Lodestone has always used.
    pub sha256: Option<String>,
    pub vendor: JreVendor,
}

/// The Adoptium URL that downloads Temurin's newest JRE of a Java major version for
/// `platform` (it redirects to the archive).
fn temurin_binary_url(major_java_version: u64, platform: JrePlatform) -> String {
    format!(
        "https://api.adoptium.net/v3/binary/latest/{major_java_version}/ga/{}/{}/jre/hotspot/normal/eclipse",
        platform.adoptium_os(),
        platform.adoptium_arch()
    )
}

/// The vendors and packages to look in, in order, for a JRE on a computer that
/// [needs a native lookup](JrePlatform::needs_native_lookup). Each lookup is for the exact
/// Java major version the Minecraft version needs and for this computer's own processor,
/// never for x64: Minecraft and its loaders are only known to work on the Java version
/// Mojang names (Forge for 1.16.5 and older, for one, doesn't start on newer Java).
pub(crate) const NATIVE_JRE_SOURCES: [(JreVendor, JavaPackage); 4] = [
    (JreVendor::Temurin, JavaPackage::Jre),
    (JreVendor::Zulu, JavaPackage::Jre),
    (JreVendor::Temurin, JavaPackage::Jdk),
    (JreVendor::Zulu, JavaPackage::Jdk),
];

/// The Adoptium API's newest Temurin build of a Java major version for `platform`.
fn temurin_assets_url(major: u64, platform: JrePlatform, package: JavaPackage) -> String {
    format!(
        "https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={}&image_type={}&os={}&vendor=eclipse",
        platform.adoptium_arch(),
        package.name(),
        platform.adoptium_os()
    )
}

#[derive(Deserialize)]
struct AdoptiumAsset {
    binary: AdoptiumBinary,
    version: AdoptiumVersion,
}

#[derive(Deserialize)]
struct AdoptiumBinary {
    architecture: String,
    os: String,
    image_type: String,
    #[serde(default)]
    heap_size: String,
    package: AdoptiumPackage,
}

#[derive(Deserialize)]
struct AdoptiumPackage {
    link: String,
    name: String,
    #[serde(default)]
    checksum: Option<String>,
}

#[derive(Deserialize)]
struct AdoptiumVersion {
    major: u64,
}

/// Whether `sha256` looks like a SHA-256 in hex.
fn is_sha256(sha256: &str) -> bool {
    sha256.len() == 64 && sha256.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Picks Temurin's build from the Adoptium API's answer, checking that it is the build
/// asked for, comes from Adoptium's releases on GitHub and has a checksum.
fn pick_temurin(
    assets: &[AdoptiumAsset],
    major: u64,
    platform: JrePlatform,
    package: JavaPackage,
) -> Option<JreDownload> {
    assets
        .iter()
        .filter(|a| {
            a.version.major == major
                && a.binary.architecture == platform.adoptium_arch()
                && a.binary.os == platform.adoptium_os()
                && a.binary.image_type == package.name()
                && (a.binary.heap_size.is_empty() || a.binary.heap_size == "normal")
                && a.binary
                    .package
                    .link
                    .starts_with("https://github.com/adoptium/")
                && a.binary
                    .package
                    .name
                    .ends_with(&format!(".{}", platform.archive_type()))
                && a.binary.package.checksum.as_deref().is_some_and(is_sha256)
        })
        .map(|a| JreDownload {
            url: a.binary.package.link.clone(),
            sha256: a.binary.package.checksum.clone(),
            vendor: JreVendor::Temurin,
        })
        .next()
}

/// Azul's metadata API's newest Zulu builds of a Java major version for `platform` (it
/// names operating systems as Rust does).
fn zulu_packages_url(major: u64, platform: JrePlatform, package: JavaPackage) -> String {
    format!(
        "https://api.azul.com/metadata/v1/zulu/packages/?java_version={major}&os={}&arch={}&archive_type={}&java_package_type={}&javafx_bundled=false&crac_supported=false&release_status=ga&availability_types=CA&latest=true&include_fields=sha256_hash&page_size=20",
        platform.os,
        platform.arch,
        platform.archive_type(),
        package.name()
    )
}

#[derive(Deserialize)]
struct ZuluPackage {
    name: String,
    download_url: String,
    java_version: Vec<u64>,
    #[serde(default)]
    distro_version: Vec<u64>,
    #[serde(default)]
    sha256_hash: Option<String>,
}

/// Picks the newest Zulu build from Azul's answer, checking that it is the Java version
/// asked for, for this processor, from Azul's own CDN and with a checksum.
fn pick_zulu(
    packages: &[ZuluPackage],
    major: u64,
    platform: JrePlatform,
    package: JavaPackage,
) -> Option<JreDownload> {
    // Azul names its files ...-jre8.0.512-macosx_aarch64.tar.gz or ...-win_aarch64.zip
    let os_tag = match platform.os {
        "macos" => "macosx",
        "windows" => "win",
        other => other,
    };
    let suffix = format!("-{os_tag}_{}.{}", platform.arch, platform.archive_type());
    let kind = format!("-{}{major}", package.name());
    packages
        .iter()
        .filter(|p| {
            p.java_version.first() == Some(&major)
                && p.name.ends_with(&suffix)
                && p.name.contains(&kind)
                && p.download_url.starts_with("https://cdn.azul.com/zulu/bin/")
                && p.download_url.ends_with(&format!("/{}", p.name))
                && p.sha256_hash.as_deref().is_some_and(is_sha256)
        })
        .max_by(|a, b| {
            (&a.java_version, &a.distro_version).cmp(&(&b.java_version, &b.distro_version))
        })
        .map(|p| JreDownload {
            url: p.download_url.clone(),
            sha256: p.sha256_hash.clone(),
            vendor: JreVendor::Zulu,
        })
}

/// The error when no vendor has a Java `major` made for this computer's processor.
fn no_native_jre(major: u64, platform: JrePlatform) -> Error {
    let sources = NATIVE_JRE_SOURCES
        .iter()
        .map(|(vendor, _)| vendor.to_string())
        .fold(Vec::<String>::new(), |mut names, name| {
            if !names.contains(&name) {
                names.push(name);
            }
            names
        })
        .join(" nor ");
    Error {
        kind: ErrorKind::UnsupportedOperation,
        source: eyre!(
            "This Minecraft version needs Java {major}, and no Java {major} is made for this \
             computer ({}): neither {sources} publishes one. Lodestone doesn't run servers \
             on Java made for other processors (under emulation), so this version can't run \
             on this computer.",
            platform.description()
        ),
    }
}

/// A list of Java builds from a vendor's API, where "not found" means there are none (the
/// APIs answer an empty list, but Adoptium has answered 404 for a version with no builds).
/// Any other failure is an error, so that an outage isn't taken for "no Java for this
/// computer".
async fn get_build_list<T: serde::de::DeserializeOwned>(
    url: &str,
    what: &str,
) -> Result<Vec<T>, Error> {
    let response = api_get(url)
        .send()
        .await
        .with_context(|| format!("Failed to get {what}: the request to {url} failed"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(Vec::new());
    }
    Ok(response
        .error_for_status()
        .with_context(|| format!("Failed to get {what}"))?
        .json()
        .await
        .with_context(|| format!("Failed to get {what}: unexpected response from {url}"))?)
}

/// Finds the JRE to download for a Java major version on this computer, before anything is
/// downloaded: always a build for this computer's own processor, of exactly that Java
/// version. Fails with an error saying why if there is none.
pub async fn find_jre_download(major: u64) -> Result<JreDownload, Error> {
    let platform = JrePlatform::current();
    select_jre(major, platform, |vendor, package| async move {
        Ok(match vendor {
            JreVendor::Temurin => {
                let assets: Vec<AdoptiumAsset> = get_build_list(
                    &temurin_assets_url(major, platform, package),
                    &format!("the Temurin Java {major} builds"),
                )
                .await?;
                pick_temurin(&assets, major, platform, package)
            }
            JreVendor::Zulu => {
                let packages: Vec<ZuluPackage> = get_build_list(
                    &zulu_packages_url(major, platform, package),
                    &format!("the Azul Zulu Java {major} builds"),
                )
                .await?;
                pick_zulu(&packages, major, platform, package)
            }
        })
    })
    .await
}

/// [`find_jre_download`] for `platform`, with `lookup` asking a vendor for its build of
/// `major` in a kind of package for `platform`. Most computers get Temurin's build from
/// Adoptium's download link, without a lookup, as they always have.
async fn select_jre<F, Fut>(
    major: u64,
    platform: JrePlatform,
    mut lookup: F,
) -> Result<JreDownload, Error>
where
    F: FnMut(JreVendor, JavaPackage) -> Fut,
    Fut: std::future::Future<Output = Result<Option<JreDownload>, Error>>,
{
    if !platform.needs_native_lookup() {
        return Ok(JreDownload {
            url: temurin_binary_url(major, platform),
            sha256: None,
            vendor: JreVendor::Temurin,
        });
    }
    for (vendor, package) in NATIVE_JRE_SOURCES {
        if let Some(download) = lookup(vendor, package).await? {
            return Ok(download);
        }
    }
    Err(no_native_jre(major, platform))
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

    const MAC_ARM: JrePlatform = JrePlatform {
        os: "macos",
        arch: "aarch64",
    };
    const WINDOWS_ARM: JrePlatform = JrePlatform {
        os: "windows",
        arch: "aarch64",
    };
    const SHA: &str = "54758f5d72e8832c4862ec7d9b53be24616048db42ca325cdf7d29a20bb63132";

    /// Native Java builds that Temurin and Zulu published for Apple silicon and Windows on
    /// ARM (November 2026), as `(platform, vendor, package, major)`.
    const CATALOG: &[(JrePlatform, JreVendor, JavaPackage, u64)] = {
        use JavaPackage::*;
        use JreVendor::*;
        &[
            (MAC_ARM, Temurin, Jre, 11),
            (MAC_ARM, Temurin, Jre, 17),
            (MAC_ARM, Temurin, Jre, 21),
            (MAC_ARM, Temurin, Jre, 25),
            (MAC_ARM, Zulu, Jre, 8),
            (MAC_ARM, Zulu, Jre, 16),
            (MAC_ARM, Zulu, Jre, 17),
            (MAC_ARM, Zulu, Jre, 21),
            (MAC_ARM, Zulu, Jre, 25),
            (WINDOWS_ARM, Temurin, Jre, 21),
            (WINDOWS_ARM, Zulu, Jre, 16),
            (WINDOWS_ARM, Zulu, Jre, 21),
            (WINDOWS_ARM, Zulu, Jre, 25),
            (WINDOWS_ARM, Zulu, Jdk, 16),
            (WINDOWS_ARM, Zulu, Jdk, 17),
        ]
    };

    /// [`select_jre`] with the vendors' answers taken from [`CATALOG`], and the lookups it
    /// made.
    async fn select_from_catalog(
        major: u64,
        platform: JrePlatform,
    ) -> (Result<JreDownload, Error>, Vec<(JreVendor, JavaPackage)>) {
        let asked = std::sync::Mutex::new(Vec::new());
        let result = select_jre(major, platform, |vendor, package| {
            asked.lock().unwrap().push((vendor, package));
            let found = CATALOG.contains(&(platform, vendor, package, major));
            async move {
                Ok(found.then(|| JreDownload {
                    url: format!("{vendor} {} {major} {}", package.name(), platform.arch),
                    sha256: Some(SHA.to_string()),
                    vendor,
                }))
            }
        })
        .await;
        (result, asked.into_inner().unwrap())
    }

    #[tokio::test]
    async fn other_computers_get_temurin_without_a_lookup() {
        for (platform, url) in [
            (
                JrePlatform {
                    os: "linux",
                    arch: "x86_64",
                },
                "https://api.adoptium.net/v3/binary/latest/8/ga/linux/x64/jre/hotspot/normal/eclipse",
            ),
            (
                JrePlatform {
                    os: "linux",
                    arch: "aarch64",
                },
                "https://api.adoptium.net/v3/binary/latest/8/ga/linux/aarch64/jre/hotspot/normal/eclipse",
            ),
            (
                JrePlatform {
                    os: "macos",
                    arch: "x86_64",
                },
                "https://api.adoptium.net/v3/binary/latest/8/ga/mac/x64/jre/hotspot/normal/eclipse",
            ),
            (
                JrePlatform {
                    os: "windows",
                    arch: "x86_64",
                },
                "https://api.adoptium.net/v3/binary/latest/8/ga/windows/x64/jre/hotspot/normal/eclipse",
            ),
        ] {
            let (download, asked) = select_from_catalog(8, platform).await;
            assert_eq!(
                download.unwrap(),
                JreDownload {
                    url: url.to_string(),
                    sha256: None,
                    vendor: JreVendor::Temurin,
                }
            );
            assert!(asked.is_empty(), "{platform:?}");
        }
    }

    #[tokio::test]
    async fn arm_computers_get_native_java_of_the_version_needed() {
        use JavaPackage::*;
        use JreVendor::*;
        for (platform, major, vendor, package) in [
            // Temurin has no Java 8 or 16 for Apple silicon; Zulu does
            (MAC_ARM, 8, Zulu, Jre),
            (MAC_ARM, 16, Zulu, Jre),
            (MAC_ARM, 17, Temurin, Jre),
            (MAC_ARM, 21, Temurin, Jre),
            (MAC_ARM, 25, Temurin, Jre),
            (WINDOWS_ARM, 16, Zulu, Jre),
            // nobody has a Java 17 JRE for Windows on ARM, but Zulu has the JDK
            (WINDOWS_ARM, 17, Zulu, Jdk),
            (WINDOWS_ARM, 21, Temurin, Jre),
            (WINDOWS_ARM, 25, Zulu, Jre),
        ] {
            let (download, _) = select_from_catalog(major, platform).await;
            let download = download.unwrap();
            assert_eq!(
                download.url,
                format!("{vendor} {} {major} aarch64", package.name()),
                "Java {major} on {platform:?}"
            );
            assert_eq!(download.vendor, vendor);
        }
    }

    #[tokio::test]
    async fn no_native_java_is_an_error_not_emulation() {
        // No vendor makes Java 8 for Windows on ARM. Every vendor is asked, for this
        // computer's processor only, and the answer is an error, not an x64 build.
        let (download, asked) = select_from_catalog(8, WINDOWS_ARM).await;
        assert_eq!(asked, NATIVE_JRE_SOURCES.to_vec());
        let error = download.unwrap_err();
        assert!(matches!(error.kind, ErrorKind::UnsupportedOperation));
        let message = error.source.to_string();
        assert!(message.contains("needs Java 8"), "{message}");
        assert!(message.contains("Windows on ARM"), "{message}");
        assert!(
            message.contains("Eclipse Temurin nor Azul Zulu"),
            "{message}"
        );

        let (download, _) = select_from_catalog(12, MAC_ARM).await;
        let message = download.unwrap_err().source.to_string();
        assert!(message.contains("macOS on Apple silicon"), "{message}");
    }

    #[tokio::test]
    async fn a_failed_lookup_is_not_taken_for_no_java() {
        let error = select_jre(8, MAC_ARM, |_, _| async {
            Err::<Option<JreDownload>, _>(eyre!("connection reset").into())
        })
        .await
        .unwrap_err();
        assert!(error.source.to_string().contains("connection reset"));
    }

    #[test]
    fn temurin_builds_are_checked() {
        let asset = |arch: &str, link: &str, name: &str, checksum: Option<&str>| {
            serde_json::json!({
                "binary": {
                    "architecture": arch, "os": "mac", "image_type": "jre",
                    "heap_size": "normal",
                    "package": { "link": link, "name": name, "checksum": checksum },
                },
                "version": { "major": 17 },
            })
        };
        let good_link = "https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.21%2B9/OpenJDK17U-jre_aarch64_mac_hotspot_17.0.21_9.tar.gz";
        let name = "OpenJDK17U-jre_aarch64_mac_hotspot_17.0.21_9.tar.gz";
        let pick = |assets: Vec<serde_json::Value>| {
            let assets: Vec<AdoptiumAsset> =
                serde_json::from_value(serde_json::Value::Array(assets)).unwrap();
            pick_temurin(&assets, 17, MAC_ARM, JavaPackage::Jre)
        };
        assert_eq!(
            pick(vec![
                asset("x64", good_link, name, Some(SHA)),
                asset("aarch64", "https://example.com/jre.tar.gz", name, Some(SHA)),
                asset("aarch64", good_link, name, None),
                asset("aarch64", good_link, name, Some("not a checksum")),
                asset("aarch64", good_link, name, Some(SHA)),
            ]),
            Some(JreDownload {
                url: good_link.to_string(),
                sha256: Some(SHA.to_string()),
                vendor: JreVendor::Temurin,
            })
        );
        assert_eq!(pick(vec![asset("x64", good_link, name, Some(SHA))]), None);
        assert_eq!(pick(Vec::new()), None);
    }

    #[test]
    fn zulu_builds_are_checked_and_the_newest_is_picked() {
        let package = |name: &str, url_base: &str, version: &[u64], sha: Option<&str>| {
            serde_json::json!({
                "name": name,
                "download_url": format!("{url_base}{name}"),
                "java_version": version,
                "distro_version": [version[0], version[2], 0, 0],
                "sha256_hash": sha,
            })
        };
        let cdn = "https://cdn.azul.com/zulu/bin/";
        let newest = "zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.tar.gz";
        let older = "zulu8.94.0.17-ca-jre8.0.492-macosx_aarch64.tar.gz";
        let packages: Vec<ZuluPackage> = serde_json::from_value(serde_json::json!([
            package(older, cdn, &[8, 0, 492], Some(SHA)),
            package(newest, cdn, &[8, 0, 512], Some(SHA)),
            // newer, but each fails a check
            package(
                "zulu8.99.0.1-ca-jre8.0.600-macosx_x64.tar.gz",
                cdn,
                &[8, 0, 600],
                Some(SHA)
            ),
            package(
                "zulu8.99.0.1-ca-jre8.0.600-macosx_aarch64.tar.gz",
                "https://mirror.example.com/",
                &[8, 0, 600],
                Some(SHA)
            ),
            package(
                "zulu8.99.0.2-ca-jre8.0.601-macosx_aarch64.tar.gz",
                cdn,
                &[8, 0, 601],
                None
            ),
            package(
                "zulu8.99.0.3-ca-jdk8.0.602-macosx_aarch64.tar.gz",
                cdn,
                &[8, 0, 602],
                Some(SHA)
            ),
            package(
                "zulu11.99.0.3-ca-jre11.0.40-macosx_aarch64.tar.gz",
                cdn,
                &[11, 0, 40],
                Some(SHA)
            ),
        ]))
        .unwrap();
        assert_eq!(
            pick_zulu(&packages, 8, MAC_ARM, JavaPackage::Jre),
            Some(JreDownload {
                url: format!("{cdn}{newest}"),
                sha256: Some(SHA.to_string()),
                vendor: JreVendor::Zulu,
            })
        );
        // a macOS build is no build for Windows
        assert_eq!(pick_zulu(&packages, 8, WINDOWS_ARM, JavaPackage::Jre), None);
        assert_eq!(pick_zulu(&packages, 17, MAC_ARM, JavaPackage::Jre), None);
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
    #[ignore = "network: Adoptium and Azul still answer the Java lookups for ARM computers"]
    async fn network_native_jre_contract() {
        let find = |major, platform| {
            select_jre(major, platform, move |vendor, package| async move {
                Ok(match vendor {
                    JreVendor::Temurin => {
                        let assets: Vec<AdoptiumAsset> = get_build_list(
                            &temurin_assets_url(major, platform, package),
                            "Temurin builds",
                        )
                        .await?;
                        pick_temurin(&assets, major, platform, package)
                    }
                    JreVendor::Zulu => {
                        let packages: Vec<ZuluPackage> = get_build_list(
                            &zulu_packages_url(major, platform, package),
                            "Zulu builds",
                        )
                        .await?;
                        pick_zulu(&packages, major, platform, package)
                    }
                })
            })
        };
        let mac8 = find(8, MAC_ARM).await.unwrap();
        assert_eq!(mac8.vendor, JreVendor::Zulu);
        assert!(mac8.url.contains("macosx_aarch64"), "{}", mac8.url);
        let mac21 = find(21, MAC_ARM).await.unwrap();
        assert_eq!(mac21.vendor, JreVendor::Temurin);
        assert!(mac21.url.contains("aarch64_mac"), "{}", mac21.url);
        let windows21 = find(21, WINDOWS_ARM).await.unwrap();
        assert!(windows21.url.contains("aarch64"), "{}", windows21.url);
        assert!(find(8, WINDOWS_ARM).await.is_err());
    }

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
