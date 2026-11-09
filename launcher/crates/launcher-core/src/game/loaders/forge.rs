//! Forge and NeoForge: their version lists, and the profiles inside their installers.
//!
//! An installer jar holds, depending on its age:
//! - **Forge 1.13 and later, NeoForge** (`spec` 0 or 1, with processors): an
//!   `install_profile.json` (libraries for the processors, `data`, `processors`) and a
//!   `version.json` (the profile), plus Forge's own jars under `maven/`;
//! - **Forge 1.12.2 builds from 14.23.5.2851**: the same files without processors;
//! - **older Forge** (1.7.10, 1.8.9, earlier 1.12.2 builds): one `install_profile.json`
//!   with `install` (where the universal jar goes) and `versionInfo` (the profile),
//!   whose libraries are `{name, url?, checksums?, clientreq?}`, and the universal jar
//!   at the root.
//!
//! The installer GUI is never run. [`super::processors`] runs the processors.

use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::Read as _,
    path::{Path, PathBuf},
};

use lodestone_minecraft::versions::compare_versions;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::{LoaderError, LoaderKind, LoaderVersion, maven_url};
use crate::game::{
    download::sha1_hex,
    libraries::{maven_path, safe_relative_path},
    version::Library,
};

pub const FORGE_MAVEN: &str = "https://maven.minecraftforge.net";
pub const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases";
pub const FORGE_METADATA_URL: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
pub const FORGE_PROMOTIONS_URL: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";
pub const NEOFORGE_VERSIONS_URL: &str =
    "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";
/// Where old Forge profiles' libraries without a `url` come from.
pub const MOJANG_LIBRARIES: &str = "https://libraries.minecraft.net/";

/// The largest JSON file read from an installer.
const MAX_JSON_BYTES: u64 = 16 * 1024 * 1024;

/// The Maven coordinate of the installer jar.
pub fn installer_coordinate(kind: LoaderKind, version: &str) -> String {
    match kind {
        LoaderKind::NeoForge => format!("net.neoforged:neoforge:{version}:installer"),
        _ => format!("net.minecraftforge:forge:{version}:installer"),
    }
}

/// The URL of the installer jar.
pub fn installer_url(kind: LoaderKind, version: &str) -> Result<String, LoaderError> {
    let path = maven_path(&installer_coordinate(kind, version), None)
        .map_err(|err| LoaderError::Invalid(err.to_string()))?;
    Ok(match kind {
        LoaderKind::NeoForge => maven_url(NEOFORGE_MAVEN, &path),
        _ => maven_url(FORGE_MAVEN, &path),
    })
}

#[derive(Deserialize)]
struct Promotions {
    #[serde(default)]
    promos: BTreeMap<String, String>,
}

/// Forge's builds for `game_version`, newest first, from its Maven metadata (the
/// Minecraft versions with their builds, oldest first) and its promotions. The
/// "recommended" promotion is marked; without one, the "latest".
pub fn parse_forge_versions(
    metadata: &[u8],
    promotions: &[u8],
    game_version: &str,
) -> Result<Vec<LoaderVersion>, LoaderError> {
    let metadata: BTreeMap<String, Vec<String>> = serde_json::from_slice(metadata)
        .map_err(|err| LoaderError::Invalid(format!("Forge's version list is invalid: {err}")))?;
    // The promotions only decorate the list.
    let promos = serde_json::from_slice::<Promotions>(promotions)
        .map(|p| p.promos)
        .unwrap_or_default();
    let promoted = promos
        .get(&format!("{game_version}-recommended"))
        .or_else(|| promos.get(&format!("{game_version}-latest")));
    let builds = metadata.get(game_version).cloned().unwrap_or_default();
    Ok(builds
        .into_iter()
        .rev()
        .map(|id| {
            let display = forge_display(&id, game_version);
            LoaderVersion {
                recommended: promoted == Some(&display),
                stable: true,
                display,
                id,
            }
        })
        .collect())
}

