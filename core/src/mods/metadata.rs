//! Reads the metadata file inside a mod or plugin jar: `fabric.mod.json`, `quilt.mod.json`,
//! `META-INF/mods.toml`, `META-INF/neoforge.mods.toml`, `mcmod.info`, `paper-plugin.yml` or
//! `plugin.yml`.

use std::io::{Read, Seek};
use std::path::Path;

use color_eyre::eyre::{eyre, Context};
use serde::Deserialize;

use super::{InstalledModLoader, ModFileMetadata};
use crate::error::{Error, ErrorKind};

/// Metadata files larger than this are ignored.
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

fn not_a_jar() -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!("The file is not a valid jar (zip) file"),
    }
}

/// Opens `path` as a zip archive. A file that is not a zip is a `BadRequest`.
fn open_jar(path: &Path) -> Result<zip::ZipArchive<std::fs::File>, Error> {
    let file = std::fs::File::open(path).context(format!("Failed to open {}", path.display()))?;
    zip::ZipArchive::new(file).map_err(|_| not_a_jar())
}

/// Whether the jar at `path` contains one of the loader's metadata files. Fails if it is
/// not a zip file.
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

/// The metadata of the jar at `path`, from the first metadata file of the loader that can
/// be read. `None` if the jar has none or is not a zip file.
pub fn read_jar_metadata(path: &Path, loader: InstalledModLoader) -> Option<ModFileMetadata> {
    let mut archive = open_jar(path).ok()?;
    read_archive_metadata(&mut archive, loader)
}

pub fn read_archive_metadata<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    loader: InstalledModLoader,
) -> Option<ModFileMetadata> {
    for name in metadata_files(loader) {
        let Some(text) = read_entry(archive, name) else {
            continue;
        };
        let parsed = match *name {
            "fabric.mod.json" => parse_fabric_mod_json(&text),
            "quilt.mod.json" => parse_quilt_mod_json(&text),
            "META-INF/mods.toml" | "META-INF/neoforge.mods.toml" => {
                let manifest_version = read_entry(archive, "META-INF/MANIFEST.MF")
                    .and_then(|m| manifest_attribute(&m, "Implementation-Version"));
                parse_mods_toml(&text, manifest_version)
            }
            "mcmod.info" => parse_mcmod_info(&text),
            _ => Some(parse_plugin_yml(&text)),
        };
        if let Some(metadata) = parsed {
            return Some(metadata);
        }
    }
    None
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

pub(crate) fn parse_fabric_mod_json(text: &str) -> Option<ModFileMetadata> {
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

pub(crate) fn parse_quilt_mod_json(text: &str) -> Option<ModFileMetadata> {
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
fn manifest_attribute(manifest: &str, key: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let (k, v) = line.split_once(':')?;
        (k.trim() == key).then(|| v.trim().to_string())
    })
}

pub(crate) fn parse_mods_toml(
    text: &str,
    manifest_version: Option<String>,
) -> Option<ModFileMetadata> {
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

pub(crate) fn parse_mcmod_info(text: &str) -> Option<ModFileMetadata> {
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
pub(crate) fn parse_plugin_yml(text: &str) -> ModFileMetadata {
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
pub(crate) mod tests {
    use std::io::Write;

    use super::*;

    /// Writes a zip with the given entries.
    pub(crate) fn make_zip(path: &Path, entries: &[(&str, &str)]) {
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

    fn metadata_of(
        entries: &[(&str, &str)],
        loader: InstalledModLoader,
    ) -> Option<ModFileMetadata> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jar");
        make_zip(&path, entries);
        read_jar_metadata(&path, loader)
    }

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

    #[test]
    fn metadata_file_presence() {
        let dir = tempfile::tempdir().unwrap();
        let jar = dir.path().join("a.jar");
        make_zip(&jar, &[("fabric.mod.json", "{}"), ("a/B.class", "x")]);
        assert!(jar_has_metadata(&jar, InstalledModLoader::Fabric).unwrap());
        assert!(!jar_has_metadata(&jar, InstalledModLoader::Forge).unwrap());
        assert!(!jar_has_metadata(&jar, InstalledModLoader::Paper).unwrap());
        // the wrong loader's file gives no metadata
        assert_eq!(read_jar_metadata(&jar, InstalledModLoader::Spigot), None);

        let not_zip = dir.path().join("b.jar");
        std::fs::write(&not_zip, "not a zip").unwrap();
        let err = jar_has_metadata(&not_zip, InstalledModLoader::Fabric).unwrap_err();
        assert!(matches!(err.kind, ErrorKind::BadRequest));
        assert_eq!(
            read_jar_metadata(&not_zip, InstalledModLoader::Fabric),
            None
        );
    }
}
