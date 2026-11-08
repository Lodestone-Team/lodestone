//! A Minecraft version's details file (`<version>.json`), the parts a client launch
//! needs: the client jar, the libraries, the asset index, the logging config, the
//! Java runtime and the arguments.

use std::collections::BTreeMap;

use lodestone_minecraft::versions::mojang::MojangVersionType;
use serde::Deserialize;

use super::rules::Rule;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionJson {
    pub id: String,
    #[serde(default, rename = "type")]
    pub kind: MojangVersionType,
    pub main_class: String,
    /// The version this one adds to, for mod loader profiles.
    #[serde(default)]
    pub inherits_from: Option<String>,
    /// The arguments of 1.13 and later.
    #[serde(default)]
    pub arguments: Option<Arguments>,
    /// The game arguments of versions before 1.13, as one string.
    #[serde(default)]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    pub asset_index: Option<AssetIndexRef>,
    /// The asset index id, as `${assets_index_name}`.
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(default)]
    pub downloads: BTreeMap<String, Download>,
    #[serde(default)]
    pub java_version: Option<JavaVersion>,
    #[serde(default)]
    pub logging: Option<Logging>,
    /// The SHA-1 of the file this was parsed from. Install records are keyed by it.
    #[serde(skip)]
    pub sha1: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Argument>,
    #[serde(default)]
    pub jvm: Vec<Argument>,
    /// The memory and garbage collector settings Mojang suggests (26.x and later).
    /// They are used when the user has not set their own.
    #[serde(default, rename = "default-user-jvm")]
    pub default_user_jvm: Vec<Argument>,
}

