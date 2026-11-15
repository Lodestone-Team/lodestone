//! The Modrinth types Lodestone Core sends to clients, and the raw Modrinth API shapes they
//! are made from.
//!
//! The `Modrinth*` types are part of the Lodestone API (see the generated
//! `core/bindings/Modrinth*.ts`, made with the `ts-rs` feature).
//! They are `snake_case`, like the Modrinth API, so most of them deserialize straight from
//! Modrinth's responses. Unknown fields in Modrinth's responses are ignored.

use serde::{Deserialize, Serialize};

/// An empty string from Modrinth (for example an icon URL) means "none".
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ModrinthProjectType {
    Mod,
    Plugin,
    Modpack,
}

impl ModrinthProjectType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModrinthProjectType::Mod => "mod",
            ModrinthProjectType::Plugin => "plugin",
            ModrinthProjectType::Modpack => "modpack",
        }
    }
}

/// The sort order of a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ModrinthSearchIndex {
    #[default]
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
}

impl ModrinthSearchIndex {
    pub fn as_str(&self) -> &'static str {
        match self {
            ModrinthSearchIndex::Relevance => "relevance",
            ModrinthSearchIndex::Downloads => "downloads",
            ModrinthSearchIndex::Follows => "follows",
            ModrinthSearchIndex::Newest => "newest",
            ModrinthSearchIndex::Updated => "updated",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct ModrinthSearchResponse {
    pub hits: Vec<ModrinthSearchHit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct ModrinthSearchHit {
    pub project_id: String,
    #[serde(default)]
    pub slug: String,
    pub project_type: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub downloads: u64,
    #[serde(default)]
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub follows: u64,
    #[serde(default)]
    pub categories: Vec<String>,
    /// Game versions.
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(default)]
    pub date_modified: String,
    #[serde(default)]
    pub client_side: String,
    #[serde(default)]
    pub server_side: String,
}

impl ModrinthSearchResponse {
    /// Turns empty icon URLs into `None`, as for projects.
    pub fn normalize(mut self) -> Self {
        for hit in &mut self.hits {
            hit.icon_url = non_empty(hit.icon_url.take());
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct ModrinthGalleryImage {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub featured: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct ModrinthProject {
    pub id: String,
    pub slug: String,
    pub project_type: String,
    pub title: String,
    /// The summary.
    pub description: String,
    /// The long description, in Markdown.
    pub body: String,
    pub icon_url: Option<String>,
    pub categories: Vec<String>,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub downloads: u64,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub followers: u64,
    pub client_side: String,
    pub server_side: String,
    pub license_id: Option<String>,
    pub source_url: Option<String>,
    pub issues_url: Option<String>,
    pub wiki_url: Option<String>,
    pub discord_url: Option<String>,
    pub gallery: Vec<ModrinthGalleryImage>,
    pub published: String,
    pub updated: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ModrinthVersionType {
    Release,
    Beta,
    Alpha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ModrinthDependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct ModrinthVersionFile {
    pub filename: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub size: u64,
    pub primary: bool,
    pub sha512: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct ModrinthDependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub dependency_type: ModrinthDependencyType,
    pub file_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct ModrinthVersion {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: ModrinthVersionType,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date_published: String,
    #[cfg_attr(feature = "ts-rs", ts(type = "number"))]
    pub downloads: u64,
    pub files: Vec<ModrinthVersionFile>,
    pub dependencies: Vec<ModrinthDependency>,
    /// What the author wrote about the version (Markdown), at most
    /// [`MAX_CHANGELOG_BYTES`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts-rs", ts(optional))]
    pub changelog: Option<String>,
}

/// Changelogs longer than this are cut (at a character boundary).
pub const MAX_CHANGELOG_BYTES: usize = 16 * 1024;

/// `text` cut to at most `max` bytes, at a character boundary.
pub fn cut_text(text: &str, max: usize) -> String {
    if text.len() <= max {
        return text.to_string();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

impl ModrinthVersion {
    /// The primary file, or the first file if none is primary.
    pub fn primary_file(&self) -> Option<&ModrinthVersionFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }
}

/// What a Modrinth link points to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct ModrinthUrlTarget {
    pub project_id: String,
    pub slug: String,
    pub project_type: String,
    pub version_id: Option<String>,
}

// ---- raw Modrinth shapes, which convert into the types above ----

#[derive(Debug, Clone, Deserialize)]
pub struct RawLicense {
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawGalleryImage {
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub featured: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawProject {
    pub id: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub project_type: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub followers: u64,
    #[serde(default)]
    pub client_side: String,
    #[serde(default)]
    pub server_side: String,
    #[serde(default)]
    pub license: Option<RawLicense>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub issues_url: Option<String>,
    #[serde(default)]
    pub wiki_url: Option<String>,
    #[serde(default)]
    pub discord_url: Option<String>,
    #[serde(default)]
    pub gallery: Vec<RawGalleryImage>,
    #[serde(default)]
    pub published: String,
    #[serde(default)]
    pub updated: String,
}

impl From<RawProject> for ModrinthProject {
    fn from(raw: RawProject) -> Self {
        ModrinthProject {
            slug: raw.slug.unwrap_or_else(|| raw.id.clone()),
            id: raw.id,
            project_type: raw.project_type,
            title: raw.title,
            description: raw.description,
            body: raw.body,
            icon_url: non_empty(raw.icon_url),
            categories: raw.categories,
            loaders: raw.loaders,
            game_versions: raw.game_versions,
            downloads: raw.downloads,
            followers: raw.followers,
            client_side: raw.client_side,
            server_side: raw.server_side,
            license_id: non_empty(raw.license.and_then(|l| l.id)),
            source_url: non_empty(raw.source_url),
            issues_url: non_empty(raw.issues_url),
            wiki_url: non_empty(raw.wiki_url),
            discord_url: non_empty(raw.discord_url),
            gallery: raw
                .gallery
                .into_iter()
                .map(|g| ModrinthGalleryImage {
                    url: g.url,
                    title: non_empty(g.title),
                    description: non_empty(g.description),
                    featured: g.featured,
                })
                .collect(),
            published: raw.published,
            updated: raw.updated,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawHashes {
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub sha512: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawVersionFile {
    #[serde(default)]
    pub hashes: RawHashes,
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawVersion {
    pub id: String,
    pub project_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version_number: String,
    pub version_type: ModrinthVersionType,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub date_published: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub files: Vec<RawVersionFile>,
    #[serde(default)]
    pub dependencies: Vec<ModrinthDependency>,
    #[serde(default)]
    pub changelog: Option<String>,
}

impl RawVersion {
    /// The primary file, or the first file if none is primary.
    pub fn primary_file(&self) -> Option<&RawVersionFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }
}

impl From<RawVersion> for ModrinthVersion {
    fn from(raw: RawVersion) -> Self {
        ModrinthVersion {
            id: raw.id,
            project_id: raw.project_id,
            name: raw.name,
            version_number: raw.version_number,
            version_type: raw.version_type,
            game_versions: raw.game_versions,
            loaders: raw.loaders,
            date_published: raw.date_published,
            downloads: raw.downloads,
            files: raw
                .files
                .into_iter()
                .map(|f| ModrinthVersionFile {
                    filename: f.filename,
                    size: f.size,
                    primary: f.primary,
                    sha512: f.hashes.sha512.unwrap_or_default().to_lowercase(),
                    url: f.url,
                })
                .collect(),
            dependencies: raw.dependencies,
            changelog: raw
                .changelog
                .map(|c| cut_text(c.trim(), MAX_CHANGELOG_BYTES))
                .filter(|c| !c.is_empty()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_version_converts() {
        let raw: RawVersion = serde_json::from_value(serde_json::json!({
            "id": "v1",
            "project_id": "p1",
            "name": "Fabric API 0.92",
            "version_number": "0.92.2+1.20.1",
            "version_type": "release",
            "game_versions": ["1.20.1"],
            "loaders": ["fabric"],
            "date_published": "2024-01-01T00:00:00Z",
            "downloads": 5,
            "changelog": "ignored",
            "files": [
                {"hashes": {"sha1": "aa", "sha512": "ABC"}, "url": "https://cdn.modrinth.com/x.jar",
                 "filename": "x.jar", "primary": false, "size": 10, "file_type": null},
                {"hashes": {"sha512": "def"}, "url": "https://cdn.modrinth.com/y.jar",
                 "filename": "y.jar", "primary": true, "size": 20}
            ],
            "dependencies": [
                {"project_id": "p2", "version_id": null, "file_name": null, "dependency_type": "required"}
            ]
        }))
        .unwrap();
        assert_eq!(raw.primary_file().unwrap().filename, "y.jar");
        assert_eq!(raw.files[0].hashes.sha1.as_deref(), Some("aa"));
        assert_eq!(raw.files[1].hashes.sha1, None);
        let version = ModrinthVersion::from(raw);
        assert_eq!(version.files[0].sha512, "abc");
        assert_eq!(version.primary_file().unwrap().filename, "y.jar");
        assert_eq!(
            version.dependencies[0].dependency_type,
            ModrinthDependencyType::Required
        );
    }

    #[test]
    fn raw_project_converts() {
        let raw: RawProject = serde_json::from_value(serde_json::json!({
            "id": "P7dR8mSH",
            "slug": "fabric-api",
            "project_type": "mod",
            "title": "Fabric API",
            "description": "Lightweight API",
            "body": "# Hi",
            "icon_url": "",
            "license": {"id": "Apache-2.0", "name": "Apache", "url": null},
            "source_url": "https://github.com/FabricMC/fabric",
            "discord_url": "",
            "gallery": [{"url": "https://cdn.modrinth.com/g.png", "featured": true, "title": "", "description": null, "created": "x", "ordering": 0}],
            "downloads": 1, "followers": 2
        }))
        .unwrap();
        let project = ModrinthProject::from(raw);
        assert_eq!(project.icon_url, None);
        assert_eq!(project.license_id.as_deref(), Some("Apache-2.0"));
        assert_eq!(project.discord_url, None);
        assert_eq!(project.gallery[0].title, None);
        assert!(project.gallery[0].featured);
    }
}

#[cfg(test)]
mod changelog_tests {
    use super::*;

    #[test]
    fn long_text_is_cut_at_a_character() {
        assert_eq!(cut_text("short", 10), "short");
        assert_eq!(cut_text("abcdef", 3), "abc…");
        // "é" is two bytes: the cut moves back before it
        assert_eq!(cut_text("aé", 2), "a…");
    }
}
