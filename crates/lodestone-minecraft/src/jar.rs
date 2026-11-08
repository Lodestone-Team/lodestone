//! Reads the metadata file inside a mod or plugin jar: `fabric.mod.json`, `quilt.mod.json`,
//! `META-INF/mods.toml`, `META-INF/neoforge.mods.toml`, `mcmod.info`, `paper-plugin.yml` or
//! `plugin.yml`.
//!
//! The `parse_*` functions take the text of a metadata file. Reading it from a jar needs
//! the `zip` feature.

#[cfg(feature = "zip")]
use std::io::{Read, Seek};
#[cfg(feature = "zip")]
use std::path::Path;

use serde::{Deserialize, Serialize};

#[cfg(feature = "zip")]
use crate::Error;

// No doc comments on the types below: they would change Lodestone Core's TypeScript
// bindings.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum InstalledModKind {
    Mod,
    Plugin,
}

// The loader a jar is for. NeoForge mods count as Forge mods, and Quilt mods as Fabric mods.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum InstalledModLoader {
    Fabric,
    Forge,
    Paper,
    Spigot,
}

impl InstalledModLoader {
    pub fn as_str(&self) -> &'static str {
        match self {
            InstalledModLoader::Fabric => "fabric",
            InstalledModLoader::Forge => "forge",
            InstalledModLoader::Paper => "paper",
            InstalledModLoader::Spigot => "spigot",
        }
    }

    pub fn kind(&self) -> InstalledModKind {
        match self {
            InstalledModLoader::Fabric | InstalledModLoader::Forge => InstalledModKind::Mod,
            InstalledModLoader::Paper | InstalledModLoader::Spigot => InstalledModKind::Plugin,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct ModFileMetadata {
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
}

/// Metadata files larger than this are ignored.
#[cfg(feature = "zip")]
const MAX_METADATA_BYTES: u64 = 1024 * 1024;

/// The metadata files a loader reads, in the order they are tried.
pub fn metadata_files(loader: InstalledModLoader) -> &'static [&'static str] {
    match loader {
        InstalledModLoader::Fabric => &["fabric.mod.json", "quilt.mod.json"],
        InstalledModLoader::Forge => &[
            "META-INF/mods.toml",
            "META-INF/neoforge.mods.toml",
            "mcmod.info",
        ],
        InstalledModLoader::Paper => &["paper-plugin.yml", "plugin.yml"],
        InstalledModLoader::Spigot => &["plugin.yml"],
    }
}

#[cfg(feature = "zip")]
fn read_entry<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, name: &str) -> Option<String> {
    let entry = archive.by_name(name).ok()?;
    if !entry.is_file() || entry.size() > MAX_METADATA_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    entry
        .take(MAX_METADATA_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    let text = String::from_utf8_lossy(&bytes);
    Some(text.trim_start_matches('\u{feff}').to_string())
}

#[cfg(feature = "zip")]
fn not_a_jar() -> Error {
    Error::Invalid("The file is not a valid jar (zip) file".to_string())
}

/// Opens `path` as a zip archive. A file that is not a zip is [`Error::Invalid`].
#[cfg(feature = "zip")]
fn open_jar(path: &Path) -> Result<zip::ZipArchive<std::fs::File>, Error> {
    let file = std::fs::File::open(path)
        .map_err(Error::io(format!("Failed to open {}", path.display())))?;
    zip::ZipArchive::new(file).map_err(|_| not_a_jar())
}

/// Whether the jar at `path` contains one of the loader's metadata files. Fails if it is
/// not a zip file.
#[cfg(feature = "zip")]
pub fn jar_has_metadata(path: &Path, loader: InstalledModLoader) -> Result<bool, Error> {
    let archive = open_jar(path)?;
    let names: Vec<String> = archive
        .file_names()
        .filter_map(|n| n.ok().map(|n| n.to_string()))
        .collect();
    Ok(metadata_files(loader)
        .iter()
        .any(|f| names.iter().any(|n| n == f)))
}

/// Where a mod says it runs, from `fabric.mod.json` `environment` or the Quilt equivalent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JarEnvironment {
    /// Only on the client.
    Client,
    /// Only on the (dedicated) server.
    Server,
}

