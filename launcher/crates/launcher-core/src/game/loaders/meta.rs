//! Fabric's (<https://meta.fabricmc.net>) and Quilt's (<https://meta.quiltmc.org>)
//! meta servers: the loader versions for a Minecraft version, and each loader's
//! profile.
//!
//! A profile's libraries are `{name, url}`, where `url` is the Maven repository. Fabric
//! adds `sha1` and `size` to most; Quilt to none. The missing hashes come from the
//! `.sha1` file Maven keeps next to each jar, fetched once when the profile is
//! installed.

use std::collections::HashMap;

use lodestone_minecraft::versions::fabric::{FabricLoaderEntry, select_fabric_loader};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::{LoaderError, LoaderKind, LoaderVersion, maven_url};
use crate::game::libraries::{maven_path, safe_relative_path};

pub const FABRIC_META: &str = "https://meta.fabricmc.net";
pub const QUILT_META: &str = "https://meta.quiltmc.org";

/// The version list of `kind` (Fabric or Quilt) for `game_version`.
pub fn versions_url(kind: LoaderKind, game_version: &str) -> String {
    match kind {
        LoaderKind::Quilt => format!("{QUILT_META}/v3/versions/loader/{game_version}"),
        _ => format!("{FABRIC_META}/v2/versions/loader/{game_version}"),
    }
}

/// The profile of a loader version.
pub fn profile_url(kind: LoaderKind, game_version: &str, loader_version: &str) -> String {
    match kind {
        LoaderKind::Quilt => {
            format!("{QUILT_META}/v3/versions/loader/{game_version}/{loader_version}/profile/json")
        }
        _ => {
            format!("{FABRIC_META}/v2/versions/loader/{game_version}/{loader_version}/profile/json")
        }
    }
}

/// Fabric's loader versions, newest first. The recommended one is the newest whose
/// loader and intermediary are stable, as Lodestone Core picks it for servers.
pub fn parse_fabric_versions(
    bytes: &[u8],
    game_version: &str,
) -> Result<Vec<LoaderVersion>, LoaderError> {
    let entries: Vec<FabricLoaderEntry> = serde_json::from_slice(bytes)
        .map_err(|err| LoaderError::Invalid(format!("Fabric's version list is invalid: {err}")))?;
    let recommended = select_fabric_loader(&entries, game_version).ok();
    Ok(entries
        .iter()
        .map(|entry| LoaderVersion {
            id: entry.loader.version.clone(),
            display: entry.loader.version.clone(),
            stable: entry.loader.stable,
            recommended: recommended.as_deref() == Some(entry.loader.version.as_str()),
        })
        .collect())
}

#[derive(Deserialize)]
struct QuiltEntry {
    loader: QuiltLoader,
}

#[derive(Deserialize)]
struct QuiltLoader {
    version: String,
}

/// Quilt's loader versions, newest first. Quilt marks pre-releases in the version
/// (`0.30.0-beta.3`); the recommended one is the newest without.
pub fn parse_quilt_versions(bytes: &[u8]) -> Result<Vec<LoaderVersion>, LoaderError> {
    let entries: Vec<QuiltEntry> = serde_json::from_slice(bytes)
        .map_err(|err| LoaderError::Invalid(format!("Quilt's version list is invalid: {err}")))?;
    let recommended = entries
        .iter()
        .map(|entry| &entry.loader.version)
        .find(|version| !version.contains('-'))
        .cloned();
    Ok(entries
        .into_iter()
        .map(|entry| {
            let version = entry.loader.version;
            LoaderVersion {
                stable: !version.contains('-'),
                recommended: recommended.as_ref() == Some(&version),
                display: version.clone(),
                id: version,
            }
        })
        .collect())
}

/// A library of a meta profile.
struct MetaLibrary<'a> {
    name: &'a str,
    path: String,
    url: String,
    sha1: Option<String>,
    size: Option<u64>,
}