/// `47.4.28` for `1.20.1-47.4.28`, and `11.15.1.2318` for `1.8.9-11.15.1.2318-1.8.9`.
pub fn forge_display(id: &str, game_version: &str) -> String {
    let rest = id
        .strip_prefix(game_version)
        .and_then(|rest| rest.strip_prefix('-'))
        .unwrap_or(id);
    rest.strip_suffix(&format!("-{game_version}"))
        .unwrap_or(rest)
        .to_string()
}

/// The Minecraft version a NeoForge version is for: `21.1.209` is 1.21.1, `21.0.167`
/// is 1.21, `20.2.3-beta` is 1.20.2, and since Minecraft's 2026 numbering
/// `26.3.0.101` is 26.3 and `26.1.1.4` is 26.1.1. Snapshot builds have none.
pub fn neoforge_game_version(version: &str) -> Option<String> {
    let numbers: Vec<u32> = version
        .split(['-', '+'])
        .next()?
        .split('.')
        .map(|part| part.parse().ok())
        .collect::<Option<_>>()?;
    match numbers.as_slice() {
        [major, minor, patch, _build] if *major >= 26 => Some(if *patch == 0 {
            format!("{major}.{minor}")
        } else {
            format!("{major}.{minor}.{patch}")
        }),
        [major, minor, _build] if (20..26).contains(major) => Some(if *minor == 0 {
            format!("1.{major}")
        } else {
            format!("1.{major}.{minor}")
        }),
        _ => None,
    }
}

#[derive(Deserialize)]
struct NeoForgeVersions {
    #[serde(default)]
    versions: Vec<String>,
}

/// NeoForge's versions for `game_version`, newest first. Betas are not stable; the
/// newest stable one is recommended.
pub fn parse_neoforge_versions(
    bytes: &[u8],
    game_version: &str,
) -> Result<Vec<LoaderVersion>, LoaderError> {
    let list: NeoForgeVersions = serde_json::from_slice(bytes).map_err(|err| {
        LoaderError::Invalid(format!("NeoForge's version list is invalid: {err}"))
    })?;
    let mut versions: Vec<String> = list
        .versions
        .into_iter()
        .filter(|v| neoforge_game_version(v).as_deref() == Some(game_version))
        .collect();
    versions.sort_by(|a, b| compare_versions(b, a));
    let stable = |v: &str| !v.contains("beta") && !v.contains("alpha");
    let recommended = versions.iter().find(|v| stable(v)).cloned();
    Ok(versions
        .into_iter()
        .map(|id| LoaderVersion {
            stable: stable(&id),
            recommended: recommended.as_ref() == Some(&id),
            display: id.clone(),
            id,
        })
        .collect())
}

/// One side's value of an `install_profile.json` `data` entry.
#[derive(Clone, Debug, Deserialize)]
pub struct DataEntry {
    pub client: String,
}

/// A processor of an `install_profile.json`.
#[derive(Clone, Debug, Deserialize)]
pub struct Processor {
    /// The Maven coordinate of the processor's jar.
    pub jar: String,
    #[serde(default)]
    pub classpath: Vec<String>,
    #[serde(default)]
    pub args: Vec<String>,
    /// Files it makes and their SHA-1s, both with placeholders.
    #[serde(default)]
    pub outputs: BTreeMap<String, String>,
    /// `["client"]`, `["server"]`, or missing for both.
    #[serde(default)]
    pub sides: Option<Vec<String>>,
}

impl Processor {
    pub fn runs_on_client(&self) -> bool {
        self.sides
            .as_ref()
            .is_none_or(|sides| sides.iter().any(|side| side == "client"))
    }
}

/// The parts of a modern `install_profile.json` an install needs.
#[derive(Clone, Debug, Deserialize)]
pub struct InstallProfile {
    #[serde(default)]
    pub spec: u32,
    #[serde(default)]
    pub minecraft: Option<String>,
    /// The entry with the profile, `/version.json`.
    #[serde(default)]
    pub json: Option<String>,
    #[serde(default)]
    pub data: BTreeMap<String, DataEntry>,
    #[serde(default)]
    pub processors: Vec<Processor>,
    /// The processors' jars and what they read.
    #[serde(default)]
    pub libraries: Vec<Library>,
}