/// What Lodestone reads from a jar: the metadata shown to users, plus the environment and
/// dependencies that decide whether players need the mod. Only Fabric and Quilt mods have
/// the last two.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct JarInfo {
    pub metadata: Option<ModFileMetadata>,
    pub environment: Option<JarEnvironment>,
    /// Ids of the mods this one depends on.
    pub depends: Vec<String>,
}

/// What Lodestone reads from the jar at `path`, from the first metadata file of the loader
/// that can be read. Empty if the jar has none or is not a zip file.
#[cfg(feature = "zip")]
pub fn read_jar_info(path: &Path, loader: InstalledModLoader) -> JarInfo {
    match open_jar(path) {
        Ok(mut archive) => read_archive_info(&mut archive, loader),
        Err(_) => JarInfo::default(),
    }
}

/// What Lodestone reads from a jar that is already open, see [`read_jar_info`].
#[cfg(feature = "zip")]
pub fn read_archive_info<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    loader: InstalledModLoader,
) -> JarInfo {
    for name in metadata_files(loader) {
        let Some(text) = read_entry(archive, name) else {
            continue;
        };
        let (parsed, environment, depends) = match *name {
            "fabric.mod.json" => {
                let (environment, depends) = fabric_environment_and_depends(&text);
                (parse_fabric_mod_json(&text), environment, depends)
            }
            "quilt.mod.json" => {
                let (environment, depends) = quilt_environment_and_depends(&text);
                (parse_quilt_mod_json(&text), environment, depends)
            }
            "META-INF/mods.toml" | "META-INF/neoforge.mods.toml" => {
                let manifest_version = read_entry(archive, "META-INF/MANIFEST.MF")
                    .and_then(|m| manifest_attribute(&m, "Implementation-Version"));
                (parse_mods_toml(&text, manifest_version), None, Vec::new())
            }
            "mcmod.info" => (parse_mcmod_info(&text), None, Vec::new()),
            _ => (Some(parse_plugin_yml(&text)), None, Vec::new()),
        };
        if let Some(metadata) = parsed {
            return JarInfo {
                metadata: Some(metadata),
                environment,
                depends,
            };
        }
    }
    JarInfo::default()
}

/// `environment` and the `depends` keys of a `fabric.mod.json`.
pub fn fabric_environment_and_depends(text: &str) -> (Option<JarEnvironment>, Vec<String>) {
    #[derive(Deserialize)]
    struct FabricModJson {
        #[serde(default)]
        environment: Option<String>,
        #[serde(default)]
        depends: serde_json::Map<String, serde_json::Value>,
    }
    let Some(parsed) = parse_json_leniently::<FabricModJson>(text) else {
        return (None, Vec::new());
    };
    let environment = match parsed.environment.as_deref().map(str::trim) {
        Some("client") => Some(JarEnvironment::Client),
        Some("server") => Some(JarEnvironment::Server),
        _ => None,
    };
    (
        environment,
        clean_list(parsed.depends.into_iter().map(|(id, _)| id)),
    )
}

/// `minecraft.environment` and the `quilt_loader.depends` ids of a `quilt.mod.json`.
/// A dependency is a string, or an object with an `id`; `group:id` is reduced to the id.
pub fn quilt_environment_and_depends(text: &str) -> (Option<JarEnvironment>, Vec<String>) {
    #[derive(Deserialize, Default)]
    struct QuiltMinecraft {
        #[serde(default)]
        environment: Option<String>,
    }
    #[derive(Deserialize, Default)]
    struct QuiltLoader {
        #[serde(default)]
        depends: Vec<serde_json::Value>,
    }
    #[derive(Deserialize)]
    struct QuiltModJson {
        #[serde(default)]
        quilt_loader: QuiltLoader,
        #[serde(default)]
        minecraft: QuiltMinecraft,
    }
    let Some(parsed) = parse_json_leniently::<QuiltModJson>(text) else {
        return (None, Vec::new());
    };
    let environment = match parsed.minecraft.environment.as_deref().map(str::trim) {
        Some("client") => Some(JarEnvironment::Client),
        Some("dedicated_server") => Some(JarEnvironment::Server),
        _ => None,
    };
    let depends = parsed
        .quilt_loader
        .depends
        .iter()
        .filter_map(|d| match d {
            serde_json::Value::String(id) => Some(id.clone()),
            serde_json::Value::Object(o) => o.get("id").and_then(|i| i.as_str()).map(String::from),
            _ => None,
        })
        .map(|id| id.rsplit(':').next().unwrap_or_default().to_string());
    (environment, clean_list(depends))
}