fn libraries(profile: &Value) -> Result<Vec<MetaLibrary<'_>>, LoaderError> {
    let invalid =
        |why: String| LoaderError::Invalid(format!("the loader profile is invalid: {why}"));
    let list = profile
        .get("libraries")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("it has no libraries".into()))?;
    list.iter()
        .map(|library| {
            let name = library
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("a library has no name".into()))?;
            let base = library
                .get("url")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid(format!("{name} has no repository")))?;
            let path = maven_path(name, None).map_err(|err| invalid(err.to_string()))?;
            safe_relative_path(&path).map_err(|err| invalid(err.to_string()))?;
            Ok(MetaLibrary {
                name,
                url: maven_url(base, &path),
                path,
                sha1: library
                    .get("sha1")
                    .and_then(Value::as_str)
                    .map(str::to_ascii_lowercase),
                size: library.get("size").and_then(Value::as_u64),
            })
        })
        .collect()
}

/// The jars of `profile` whose SHA-1 the profile does not give, as their URLs. Their
/// `.sha1` files are fetched before [`normalize`].
pub fn missing_hashes(profile: &Value) -> Result<Vec<String>, LoaderError> {
    Ok(libraries(profile)?
        .into_iter()
        .filter(|library| library.sha1.is_none())
        .map(|library| library.url)
        .collect())
}