impl InstallProfile {
    pub fn parse(bytes: &[u8]) -> Result<Self, LoaderError> {
        serde_json::from_slice(bytes).map_err(|err| {
            LoaderError::Invalid(format!(
                "the installer's install_profile.json is invalid: {err}"
            ))
        })
    }

    /// The processors the client runs.
    pub fn client_processors(&self) -> impl Iterator<Item = &Processor> {
        self.processors.iter().filter(|p| p.runs_on_client())
    }
}

/// Where an old installer puts its universal jar.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyInstall {
    /// The Maven coordinate of the Forge library, which is the universal jar.
    pub path: String,
    /// The universal jar's entry in the installer.
    pub file_path: String,
    pub minecraft: Option<String>,
}

/// What an installer holds.
#[derive(Clone, Debug)]
pub enum InstallerProfile {
    Modern {
        install_profile: InstallProfile,
        /// The raw file, kept for the processors and keyed by its SHA-1.
        install_profile_bytes: Vec<u8>,
        version: Value,
    },
    Legacy {
        install: LegacyInstall,
        version: Value,
    },
}

/// An installer jar, opened with zip 0.6.6.
pub struct InstallerJar {
    path: PathBuf,
    archive: zip::ZipArchive<std::io::BufReader<File>>,
}

impl InstallerJar {
    pub fn open(path: &Path) -> Result<Self, LoaderError> {
        let file = File::open(path).map_err(|err| {
            LoaderError::Invalid(format!("couldn't open {}: {err}", path.display()))
        })?;
        let archive = zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(|err| {
            LoaderError::Invalid(format!(
                "{} is not a valid installer: {err}",
                path.display()
            ))
        })?;
        Ok(Self {
            path: path.to_owned(),
            archive,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The entry `name` (a leading `/` is ignored), if there is one.
    pub fn read(&mut self, name: &str, limit: u64) -> Result<Option<Vec<u8>>, LoaderError> {
        let name = name.trim_start_matches('/');
        let mut entry = match self.archive.by_name(name) {
            Ok(entry) => entry,
            Err(zip::result::ZipError::FileNotFound) => return Ok(None),
            Err(err) => {
                return Err(LoaderError::Invalid(format!(
                    "couldn't read {name} from the installer: {err}"
                )));
            }
        };
        if entry.size() > limit {
            return Err(LoaderError::Invalid(format!(
                "{name} in the installer is too large"
            )));
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        (&mut entry)
            .take(limit)
            .read_to_end(&mut bytes)
            .map_err(|err| {
                LoaderError::Invalid(format!("couldn't read {name} from the installer: {err}"))
            })?;
        Ok(Some(bytes))
    }

    /// The files under `maven/`, as paths relative to the libraries directory, with
    /// their entry names. Unsafe paths are refused.
    pub fn maven_files(&self) -> Result<Vec<(String, PathBuf)>, LoaderError> {
        let mut files = Vec::new();
        for name in self.archive.file_names() {
            let Some(relative) = name.strip_prefix("maven/") else {
                continue;
            };
            if relative.is_empty() || name.ends_with('/') {
                continue;
            }
            let path = safe_relative_path(relative).map_err(|_| {
                LoaderError::Invalid(format!("the installer has an unsafe path {name:?}"))
            })?;
            files.push((name.to_string(), path));
        }
        files.sort();
        Ok(files)
    }

    /// Reads the profile.
    pub fn profile(&mut self) -> Result<InstallerProfile, LoaderError> {
        let bytes = self
            .read("install_profile.json", MAX_JSON_BYTES)?
            .ok_or_else(|| {
                LoaderError::Invalid("the installer has no install_profile.json".into())
            })?;
        let raw: Value = serde_json::from_slice(&bytes).map_err(|err| {
            LoaderError::Invalid(format!(
                "the installer's install_profile.json is invalid: {err}"
            ))
        })?;
        if let (Some(install), Some(version)) = (raw.get("install"), raw.get("versionInfo")) {
            let install: LegacyInstall =
                serde_json::from_value(install.clone()).map_err(|err| {
                    LoaderError::Invalid(format!(
                        "the installer's install section is invalid: {err}"
                    ))
                })?;
            return Ok(InstallerProfile::Legacy {
                install,
                version: version.clone(),
            });
        }
        let install_profile = InstallProfile::parse(&bytes)?;
        let json_name = install_profile
            .json
            .clone()
            .unwrap_or_else(|| "version.json".into());
        let version_bytes = self
            .read(&json_name, MAX_JSON_BYTES)?
            .ok_or_else(|| LoaderError::Invalid(format!("the installer has no {json_name}")))?;
        let version = serde_json::from_slice(&version_bytes).map_err(|err| {
            LoaderError::Invalid(format!("the installer's {json_name} is invalid: {err}"))
        })?;
        Ok(InstallerProfile::Modern {
            install_profile,
            install_profile_bytes: bytes,
            version,
        })
    }

    /// Writes entry `name` to `dest`, atomically.
    pub fn extract(&mut self, name: &str, dest: &Path) -> Result<(), LoaderError> {
        let name = name.trim_start_matches('/');
        let mut entry = self.archive.by_name(name).map_err(|err| {
            LoaderError::Invalid(format!("couldn't read {name} from the installer: {err}"))
        })?;
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes).map_err(|err| {
            LoaderError::Invalid(format!("couldn't read {name} from the installer: {err}"))
        })?;
        crate::storage::write_atomic(dest, &bytes).map_err(|err| {
            LoaderError::Invalid(format!("couldn't write {}: {err}", dest.display()))
        })
    }
}

fn invalid_profile(why: impl std::fmt::Display) -> LoaderError {
    LoaderError::Invalid(format!("the Forge profile is invalid: {why}"))
}

/// A modern `version.json` in the launcher's form: `id` set to `id`, every library
/// with a `downloads.artifact` path. Libraries without a URL are kept with an empty
/// one: they come from the installer's `maven/` folder or from the processors.
pub fn normalize_modern(version: &Value, id: &str) -> Result<Value, LoaderError> {
    let Value::Object(fields) = version else {
        return Err(invalid_profile("it is not a JSON object"));
    };
    if fields.get("mainClass").and_then(Value::as_str).is_none() {
        return Err(invalid_profile("it has no main class"));
    }
    let mut out = fields.clone();
    let libraries = fields
        .get("libraries")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut normalized = Vec::with_capacity(libraries.len());
    for mut library in libraries {
        let name = library
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_profile("a library has no name"))?
            .to_string();
        let path = maven_path(&name, None).map_err(invalid_profile)?;
        let artifact = library
            .pointer_mut("/downloads/artifact")
            .and_then(Value::as_object_mut);
        match artifact {
            Some(artifact) => {
                let path = artifact
                    .get("path")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or(path);
                safe_relative_path(&path).map_err(invalid_profile)?;
                artifact.insert("path".into(), path.into());
                if artifact.get("sha1").and_then(Value::as_str).is_none() {
                    artifact.insert("sha1".into(), "".into());
                }
            }
            None => {
                // Some early 1.13+ profiles list a library by name and repository.
                let url = library
                    .get("url")
                    .and_then(Value::as_str)
                    .map(|base| maven_url(base, &path))
                    .unwrap_or_default();
                safe_relative_path(&path).map_err(invalid_profile)?;
                library["downloads"] = json!({"artifact": {"path": path, "url": url, "sha1": ""}});
            }
        }
        normalized.push(library);
    }
    out.insert("id".into(), id.into());
    out.insert("libraries".into(), Value::Array(normalized));
    Ok(Value::Object(out))
}

/// An old profile's library: where its jar is, and the SHA-1s it may have.
struct LegacyLibrary {
    name: String,
    path: String,
    url: String,
    checksums: Vec<String>,
}

fn legacy_libraries(
    version: &Value,
    install: &LegacyInstall,
) -> Result<Vec<LegacyLibrary>, LoaderError> {
    let list = version
        .get("libraries")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_profile("it has no libraries"))?;
    let mut out = Vec::new();
    for library in list {
        // `clientreq: false` marks server-only libraries (jline).
        if library.get("clientreq").and_then(Value::as_bool) == Some(false) {
            continue;
        }
        let name = library
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_profile("a library has no name"))?;
        let path = maven_path(name, None).map_err(invalid_profile)?;
        safe_relative_path(&path).map_err(invalid_profile)?;
        let url = if name == install.path {
            String::new()
        } else {
            let base = library
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or(MOJANG_LIBRARIES);
            maven_url(base, &path)
        };
        let checksums = library
            .get("checksums")
            .and_then(Value::as_array)
            .map(|sums| {
                sums.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_ascii_lowercase)
                    .collect()
            })
            .unwrap_or_default();
        out.push(LegacyLibrary {
            name: name.to_string(),
            path,
            url,
            checksums,
        });
    }
    Ok(out)
}