/// An argument: plain, or one or more values under rules.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Argument {
    Plain(String),
    Conditional {
        #[serde(default)]
        rules: Vec<Rule>,
        value: ArgumentValue,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum ArgumentValue {
    One(String),
    Many(Vec<String>),
}

impl ArgumentValue {
    pub fn values(&self) -> &[String] {
        match self {
            ArgumentValue::One(value) => std::slice::from_ref(value),
            ArgumentValue::Many(values) => values,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Download {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// A download with the path it goes to, relative to the libraries directory.
///
/// Mod loaders list some files with an empty `url`: they come out of the loader's
/// installer, or its processors make them.
#[derive(Clone, Debug, Deserialize)]
pub struct Artifact {
    pub path: Option<String>,
    pub sha1: String,
    /// Mojang's files always have it; a few mod loader files do not.
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub url: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct LibraryDownloads {
    pub artifact: Option<Artifact>,
    /// Old-style natives, by classifier (`natives-osx`, `natives-windows-64`).
    #[serde(default)]
    pub classifiers: BTreeMap<String, Artifact>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Library {
    /// The Maven coordinates, `group:artifact:version[:classifier]`.
    pub name: String,
    #[serde(default)]
    pub downloads: LibraryDownloads,
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// Old-style natives: the classifier for each OS, which may contain `${arch}`.
    #[serde(default)]
    pub natives: BTreeMap<String, String>,
    #[serde(default)]
    pub extract: Option<Extract>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    #[serde(default)]
    pub total_size: Option<u64>,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JavaVersion {
    pub component: Option<String>,
    pub major_version: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Logging {
    pub client: Option<LoggingConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LoggingConfig {
    /// The JVM argument, with `${path}` for the config file.
    pub argument: String,
    pub file: LoggingFile,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LoggingFile {
    pub id: String,
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

/// The Java runtime of versions that do not name one: Java 8.
pub const DEFAULT_RUNTIME: &str = "jre-legacy";

impl VersionJson {
    pub fn parse(bytes: &[u8]) -> serde_json::Result<Self> {
        let mut version: Self = serde_json::from_slice(bytes)?;
        version.sha1 = super::download::sha1_hex(bytes);
        Ok(version)
    }

    /// The Java runtime component, such as `java-runtime-gamma`.
    pub fn runtime_component(&self) -> &str {
        self.java_version
            .as_ref()
            .and_then(|java| java.component.as_deref())
            .unwrap_or(DEFAULT_RUNTIME)
    }

    /// The Java major version the game needs.
    pub fn java_major(&self) -> u32 {
        self.java_version
            .as_ref()
            .map_or(8, |java| java.major_version)
    }

    /// The asset index id, as `${assets_index_name}`.
    pub fn assets_id(&self) -> &str {
        self.asset_index
            .as_ref()
            .map(|index| index.id.as_str())
            .or(self.assets.as_deref())
            .unwrap_or("legacy")
    }

    pub fn client_download(&self) -> Option<&Download> {
        self.downloads.get("client")
    }

    pub fn client_logging(&self) -> Option<&LoggingConfig> {
        self.logging.as_ref()?.client.as_ref()
    }

    /// Whether the game logs through Log4j 2, and so needs Mojang's patched logging
    /// config (CVE-2021-44228). Versions before 1.7 have no Log4j.
    pub fn uses_log4j(&self) -> bool {
        self.libraries.iter().any(|library| {
            library
                .name
                .starts_with("org.apache.logging.log4j:log4j-core:")
        })
    }

    /// Whether the game uses LWJGL 3, which needs `-XstartOnFirstThread` on macOS.
    /// LWJGL 2 (`org.lwjgl.lwjgl:*`, before 1.13) uses AWT, which must not have it.
    pub fn uses_lwjgl3(&self) -> bool {
        self.libraries
            .iter()
            .any(|library| library.name.starts_with("org.lwjgl:lwjgl:"))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn fixture(id: &str) -> VersionJson {
        let path = format!("{}/testdata/versions/{id}.json", env!("CARGO_MANIFEST_DIR"));
        VersionJson::parse(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn parses_every_fixture() {
        for (id, component, assets, lwjgl3, legacy_args) in [
            ("1.5.2", "jre-legacy", "pre-1.6", false, true),
            ("1.8.9", "jre-legacy", "1.8", false, true),
            ("1.12.2", "jre-legacy", "1.12", false, true),
            ("1.16.5", "jre-legacy", "1.16", true, false),
            ("1.20.1", "java-runtime-gamma", "5", true, false),
            ("26.3", "java-runtime-epsilon", "34", true, false),
        ] {
            let version = fixture(id);
            assert_eq!(version.id, id);
            assert_eq!(version.runtime_component(), component, "{id}");
            assert_eq!(version.assets_id(), assets, "{id}");
            assert_eq!(version.uses_lwjgl3(), lwjgl3, "{id}");
            assert_eq!(version.minecraft_arguments.is_some(), legacy_args, "{id}");
            assert_eq!(version.arguments.is_some(), !legacy_args, "{id}");
            assert!(version.client_download().is_some(), "{id}");
            assert_eq!(version.kind, MojangVersionType::Release);
        }
        let latest = fixture("26.3");
        assert_eq!(latest.java_major(), 25);
        assert_eq!(
            latest.arguments.unwrap().default_user_jvm.len(),
            3,
            "26.3 suggests memory and GC settings"
        );
    }

    #[test]
    fn log4j_versions_have_logging_configs() {
        assert!(!fixture("1.5.2").uses_log4j());
        assert!(fixture("1.5.2").client_logging().is_none());
        for id in ["1.8.9", "1.12.2", "1.16.5", "1.20.1", "26.3"] {
            let version = fixture(id);
            assert!(version.uses_log4j(), "{id}");
            let logging = version.client_logging().unwrap();
            assert_eq!(logging.argument, "-Dlog4j.configurationFile=${path}");
        }
        assert_eq!(
            fixture("1.8.9").client_logging().unwrap().file.id,
            "client-1.7.xml"
        );
        assert_eq!(
            fixture("1.20.1").client_logging().unwrap().file.id,
            "client-1.12.xml"
        );
    }
}