fn clean(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn clean_list(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

/// The name of a person in `fabric.mod.json`: a string, or an object with a `name`.
fn fabric_person_name(person: &serde_json::Value) -> Option<String> {
    match person {
        serde_json::Value::String(name) => Some(name.clone()),
        serde_json::Value::Object(object) => object
            .get("name")
            .and_then(|n| n.as_str())
            .map(String::from),
        _ => None,
    }
}

fn parse_json_leniently<T: serde::de::DeserializeOwned>(text: &str) -> Option<T> {
    serde_json::from_str(text).ok().or_else(|| {
        // Fabric's own parser accepts raw line breaks and tabs inside strings
        let cleaned: String = text
            .chars()
            .map(|c| {
                if matches!(c, '\n' | '\r' | '\t') {
                    ' '
                } else {
                    c
                }
            })
            .collect();
        serde_json::from_str(&cleaned).ok()
    })
}

pub fn parse_fabric_mod_json(text: &str) -> Option<ModFileMetadata> {
    #[derive(Deserialize)]
    struct FabricModJson {
        id: Option<String>,
        name: Option<String>,
        version: Option<String>,
        description: Option<String>,
        #[serde(default)]
        authors: Vec<serde_json::Value>,
    }
    let parsed: FabricModJson = parse_json_leniently(text)?;
    Some(ModFileMetadata {
        id: clean(parsed.id),
        name: clean(parsed.name),
        version: clean(parsed.version),
        description: clean(parsed.description),
        authors: clean_list(parsed.authors.iter().filter_map(fabric_person_name)),
    })
}

pub fn parse_quilt_mod_json(text: &str) -> Option<ModFileMetadata> {
    #[derive(Deserialize, Default)]
    struct QuiltMetadata {
        name: Option<String>,
        description: Option<String>,
        #[serde(default)]
        contributors: serde_json::Map<String, serde_json::Value>,
    }
    #[derive(Deserialize)]
    struct QuiltLoader {
        id: Option<String>,
        version: Option<String>,
        #[serde(default)]
        metadata: QuiltMetadata,
    }
    #[derive(Deserialize)]
    struct QuiltModJson {
        quilt_loader: QuiltLoader,
    }
    let parsed: QuiltModJson = parse_json_leniently(text)?;
    let loader = parsed.quilt_loader;
    Some(ModFileMetadata {
        id: clean(loader.id),
        name: clean(loader.metadata.name),
        version: clean(loader.version),
        description: clean(loader.metadata.description),
        authors: clean_list(loader.metadata.contributors.into_iter().map(|(k, _)| k)),
    })
}

/// The value of `key` in a jar's `META-INF/MANIFEST.MF`.
pub fn manifest_attribute(manifest: &str, key: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let (k, v) = line.split_once(':')?;
        (k.trim() == key).then(|| v.trim().to_string())
    })
}

pub fn parse_mods_toml(text: &str, manifest_version: Option<String>) -> Option<ModFileMetadata> {
    let table: toml::Table = toml::from_str(text).ok()?;
    let first = table.get("mods")?.as_array()?.first()?.as_table()?;
    let string = |key: &str| first.get(key).and_then(|v| v.as_str()).map(String::from);
    let version = clean(string("version")).and_then(|v| {
        if v.contains("${") {
            // "${file.jarVersion}" is filled in from the manifest at build time
            clean(manifest_version.clone())
        } else {
            Some(v)
        }
    });
    let authors = match first.get("authors") {
        Some(toml::Value::String(s)) => clean_list(s.split(',').map(String::from)),
        Some(toml::Value::Array(a)) => {
            clean_list(a.iter().filter_map(|v| v.as_str()).map(String::from))
        }
        _ => Vec::new(),
    };
    Some(ModFileMetadata {
        id: clean(string("modId")),
        name: clean(string("displayName")),
        version,
        description: clean(string("description")),
        authors,
    })
}