/// The URLs of an old profile's jars, whose `.sha1` files are fetched before
/// [`normalize_legacy`]. Old profiles have no SHA-1 for most jars, and `checksums`
/// mixes the jar's with the `.pack.xz` file's.
pub fn legacy_downloads(
    version: &Value,
    install: &LegacyInstall,
) -> Result<Vec<String>, LoaderError> {
    Ok(legacy_libraries(version, install)?
        .into_iter()
        .filter(|library| !library.url.is_empty())
        .map(|library| library.url)
        .collect())
}

/// An old profile in Mojang's format, with `id` set to `id`. The SHA-1 of each jar is
/// its `.sha1` file's (in `hashes`, by URL), which must be one of the profile's
/// `checksums` when it lists some. The Forge library itself is the universal jar,
/// taken from the installer (`universal` is its contents).
pub fn normalize_legacy(
    version: &Value,
    install: &LegacyInstall,
    id: &str,
    hashes: &HashMap<String, String>,
    universal: &[u8],
) -> Result<Value, LoaderError> {
    let Value::Object(fields) = version else {
        return Err(invalid_profile("it is not a JSON object"));
    };
    if fields.get("mainClass").and_then(Value::as_str).is_none() {
        return Err(invalid_profile("it has no main class"));
    }
    let mut libraries = Vec::new();
    for library in legacy_libraries(version, install)? {
        let (sha1, size) = if library.url.is_empty() {
            (sha1_hex(universal), Some(universal.len() as u64))
        } else {
            let sha1 = hashes.get(&library.url).cloned().ok_or_else(|| {
                LoaderError::Invalid(format!("no SHA-1 is known for {}", library.name))
            })?;
            if !library.checksums.is_empty() && !library.checksums.contains(&sha1) {
                return Err(LoaderError::Invalid(format!(
                    "the SHA-1 of {} is not one the Forge profile lists",
                    library.name
                )));
            }
            (sha1, None)
        };
        let mut artifact = Map::new();
        artifact.insert("path".into(), library.path.into());
        artifact.insert("url".into(), library.url.into());
        artifact.insert("sha1".into(), sha1.into());
        if let Some(size) = size {
            artifact.insert("size".into(), size.into());
        }
        libraries.push(json!({"name": library.name, "downloads": {"artifact": artifact}}));
    }
    let mut out = fields.clone();
    out.insert("id".into(), id.into());
    out.insert("libraries".into(), Value::Array(libraries));
    // `jar` names the vanilla jar, which the merge uses anyway.
    out.remove("jar");
    Ok(Value::Object(out))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::game::{loaders::tests::loader_fixture, version::VersionJson};

    /// A Forge profile from the recorded installer JSONs, normalised.
    pub fn forge_profile(build: &str, game_version: &str) -> VersionJson {
        let id = crate::game::loaders::profile_id(
            &crate::game::loaders::LoaderChoice {
                kind: LoaderKind::Forge,
                version: build.into(),
            },
            game_version,
        )
        .unwrap();
        let profile_path = format!("forge/{build}/install_profile.json");
        let raw: Value = serde_json::from_slice(&loader_fixture(&profile_path)).unwrap();
        let normalized = match (raw.get("install"), raw.get("versionInfo")) {
            (Some(install), Some(version)) => {
                let install: LegacyInstall = serde_json::from_value(install.clone()).unwrap();
                let hashes = legacy_downloads(version, &install)
                    .unwrap()
                    .into_iter()
                    .map(|url| (url, "0".repeat(40)))
                    .collect();
                // The recorded checksums would not match the made-up hashes.
                let mut version = version.clone();
                for library in version["libraries"].as_array_mut().unwrap() {
                    library.as_object_mut().unwrap().remove("checksums");
                }
                normalize_legacy(&version, &install, &id, &hashes, b"universal jar").unwrap()
            }
            _ => {
                let version: Value =
                    serde_json::from_slice(&loader_fixture(&format!("forge/{build}/version.json")))
                        .unwrap();
                normalize_modern(&version, &id).unwrap()
            }
        };
        VersionJson::parse(&serde_json::to_vec(&normalized).unwrap()).unwrap()
    }

    pub fn neoforge_profile(version: &str) -> VersionJson {
        let raw: Value =
            serde_json::from_slice(&loader_fixture(&format!("neoforge/{version}/version.json")))
                .unwrap();
        let normalized = normalize_modern(&raw, &format!("neoforge-{version}")).unwrap();
        VersionJson::parse(&serde_json::to_vec(&normalized).unwrap()).unwrap()
    }

    #[test]
    fn forge_versions_newest_first_with_the_promotion() {
        let metadata = loader_fixture("forge/maven-metadata.json");
        let promotions = loader_fixture("forge/promotions_slim.json");
        let versions = parse_forge_versions(&metadata, &promotions, "1.20.1").unwrap();
        assert_eq!(versions[0].id, "1.20.1-47.4.28");
        assert_eq!(versions[0].display, "47.4.28");
        assert!(versions[0].recommended);
        assert_eq!(versions.iter().filter(|v| v.recommended).count(), 1);
        let old = parse_forge_versions(&metadata, &promotions, "1.8.9").unwrap();
        assert_eq!(old[0].id, "1.8.9-11.15.1.2318-1.8.9");
        assert_eq!(old[0].display, "11.15.1.2318");
        assert!(old[0].recommended);
        // 1.12.2's recommended build is not the newest.
        let versions = parse_forge_versions(&metadata, &promotions, "1.12.2").unwrap();
        assert_eq!(versions[0].display, "14.23.5.2864");
        assert!(!versions[0].recommended);
        assert!(
            versions
                .iter()
                .any(|v| v.display == "14.23.5.2859" && v.recommended)
        );
        assert!(
            parse_forge_versions(&metadata, &promotions, "3.0")
                .unwrap()
                .is_empty()
        );
        assert!(parse_forge_versions(&metadata, b"nonsense", "1.20.1").is_ok());
        assert!(parse_forge_versions(b"[]", &promotions, "1.20.1").is_err());
    }

    #[test]
    fn neoforge_game_versions() {
        for (neo, game) in [
            ("21.1.209", Some("1.21.1")),
            ("21.0.167", Some("1.21")),
            ("20.2.3-beta", Some("1.20.2")),
            ("20.4.80-beta", Some("1.20.4")),
            ("26.3.0.101", Some("26.3")),
            ("26.1.1.4-beta", Some("26.1.1")),
            ("0.25w14craftmine.3-beta", None),
            ("nonsense", None),
        ] {
            assert_eq!(neoforge_game_version(neo).as_deref(), game, "{neo}");
        }
        let versions =
            parse_neoforge_versions(&loader_fixture("neoforge/versions.json"), "1.21.1").unwrap();
        assert_eq!(versions[0].id, "21.1.209");
        assert!(versions[0].recommended && versions[0].stable);
        assert!(versions.iter().all(|v| v.id.starts_with("21.1.")));
        let new =
            parse_neoforge_versions(&loader_fixture("neoforge/versions.json"), "26.3").unwrap();
        assert_eq!(new[0].id, "26.3.0.101");
        assert!(new.iter().any(|v| !v.stable));
        let betas =
            parse_neoforge_versions(&loader_fixture("neoforge/versions.json"), "1.20.2").unwrap();
        assert!(betas.iter().all(|v| !v.stable && !v.recommended));
    }

    #[test]
    fn installer_urls() {
        assert_eq!(
            installer_url(LoaderKind::Forge, "1.20.1-47.4.28").unwrap(),
            "https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.4.28/forge-1.20.1-47.4.28-installer.jar"
        );
        assert_eq!(
            installer_url(LoaderKind::NeoForge, "21.1.209").unwrap(),
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.209/neoforge-21.1.209-installer.jar"
        );
    }

    #[test]
    fn modern_install_profiles() {
        let profile =
            InstallProfile::parse(&loader_fixture("forge/1.20.1-47.4.28/install_profile.json"))
                .unwrap();
        assert_eq!(profile.minecraft.as_deref(), Some("1.20.1"));
        let client: Vec<_> = profile
            .client_processors()
            .map(|p| p.jar.as_str())
            .collect();
        assert_eq!(
            client,
            [
                "net.minecraftforge:installertools:1.4.1",
                "net.minecraftforge:installertools:1.4.1",
                "net.minecraftforge:installertools:1.4.1",
                "net.minecraftforge:jarsplitter:1.1.4",
                "net.minecraftforge:ForgeAutoRenamingTool:0.1.22:all",
                "net.minecraftforge:binarypatcher:1.1.1",
            ]
        );
        assert_eq!(
            profile.data["PATCHED"].client,
            "[net.minecraftforge:forge:1.20.1-47.4.28:client]"
        );
        let neo = InstallProfile::parse(&loader_fixture("neoforge/21.1.209/install_profile.json"))
            .unwrap();
        assert_eq!(neo.client_processors().count(), 6);
        // 1.12.2's newer installers have no processors.
        let old = InstallProfile::parse(&loader_fixture(
            "forge/1.12.2-14.23.5.2864/install_profile.json",
        ))
        .unwrap();
        assert_eq!(old.client_processors().count(), 0);
    }

    #[test]
    fn modern_profiles_keep_local_libraries_without_urls() {
        let forge = forge_profile("1.21.1-52.1.21", "1.21.1");
        let client = forge
            .libraries
            .iter()
            .find(|l| l.name == "net.minecraftforge:forge:1.21.1-52.1.21:client")
            .unwrap();
        let artifact = client.downloads.artifact.as_ref().unwrap();
        assert!(artifact.url.is_empty());
        assert_eq!(artifact.sha1, "28f7acaa0e9b1c2d2a7bf52c07ae16133bd3da4e");
        let forge = forge_profile("1.20.1-47.4.28", "1.20.1");
        assert_eq!(forge.id, "1.20.1-forge-47.4.28");
        assert_eq!(forge.inherits_from.as_deref(), Some("1.20.1"));
        assert_eq!(
            forge.main_class,
            "cpw.mods.bootstraplauncher.BootstrapLauncher"
        );
        assert!(forge.client_logging().is_none(), "Forge's logging is empty");
        let neo = neoforge_profile("21.1.209");
        assert_eq!(neo.id, "neoforge-21.1.209");
    }

    #[test]
    fn legacy_profiles() {
        for (build, game, tweaker) in [
            (
                "1.8.9-11.15.1.2318-1.8.9",
                "1.8.9",
                "net.minecraftforge.fml.common.launcher.FMLTweaker",
            ),
            (
                "1.7.10-10.13.4.1614-1.7.10",
                "1.7.10",
                "cpw.mods.fml.common.launcher.FMLTweaker",
            ),
        ] {
            let raw: Value = serde_json::from_slice(&loader_fixture(&format!(
                "forge/{build}/install_profile.json"
            )))
            .unwrap();
            let install: LegacyInstall = serde_json::from_value(raw["install"].clone()).unwrap();
            assert_eq!(install.path, format!("net.minecraftforge:forge:{build}"));
            let urls = legacy_downloads(&raw["versionInfo"], &install).unwrap();
            assert!(urls.contains(&"https://libraries.minecraft.net/net/minecraft/launchwrapper/1.12/launchwrapper-1.12.jar".to_string()));
            assert!(urls.contains(&"https://maven.minecraftforge.net/com/typesafe/akka/akka-actor_2.11/2.3.3/akka-actor_2.11-2.3.3.jar".to_string()));
            // jline is server-only, and the Forge jar comes from the installer.
            assert!(
                !urls
                    .iter()
                    .any(|u| u.contains("jline") || u.contains("/forge/"))
            );
            let version = forge_profile(build, game);
            assert!(
                version
                    .minecraft_arguments
                    .as_deref()
                    .unwrap()
                    .contains(tweaker)
            );
            let forge = &version.libraries[0];
            assert_eq!(forge.name, install.path);
            let artifact = forge.downloads.artifact.as_ref().unwrap();
            assert!(artifact.url.is_empty());
            assert_eq!(artifact.sha1, sha1_hex(b"universal jar"));
        }

        // A `.sha1` file that matches none of the listed checksums is refused.
        let raw: Value = serde_json::from_slice(&loader_fixture(
            "forge/1.8.9-11.15.1.2318-1.8.9/install_profile.json",
        ))
        .unwrap();
        let install: LegacyInstall = serde_json::from_value(raw["install"].clone()).unwrap();
        let mut hashes: HashMap<String, String> = legacy_downloads(&raw["versionInfo"], &install)
            .unwrap()
            .into_iter()
            .map(|url| (url, "0".repeat(40)))
            .collect();
        let err = normalize_legacy(&raw["versionInfo"], &install, "x", &hashes, b"u").unwrap_err();
        assert!(err.to_string().contains("akka-actor"), "{err}");
        // With the real ones it passes.
        for (url, sha1) in [
            (
                "https://maven.minecraftforge.net/com/typesafe/akka/akka-actor_2.11/2.3.3/akka-actor_2.11-2.3.3.jar",
                "ed62e9fc709ca0f2ff1a3220daa8b70a2870078e",
            ),
            (
                "https://maven.minecraftforge.net/com/typesafe/config/1.2.1/config-1.2.1.jar",
                "f771f71fdae3df231bcd54d5ca2d57f0bf93f467",
            ),
            (
                "https://maven.minecraftforge.net/org/scala-lang/scala-actors-migration_2.11/1.1.0/scala-actors-migration_2.11-1.1.0.jar",
                "dfa8bc42b181d5b9f1a5dd147f8ae308b893eb6f",
            ),
            (
                "https://maven.minecraftforge.net/org/scala-lang/scala-compiler/2.11.1/scala-compiler-2.11.1.jar",
                "56ea2e6c025e0821f28d73ca271218b8dd04926a",
            ),
            (
                "https://maven.minecraftforge.net/org/scala-lang/scala-library/2.11.1/scala-library-2.11.1.jar",
                "0e11da23da3eabab9f4777b9220e60d44c1aab6a",
            ),
            (
                "https://maven.minecraftforge.net/org/scala-lang/scala-parser-combinators_2.11/1.0.1/scala-parser-combinators_2.11-1.0.1.jar",
                "f05d7345bf5a58924f2837c6c1f4d73a938e1ff0",
            ),
            (
                "https://maven.minecraftforge.net/org/scala-lang/scala-reflect/2.11.1/scala-reflect-2.11.1.jar",
                "6580347e61cc7f8e802941e7fde40fa83b8badeb",
            ),
            (
                "https://maven.minecraftforge.net/org/scala-lang/scala-swing_2.11/1.0.1/scala-swing_2.11-1.0.1.jar",
                "b1cdd92bd47b1e1837139c1c53020e86bb9112ae",
            ),
            (
                "https://maven.minecraftforge.net/org/scala-lang/scala-xml_2.11/1.0.2/scala-xml_2.11-1.0.2.jar",
                "7a80ec00aec122fba7cd4e0d4cdd87ff7e4cb6d0",
            ),
        ] {
            hashes.insert(url.into(), sha1.into());
        }
        let normalized =
            normalize_legacy(&raw["versionInfo"], &install, "x", &hashes, b"u").unwrap();
        assert!(normalized.get("jar").is_none());
        assert_eq!(normalized["id"], "x");
    }

    #[test]
    fn rejects_unsafe_modern_paths() {
        let version = json!({
            "mainClass": "x",
            "libraries": [{"name": "a:b:1", "downloads": {"artifact": {"path": "../../x.jar", "url": "", "sha1": ""}}}],
        });
        assert!(normalize_modern(&version, "x").is_err());
    }
}
