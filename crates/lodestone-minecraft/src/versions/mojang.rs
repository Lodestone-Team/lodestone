//! Mojang's version manifest and version details.

use serde::Deserialize;

use crate::Error;

/// The list of all Minecraft versions.
pub const VERSION_MANIFEST_URL: &str =
    "https://launchermeta.mojang.com/mc/game/version_manifest.json";

/// The same list with each version's SHA-1, which the official launcher uses. Lodestone
/// Launcher reads this one so it can check each version's details.
pub const VERSION_MANIFEST_V2_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// What kind of version an entry is, from its `type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MojangVersionType {
    #[default]
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
    /// A type this crate does not know yet.
    #[serde(other)]
    Other,
}

impl MojangVersionType {
    /// The `${version_type}` the game is started with.
    pub fn as_str(self) -> &'static str {
        match self {
            MojangVersionType::Release => "release",
            MojangVersionType::Snapshot => "snapshot",
            MojangVersionType::OldBeta => "old_beta",
            MojangVersionType::OldAlpha => "old_alpha",
            MojangVersionType::Other => "other",
        }
    }
}

/// An entry of the version manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct MojangVersion {
    pub id: String,
    /// Where the details of the version are.
    pub url: String,
    #[serde(default, rename = "type")]
    pub kind: MojangVersionType,
    /// When the version was released, as an RFC 3339 timestamp.
    #[serde(default, rename = "releaseTime")]
    pub release_time: Option<String>,
    /// The SHA-1 of the details at `url`. Only the v2 manifest has it.
    #[serde(default)]
    pub sha1: Option<String>,
}

/// The newest release and snapshot.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MojangLatest {
    pub release: String,
    pub snapshot: String,
}

/// The version manifest at [`VERSION_MANIFEST_URL`] or [`VERSION_MANIFEST_V2_URL`]. The
/// versions are newest first.
#[derive(Debug, Clone, Deserialize)]
pub struct MojangVersionManifest {
    #[serde(default)]
    pub latest: Option<MojangLatest>,
    pub versions: Vec<MojangVersion>,
}

/// The details of a version, at [`MojangVersion::url`].
#[derive(Debug, Clone, Deserialize)]
pub struct MojangVersionInfo {
    pub downloads: Option<MojangDownloads>,
    #[serde(rename = "javaVersion")]
    pub java_version: Option<MojangJavaVersion>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MojangDownloads {
    pub server: Option<MojangDownload>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MojangDownload {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MojangJavaVersion {
    #[serde(rename = "majorVersion")]
    pub major_version: u64,
    /// The Java runtime Mojang ships for this version, such as `java-runtime-gamma` or
    /// `jre-legacy`.
    #[serde(default)]
    pub component: Option<String>,
}

/// The ids of the versions, in manifest order (newest first).
pub fn version_ids(versions: &[MojangVersion]) -> Vec<String> {
    versions.iter().map(|v| v.id.clone()).collect()
}

/// Mojang uses '-' where Forge version names use '_' (for example 1.7.10_pre4).
pub fn mojang_version_id(version: &str) -> String {
    version.replace('_', "-")
}

/// The manifest entry of `version`, which may use Forge's '_' naming.
pub fn find_mojang_version<'a>(
    versions: &'a [MojangVersion],
    version: &str,
) -> Result<&'a MojangVersion, Error> {
    let id = mojang_version_id(version);
    versions
        .iter()
        .find(|v| v.id == id)
        .ok_or_else(|| Error::NotFound(format!("Minecraft version {version} does not exist")))
}

/// The server jar URL of `version`, from its details.
pub fn vanilla_server_url(info: MojangVersionInfo, version: &str) -> Result<String, Error> {
    info.downloads
        .and_then(|d| d.server)
        .map(|s| s.url)
        .ok_or_else(|| Error::NotFound(format!("Minecraft {version} has no server download")))
}

/// The Java major version a Minecraft version needs, from its details.
pub fn jre_major_version(info: &MojangVersionInfo) -> u64 {
    let major = info.java_version.as_ref().map_or(8, |j| j.major_version);
    // Adoptium doesn't provide Java 16; 17 is compatible and an LTS release
    if major == 16 {
        17
    } else {
        major
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::versions::fixture;

    fn mojang_manifest() -> Vec<MojangVersion> {
        fixture::<MojangVersionManifest>(include_str!(
            "../../testdata/mojang_version_manifest.json"
        ))
        .versions
    }

    fn mojang_info(id: &str) -> MojangVersionInfo {
        fixture(match id {
            "1.18.2" => include_str!("../../testdata/mojang_1.18.2.json"),
            "21w44a" => include_str!("../../testdata/mojang_21w44a.json"),
            "1.8.4" => include_str!("../../testdata/mojang_1.8.4.json"),
            "1.7.10-pre4" => include_str!("../../testdata/mojang_1.7.10-pre4.json"),
            "a1.0.16" => include_str!("../../testdata/mojang_a1.0.16.json"),
            _ => panic!("no fixture for {id}"),
        })
    }

    /// The details of `version`, with the manifest and details read from fixtures.
    fn offline_version_info(version: &str) -> Result<MojangVersionInfo, Error> {
        let versions = mojang_manifest();
        Ok(mojang_info(&find_mojang_version(&versions, version)?.id))
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
            url("1.8.4asdasd").unwrap_err(),
            Error::NotFound(_)
        ));
        // very old versions have no server jar
        let err = url("a1.0.16").unwrap_err();
        assert!(matches!(err, Error::NotFound(_)));
        assert!(err.to_string().contains("no server download"), "{err:?}");
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
    fn reads_types_and_the_latest_versions() {
        let manifest: MojangVersionManifest =
            fixture(include_str!("../../testdata/mojang_version_manifest.json"));
        let latest = manifest.latest.unwrap();
        assert_eq!(latest.release, "26.3");
        assert_eq!(latest.snapshot, "26.4-snapshot-7");
        let kinds: Vec<_> = manifest.versions.iter().map(|v| v.kind).collect();
        assert_eq!(kinds[0], MojangVersionType::Snapshot);
        assert_eq!(kinds[1], MojangVersionType::Release);
        assert_eq!(kinds.last(), Some(&MojangVersionType::OldAlpha));
        assert_eq!(
            manifest.versions[0].release_time.as_deref(),
            Some("2026-11-03T13:08:56+00:00")
        );
        // The v1 manifest has no hashes; unknown types still parse.
        assert_eq!(manifest.versions[0].sha1, None);
        let entry: MojangVersion =
            fixture(r#"{"id": "x", "url": "u", "type": "pending", "sha1": "abc"}"#);
        assert_eq!(entry.kind, MojangVersionType::Other);
        assert_eq!(entry.sha1.as_deref(), Some("abc"));
        let info: MojangVersionInfo =
            fixture(r#"{"javaVersion": {"component": "jre-legacy", "majorVersion": 8}}"#);
        assert_eq!(
            info.java_version.unwrap().component.as_deref(),
            Some("jre-legacy")
        );
    }

    #[test]
    fn lists_manifest_versions_in_order() {
        assert_eq!(
            version_ids(&mojang_manifest()),
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