pub fn parse_mcmod_info(text: &str) -> Option<ModFileMetadata> {
    #[derive(Deserialize)]
    struct McmodEntry {
        modid: Option<String>,
        name: Option<String>,
        version: Option<String>,
        description: Option<String>,
        #[serde(default, alias = "authors")]
        #[serde(rename = "authorList")]
        author_list: Vec<String>,
    }
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum McmodInfo {
        List(Vec<McmodEntry>),
        Wrapped {
            #[serde(rename = "modList")]
            mod_list: Vec<McmodEntry>,
        },
    }
    let entries = match parse_json_leniently::<McmodInfo>(text)? {
        McmodInfo::List(list) => list,
        McmodInfo::Wrapped { mod_list } => mod_list,
    };
    let entry = entries.into_iter().next()?;
    Some(ModFileMetadata {
        id: clean(entry.modid),
        name: clean(entry.name),
        version: clean(entry.version),
        description: clean(entry.description),
        authors: clean_list(entry.author_list),
    })
}

/// Removes matching quotes around a YAML scalar, and a trailing comment from an unquoted one.
fn yaml_scalar(value: &str) -> String {
    let value = value.trim();
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            return value[1..value.len() - 1].to_string();
        }
    }
    match value.find(" #") {
        Some(i) => value[..i].trim_end().to_string(),
        None => value.to_string(),
    }
}

/// `[a, "b"]` as a list.
fn yaml_inline_list(value: &str) -> Vec<String> {
    value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(yaml_scalar)
        .collect()
}