/// `profile` in Mojang's format: `id` set to `id`, and every library with a
/// `downloads.artifact`, its SHA-1 from the profile or `hashes` (by URL).
pub fn normalize(
    profile: &Value,
    id: &str,
    hashes: &HashMap<String, String>,
) -> Result<Value, LoaderError> {
    let Value::Object(fields) = profile else {
        return Err(LoaderError::Invalid(
            "the loader profile is not a JSON object".into(),
        ));
    };
    if fields.get("mainClass").and_then(Value::as_str).is_none() {
        return Err(LoaderError::Invalid(
            "the loader profile has no main class".into(),
        ));
    }
    let mut libraries_out = Vec::new();
    for library in libraries(profile)? {
        let sha1 = library
            .sha1
            .or_else(|| hashes.get(&library.url).cloned())
            .ok_or_else(|| {
                LoaderError::Invalid(format!("no SHA-1 is known for {}", library.name))
            })?;
        let mut artifact = Map::new();
        artifact.insert("path".into(), library.path.into());
        artifact.insert("url".into(), library.url.into());
        artifact.insert("sha1".into(), sha1.into());
        if let Some(size) = library.size {
            artifact.insert("size".into(), size.into());
        }
        libraries_out.push(json!({
            "name": library.name,
            "downloads": {"artifact": artifact},
        }));
    }
    let mut out = fields.clone();
    out.insert("id".into(), id.into());
    out.insert("libraries".into(), Value::Array(libraries_out));
    Ok(Value::Object(out))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::game::{loaders::tests::loader_fixture, version::VersionJson};

    /// Fabric 0.16.14 for 1.20.1, normalised with made-up hashes for the jars the
    /// profile has none for.
    pub fn fabric_profile() -> VersionJson {
        profile(
            LoaderKind::Fabric,
            "fabric/profile-0.16.14-1.20.1.json",
            "fabric-loader-0.16.14-1.20.1",
        )
    }

    pub fn quilt_profile() -> VersionJson {
        profile(
            LoaderKind::Quilt,
            "quilt/profile-0.29.2-1.20.1.json",
            "quilt-loader-0.29.2-1.20.1",
        )
    }

    fn profile(_kind: LoaderKind, path: &str, id: &str) -> VersionJson {
        let value: Value = serde_json::from_slice(&loader_fixture(path)).unwrap();
        let hashes = missing_hashes(&value)
            .unwrap()
            .into_iter()
            .map(|url| (url, "0".repeat(40)))
            .collect();
        let normalized = normalize(&value, id, &hashes).unwrap();
        VersionJson::parse(&serde_json::to_vec(&normalized).unwrap()).unwrap()
    }

    #[test]
    fn fabric_versions_and_the_recommended_one() {
        let versions =
            parse_fabric_versions(&loader_fixture("fabric/loaders-1.20.1.json"), "1.20.1").unwrap();
        assert_eq!(versions[0].id, "0.20.1");
        assert!(versions[0].recommended);
        assert_eq!(versions.iter().filter(|v| v.recommended).count(), 1);
        assert!(versions.iter().any(|v| v.id == "0.16.14" && !v.stable));
        assert!(versions.iter().any(|v| !v.stable));
        assert!(parse_fabric_versions(b"{}", "1.20.1").is_err());
    }

    #[test]
    fn quilt_versions_skip_betas_for_the_recommendation() {
        let versions = parse_quilt_versions(&loader_fixture("quilt/loaders-1.20.1.json")).unwrap();
        let recommended: Vec<_> = versions.iter().filter(|v| v.recommended).collect();
        assert_eq!(recommended.len(), 1);
        assert!(!recommended[0].id.contains('-'));
        assert!(versions.iter().any(|v| v.id == "0.29.2"));
        for version in &versions {
            assert_eq!(version.stable, !version.id.contains('-'));
        }
    }

    #[test]
    fn fabric_profile_hashes_come_from_the_profile_or_sha1_files() {
        let value: Value =
            serde_json::from_slice(&loader_fixture("fabric/profile-0.16.14-1.20.1.json")).unwrap();
        // Fabric gives hashes for everything except intermediary and the loader.
        assert_eq!(
            missing_hashes(&value).unwrap(),
            [
                "https://maven.fabricmc.net/net/fabricmc/intermediary/1.20.1/intermediary-1.20.1.jar",
                "https://maven.fabricmc.net/net/fabricmc/fabric-loader/0.16.14/fabric-loader-0.16.14.jar",
            ]
        );
        assert!(normalize(&value, "x", &HashMap::new()).is_err());
        let version = fabric_profile();
        assert_eq!(version.id, "fabric-loader-0.16.14-1.20.1");
        assert_eq!(version.inherits_from.as_deref(), Some("1.20.1"));
        let asm = &version.libraries[0];
        assert_eq!(asm.name, "org.ow2.asm:asm:9.8");
        let artifact = asm.downloads.artifact.as_ref().unwrap();
        assert_eq!(
            artifact.url,
            "https://maven.fabricmc.net/org/ow2/asm/asm/9.8/asm-9.8.jar"
        );
        assert_eq!(artifact.sha1, "dc19ecb3f7889b7860697215cae99c0f9b6f6b4b");
        assert_eq!(artifact.size, Some(126113));
        let loader = version.libraries.last().unwrap();
        assert_eq!(loader.downloads.artifact.as_ref().unwrap().size, None);
    }

    #[test]
    fn quilt_needs_every_hash() {
        let value: Value =
            serde_json::from_slice(&loader_fixture("quilt/profile-0.29.2-1.20.1.json")).unwrap();
        let missing = missing_hashes(&value).unwrap();
        assert_eq!(missing.len(), 11);
        assert!(missing.contains(
            &"https://maven.quiltmc.org/repository/release/org/quiltmc/quilt-loader/0.29.2/quilt-loader-0.29.2.jar".to_string()
        ));
        let version = quilt_profile();
        assert_eq!(
            version.main_class,
            "org.quiltmc.loader.impl.launch.knot.KnotClient"
        );
    }

    #[test]
    fn rejects_unsafe_library_names() {
        let value = json!({
            "mainClass": "x",
            "libraries": [{"name": "../../evil:x:1", "url": "https://maven.fabricmc.net/"}],
        });
        assert!(missing_hashes(&value).is_err());
        let value = json!({"libraries": []});
        assert!(normalize(&value, "x", &HashMap::new()).is_err());
    }
}