/// Reads the top-level `name`, `version`, `description`, `author` and `authors` keys of a
/// `plugin.yml` or `paper-plugin.yml`. Only simple YAML is understood: scalars, inline
/// lists, block lists and block scalars (`|` and `>`).
pub fn parse_plugin_yml(text: &str) -> ModFileMetadata {
    let mut name = None;
    let mut version = None;
    let mut description = None;
    let mut authors = Vec::new();

    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        if line.is_empty()
            || line.starts_with(char::is_whitespace)
            || line.starts_with('#')
            || line.starts_with('-')
        {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if !matches!(
            key,
            "name" | "version" | "description" | "author" | "authors"
        ) {
            continue;
        }
        let value = value.trim();
        // the indented lines that belong to this key
        let mut block = Vec::new();
        while i < lines.len()
            && (lines[i].trim().is_empty() || lines[i].starts_with(char::is_whitespace))
        {
            block.push(lines[i]);
            i += 1;
        }
        let parsed: Vec<String> = if value.starts_with('[') {
            yaml_inline_list(value)
        } else if value.is_empty() {
            // a block list
            block
                .iter()
                .filter_map(|l| l.trim().strip_prefix('-').map(yaml_scalar))
                .collect()
        } else if value.starts_with('|') || value.starts_with('>') {
            let separator = if value.starts_with('|') { "\n" } else { " " };
            vec![block
                .iter()
                .map(|l| l.trim())
                .collect::<Vec<_>>()
                .join(separator)]
        } else {
            vec![yaml_scalar(value)]
        };
        let first = parsed.first().cloned();
        match key {
            "name" => name = clean(first),
            "version" => version = clean(first),
            "description" => description = clean(first),
            _ => authors.extend(clean_list(parsed)),
        }
    }
    authors.dedup();
    ModFileMetadata {
        id: name.clone(),
        name,
        version,
        description,
        authors,
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "zip")]
    use std::io::Write;

    use super::*;

    /// Writes a zip with the given entries.
    #[cfg(feature = "zip")]
    fn make_zip(path: &Path, entries: &[(&str, &str)]) {
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for (name, contents) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contents.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }

    #[cfg(feature = "zip")]
    fn metadata_of(
        entries: &[(&str, &str)],
        loader: InstalledModLoader,
    ) -> Option<ModFileMetadata> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jar");
        make_zip(&path, entries);
        read_jar_info(&path, loader).metadata
    }

    #[cfg(feature = "zip")]
    #[test]
    fn fabric_mod_json() {
        let json = r#"{
            "schemaVersion": 1,
            "id": "sodium",
            "version": "0.5.3",
            "name": "Sodium",
            "description": "A rendering
engine",
            "authors": ["JellySquid", {"name": "IMS", "contact": {}}, 3]
        }"#;
        let metadata =
            metadata_of(&[("fabric.mod.json", json)], InstalledModLoader::Fabric).unwrap();
        assert_eq!(metadata.id.as_deref(), Some("sodium"));
        assert_eq!(metadata.name.as_deref(), Some("Sodium"));
        assert_eq!(metadata.version.as_deref(), Some("0.5.3"));
        assert_eq!(metadata.description.as_deref(), Some("A rendering engine"));
        assert_eq!(metadata.authors, vec!["JellySquid", "IMS"]);
    }

    #[cfg(feature = "zip")]
    #[test]
    fn quilt_mod_json() {
        let json = r#"{"schema_version": 1, "quilt_loader": {"group": "x", "id": "qsl", "version": "1.0",
            "metadata": {"name": "QSL", "description": "Std lib", "contributors": {"Alice": "Owner"}}}}"#;
        let metadata =
            metadata_of(&[("quilt.mod.json", json)], InstalledModLoader::Fabric).unwrap();
        assert_eq!(metadata.id.as_deref(), Some("qsl"));
        assert_eq!(metadata.name.as_deref(), Some("QSL"));
        assert_eq!(metadata.authors, vec!["Alice"]);
        // fabric.mod.json wins over quilt.mod.json
        let both = metadata_of(
            &[
                ("quilt.mod.json", json),
                ("fabric.mod.json", r#"{"id": "f"}"#),
            ],
            InstalledModLoader::Fabric,
        )
        .unwrap();
        assert_eq!(both.id.as_deref(), Some("f"));
    }

    #[cfg(feature = "zip")]
    #[test]
    fn forge_mods_toml() {
        let toml = r#"
modLoader="javafml"
loaderVersion="[47,)"
[[mods]]
modId="jei"
version="${file.jarVersion}"
displayName="Just Enough Items"
authors="mezz, Alice"
description='''
Item viewer
'''
"#;
        let metadata = metadata_of(
            &[
                ("META-INF/mods.toml", toml),
                (
                    "META-INF/MANIFEST.MF",
                    "Manifest-Version: 1.0\r\nImplementation-Version: 15.2.0.27\r\n",
                ),
            ],
            InstalledModLoader::Forge,
        )
        .unwrap();
        assert_eq!(metadata.id.as_deref(), Some("jei"));
        assert_eq!(metadata.name.as_deref(), Some("Just Enough Items"));
        assert_eq!(metadata.version.as_deref(), Some("15.2.0.27"));
        assert_eq!(metadata.description.as_deref(), Some("Item viewer"));
        assert_eq!(metadata.authors, vec!["mezz", "Alice"]);

        let neo = metadata_of(
            &[(
                "META-INF/neoforge.mods.toml",
                "[[mods]]\nmodId=\"neo\"\nversion=\"2.0\"\n",
            )],
            InstalledModLoader::Forge,
        )
        .unwrap();
        assert_eq!(neo.id.as_deref(), Some("neo"));
        assert_eq!(neo.version.as_deref(), Some("2.0"));
    }

    #[cfg(feature = "zip")]
    #[test]
    fn forge_mcmod_info() {
        let list = r#"[{"modid": "old", "name": "Old Mod", "version": "1.7.10-1", "authorList": ["Bob"]}]"#;
        let metadata = metadata_of(&[("mcmod.info", list)], InstalledModLoader::Forge).unwrap();
        assert_eq!(metadata.id.as_deref(), Some("old"));
        assert_eq!(metadata.authors, vec!["Bob"]);
        let wrapped = r#"{"modListVersion": 2, "modList": [{"modid": "w", "name": "W"}]}"#;
        let metadata = metadata_of(&[("mcmod.info", wrapped)], InstalledModLoader::Forge).unwrap();
        assert_eq!(metadata.name.as_deref(), Some("W"));
    }

    #[cfg(feature = "zip")]
    #[test]
    fn fabric_and_quilt_environment_and_depends() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("a.jar");
        make_zip(
            &jar,
            &[(
                "fabric.mod.json",
                r#"{"id": "a", "environment": "server", "depends": {"fabricloader": ">=0.15", "fabric-api": "*", "minecraft": "1.20.1"}}"#,
            )],
        );
        let info = read_jar_info(&jar, InstalledModLoader::Fabric);
        assert_eq!(info.metadata.unwrap().id.as_deref(), Some("a"));
        assert_eq!(info.environment, Some(JarEnvironment::Server));
        let mut depends = info.depends;
        depends.sort();
        assert_eq!(depends, vec!["fabric-api", "fabricloader", "minecraft"]);

        assert_eq!(
            fabric_environment_and_depends(r#"{"environment": "client"}"#).0,
            Some(JarEnvironment::Client)
        );
        assert_eq!(
            fabric_environment_and_depends(r#"{"environment": "*"}"#).0,
            None
        );
        assert_eq!(fabric_environment_and_depends("{}"), (None, vec![]));

        let quilt = r#"{"quilt_loader": {"id": "q", "depends": ["quilt_loader", {"id": "org.quiltmc:qsl"}, 3]},
            "minecraft": {"environment": "dedicated_server"}}"#;
        let (environment, depends) = quilt_environment_and_depends(quilt);
        assert_eq!(environment, Some(JarEnvironment::Server));
        assert_eq!(depends, vec!["quilt_loader", "qsl"]);

        // other loaders have neither
        let forge = dir.path().join("f.jar");
        make_zip(&forge, &[("META-INF/mods.toml", "[[mods]]\nmodId=\"f\"\n")]);
        let info = read_jar_info(&forge, InstalledModLoader::Forge);
        assert!(info.metadata.is_some());
        assert_eq!(info.environment, None);
        assert!(info.depends.is_empty());
    }

    #[cfg(feature = "zip")]
    #[test]
    fn plugin_yml() {
        let yml = "# comment\nname: LuckPerms\nversion: '5.4.102'\nmain: me.lucko.Main\ndescription: \"A permissions plugin\"\nauthor: Luck\nauthors: [Alice, \"Bob\"]\ncommands:\n  lp:\n    description: nested, ignored\n    name: nested\n";
        let metadata = metadata_of(&[("plugin.yml", yml)], InstalledModLoader::Spigot).unwrap();
        assert_eq!(metadata.id.as_deref(), Some("LuckPerms"));
        assert_eq!(metadata.name.as_deref(), Some("LuckPerms"));
        assert_eq!(metadata.version.as_deref(), Some("5.4.102"));
        assert_eq!(
            metadata.description.as_deref(),
            Some("A permissions plugin")
        );
        assert_eq!(metadata.authors, vec!["Luck", "Alice", "Bob"]);
    }

    #[test]
    fn plugin_yml_block_values() {
        let yml = "name: Essentials # the name\nversion: 2.20.1\nauthors:\n  - zenexer\n  - 'md_5'\ndescription: |\n  Line one\n  Line two\n";
        let metadata = parse_plugin_yml(yml);
        assert_eq!(metadata.name.as_deref(), Some("Essentials"));
        assert_eq!(metadata.version.as_deref(), Some("2.20.1"));
        assert_eq!(metadata.authors, vec!["zenexer", "md_5"]);
        assert_eq!(metadata.description.as_deref(), Some("Line one\nLine two"));
    }

    #[cfg(feature = "zip")]
    #[test]
    fn paper_plugin_yml_wins_over_plugin_yml() {
        let entries = [
            ("plugin.yml", "name: Bukkit\nversion: 1\n"),
            ("paper-plugin.yml", "name: Paper\nversion: 2\n"),
        ];
        let paper = metadata_of(&entries, InstalledModLoader::Paper).unwrap();
        assert_eq!(paper.name.as_deref(), Some("Paper"));
        let spigot = metadata_of(&entries, InstalledModLoader::Spigot).unwrap();
        assert_eq!(spigot.name.as_deref(), Some("Bukkit"));
    }

    #[cfg(feature = "zip")]
    #[test]
    fn metadata_file_presence() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("a.jar");
        make_zip(&jar, &[("fabric.mod.json", "{}"), ("a/B.class", "x")]);
        assert!(jar_has_metadata(&jar, InstalledModLoader::Fabric).unwrap());
        assert!(!jar_has_metadata(&jar, InstalledModLoader::Forge).unwrap());
        assert!(!jar_has_metadata(&jar, InstalledModLoader::Paper).unwrap());
        // the wrong loader's file gives no metadata
        assert_eq!(
            read_jar_info(&jar, InstalledModLoader::Spigot).metadata,
            None
        );

        let not_zip = dir.path().join("b.jar");
        std::fs::write(&not_zip, "not a zip").unwrap();
        let err = jar_has_metadata(&not_zip, InstalledModLoader::Fabric).unwrap_err();
        assert!(matches!(err, Error::Invalid(_)));
        assert_eq!(
            read_jar_info(&not_zip, InstalledModLoader::Fabric).metadata,
            None
        );
    }
}
