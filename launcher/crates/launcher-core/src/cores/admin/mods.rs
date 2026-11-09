//! Mods, plugins, Modrinth and modpacks: work package A (modpacks, create flow) and C (the Mods tab) (see `launcher/docs/host-parity.md`).
//!
//! Add the routes as `impl AdminClient` methods here, with this area's own types.
//! To do:
//!
//! - `C: GET /instance/{uuid}/mods[?check_updates=true] -> InstalledModList`
//! - `C: POST .../mods/install, POST .../mods/upload[?overwrite=true], PUT .../mods/{file}/enable|disable, DELETE .../mods/{file}`
//! - `C: GET .../mods/client_pack -> ClientPackDownload`

use std::time::Duration;

use http_client::Method;
use serde::{Deserialize, Serialize};

use super::{AdminClient, segment};
use crate::cores::{CoreError, client::MAX_JSON_BYTES};

// ---- Modrinth, through the core (A and C) ----

/// What a search looks for (`ModrinthProjectType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModrinthProjectType {
    Mod,
    Plugin,
    Modpack,
}

impl ModrinthProjectType {
    pub fn as_str(self) -> &'static str {
        match self {
            ModrinthProjectType::Mod => "mod",
            ModrinthProjectType::Plugin => "plugin",
            ModrinthProjectType::Modpack => "modpack",
        }
    }
}

/// A search's order (`ModrinthSearchIndex`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModrinthSearchIndex {
    #[default]
    Relevance,
    Downloads,
    Follows,
    Newest,
    Updated,
}

impl ModrinthSearchIndex {
    pub const ALL: [ModrinthSearchIndex; 5] = [
        ModrinthSearchIndex::Relevance,
        ModrinthSearchIndex::Downloads,
        ModrinthSearchIndex::Follows,
        ModrinthSearchIndex::Newest,
        ModrinthSearchIndex::Updated,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ModrinthSearchIndex::Relevance => "relevance",
            ModrinthSearchIndex::Downloads => "downloads",
            ModrinthSearchIndex::Follows => "follows",
            ModrinthSearchIndex::Newest => "newest",
            ModrinthSearchIndex::Updated => "updated",
        }
    }
}

/// `GET /modrinth/search`'s query.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModrinthSearch {
    pub query: String,
    pub project_type: ModrinthProjectType,
    pub game_version: Option<String>,
    pub loader: Option<String>,
    pub index: ModrinthSearchIndex,
    pub offset: u32,
    pub limit: u32,
}

impl ModrinthSearch {
    fn path(&self) -> String {
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        let text = self.query.trim();
        if !text.is_empty() {
            query.append_pair("query", text);
        }
        query.append_pair("project_type", self.project_type.as_str());
        if let Some(version) = self.game_version.as_deref().filter(|v| !v.is_empty()) {
            query.append_pair("game_version", version);
        }
        if let Some(loader) = self.loader.as_deref().filter(|l| !l.is_empty()) {
            query.append_pair("loader", loader);
        }
        query.append_pair("index", self.index.as_str());
        query.append_pair("offset", &self.offset.to_string());
        query.append_pair("limit", &self.limit.to_string());
        format!("modrinth/search?{}", query.finish())
    }
}

/// A search result (`ModrinthSearchHit`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ModrinthSearchHit {
    pub project_id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub project_type: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
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

/// A page of results (`ModrinthSearchResponse`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct ModrinthSearchResponse {
    pub hits: Vec<ModrinthSearchHit>,
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub limit: u32,
    #[serde(default)]
    pub total_hits: u32,
}

/// An image of a project's gallery (`ModrinthGalleryImage`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ModrinthGalleryImage {
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub featured: bool,
}

/// A project (`ModrinthProject`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ModrinthProject {
    pub id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub project_type: String,
    pub title: String,
    /// The summary.
    #[serde(default)]
    pub description: String,
    /// The long description, in Markdown (untrusted).
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
    pub license_id: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub issues_url: Option<String>,
    #[serde(default)]
    pub wiki_url: Option<String>,
    #[serde(default)]
    pub discord_url: Option<String>,
    #[serde(default)]
    pub gallery: Vec<ModrinthGalleryImage>,
    #[serde(default)]
    pub published: String,
    #[serde(default)]
    pub updated: String,
}

/// A version's release channel (`ModrinthVersionType`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModrinthVersionType {
    Release,
    Beta,
    Alpha,
    #[serde(other)]
    Unknown,
}

/// A file of a version (`ModrinthVersionFile`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ModrinthVersionFile {
    pub filename: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub sha512: String,
    #[serde(default)]
    pub url: String,
}

/// A dependency of a version (`ModrinthDependency`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ModrinthDependency {
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub file_name: Option<String>,
    /// `required`, `optional`, `incompatible` or `embedded`.
    #[serde(default)]
    pub dependency_type: String,
}

/// A version of a project (`ModrinthVersion`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ModrinthVersion {
    pub id: String,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub name: String,
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
    pub files: Vec<ModrinthVersionFile>,
    #[serde(default)]
    pub dependencies: Vec<ModrinthDependency>,
}

impl ModrinthVersion {
    /// The file the core installs: the primary one, else the first (`primaryFile`).
    pub fn primary_file(&self) -> Option<&ModrinthVersionFile> {
        self.files
            .iter()
            .find(|f| f.primary)
            .or_else(|| self.files.first())
    }
}

/// What a modrinth.com link points to (`ModrinthUrlTarget`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ModrinthUrlTarget {
    pub project_id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub project_type: String,
    #[serde(default)]
    pub version_id: Option<String>,
}

/// Whether typed text is a Modrinth link to resolve instead of a search
/// (`looksLikeModrinthLink`).
pub fn looks_like_modrinth_link(text: &str) -> bool {
    text.to_ascii_lowercase().contains("modrinth.com/")
}

impl AdminClient {
    /// `GET /modrinth/search`.
    pub async fn modrinth_search(
        &self,
        search: &ModrinthSearch,
    ) -> Result<ModrinthSearchResponse, CoreError> {
        let bytes = self
            .send_slow(
                Method::GET,
                &search.path(),
                None,
                MAX_JSON_BYTES,
                MODRINTH_TIMEOUT,
            )
            .await?;
        serde_json::from_slice(&bytes).map_err(|err| CoreError::Invalid(err.to_string()))
    }

    /// `GET /modrinth/project/{id or slug}`.
    pub async fn modrinth_project(&self, id: &str) -> Result<ModrinthProject, CoreError> {
        self.get(&format!("modrinth/project/{}", segment(id))).await
    }

    /// `GET /modrinth/project/{id}/versions`, newest first, filtered by game version and
    /// loader when given.
    pub async fn modrinth_versions(
        &self,
        id: &str,
        game_version: Option<&str>,
        loader: Option<&str>,
    ) -> Result<Vec<ModrinthVersion>, CoreError> {
        let path = versions_path(id, game_version, loader);
        let bytes = self
            .send_slow(
                Method::GET,
                &path,
                None,
                16 * MAX_JSON_BYTES,
                MODRINTH_TIMEOUT,
            )
            .await?;
        serde_json::from_slice(&bytes).map_err(|err| CoreError::Invalid(err.to_string()))
    }

    /// `GET /modrinth/version/{id}`.
    pub async fn modrinth_version(&self, id: &str) -> Result<ModrinthVersion, CoreError> {
        self.get(&format!("modrinth/version/{}", segment(id))).await
    }

    /// `GET /modrinth/resolve?url=`: the project (and version) a modrinth.com link
    /// points to.
    pub async fn modrinth_resolve(&self, link: &str) -> Result<ModrinthUrlTarget, CoreError> {
        let path = query_path("modrinth/resolve", &[("url", Some(link.trim()))]);
        self.get(&path).await
    }
}

/// `path?name=value&…` for the pairs that have a non-empty value.
fn query_path(path: &str, pairs: &[(&str, Option<&str>)]) -> String {
    let mut query = url::form_urlencoded::Serializer::new(String::new());
    for (name, value) in pairs {
        if let Some(value) = value.filter(|v| !v.is_empty()) {
            query.append_pair(name, value);
        }
    }
    let query = query.finish();
    if query.is_empty() {
        path.to_string()
    } else {
        format!("{path}?{query}")
    }
}

fn versions_path(id: &str, game_version: Option<&str>, loader: Option<&str>) -> String {
    query_path(
        &format!("modrinth/project/{}/versions", segment(id)),
        &[("game_version", game_version), ("loader", loader)],
    )
}

/// Modrinth answers through the core; the dashboard waits 30 s.
const MODRINTH_TIMEOUT: Duration = Duration::from_secs(30);

// ---- Modpacks (A) ----

/// What the core found in a modpack (`ModpackInfo`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ModpackInfo {
    pub name: String,
    #[serde(default)]
    pub version_number: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub game_version: String,
    /// For example `fabric`.
    #[serde(default)]
    pub loader: String,
    #[serde(default)]
    pub loader_version: Option<String>,
    #[serde(default)]
    pub server_file_count: u32,
    /// Bytes.
    #[serde(default)]
    pub server_download_size: u64,
    /// Set when no instance can be made from the pack.
    #[serde(default)]
    pub unsupported_reason: Option<String>,
    /// For `create_modpack`, to reuse the downloaded or uploaded pack.
    #[serde(default)]
    pub upload_id: Option<String>,
}

impl AdminClient {
    /// `POST /modpack/inspect`: the core downloads a Modrinth modpack version and says
    /// what is in it. May take a while.
    pub async fn inspect_modpack(&self, version_id: &str) -> Result<ModpackInfo, CoreError> {
        let body = serde_json::to_vec(&serde_json::json!({ "version_id": version_id }))
            .map_err(|err| CoreError::Invalid(err.to_string()))?;
        let bytes = self
            .send_slow(
                Method::POST,
                "modpack/inspect",
                Some((body, "application/json".into())),
                MAX_JSON_BYTES,
                MODPACK_TIMEOUT,
            )
            .await?;
        serde_json::from_slice(&bytes).map_err(|err| CoreError::Invalid(err.to_string()))
    }

    /// `POST /modpack/upload` (multipart, field `file`): an `.mrpack` from this
    /// computer. The answer has an `upload_id` for `create_modpack`.
    pub async fn upload_modpack(
        &self,
        file_name: &str,
        bytes: &[u8],
    ) -> Result<ModpackInfo, CoreError> {
        let body = crate::cores::hosting::multipart_file(file_name, bytes);
        let bytes = self
            .send_slow(
                Method::POST,
                "modpack/upload",
                Some(body),
                MAX_JSON_BYTES,
                MODPACK_TIMEOUT,
            )
            .await?;
        serde_json::from_slice(&bytes).map_err(|err| CoreError::Invalid(err.to_string()))
    }
}

/// Downloading or uploading a modpack: the dashboard sets no limit.
const MODPACK_TIMEOUT: Duration = Duration::from_secs(30 * 60);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_paths() {
        let search = ModrinthSearch {
            query: " create mod ".into(),
            project_type: ModrinthProjectType::Modpack,
            game_version: None,
            loader: Some("fabric".into()),
            index: ModrinthSearchIndex::Downloads,
            offset: 20,
            limit: 20,
        };
        assert_eq!(
            search.path(),
            "modrinth/search?query=create+mod&project_type=modpack&loader=fabric&index=downloads&offset=20&limit=20"
        );
        assert!(looks_like_modrinth_link("https://Modrinth.com/modpack/x"));
        assert!(!looks_like_modrinth_link("fabulously optimized"));
    }

    #[test]
    fn reads_versions_and_packs() {
        let version: ModrinthVersion = serde_json::from_value(serde_json::json!({
            "id": "v1", "project_id": "p", "name": "1.0", "version_number": "1.0.0",
            "version_type": "snapshot", "game_versions": ["1.21.1"], "loaders": ["fabric"],
            "date_published": "2025-01-01T00:00:00Z", "downloads": 3,
            "files": [
                { "filename": "a.mrpack", "size": 10, "primary": false, "sha512": "", "url": "" },
                { "filename": "b.mrpack", "size": 20, "primary": true, "sha512": "", "url": "" }
            ],
            "dependencies": []
        }))
        .unwrap();
        assert_eq!(version.version_type, ModrinthVersionType::Unknown);
        assert_eq!(version.primary_file().unwrap().filename, "b.mrpack");
        let info: ModpackInfo = serde_json::from_value(serde_json::json!({
            "name": "Pack", "version_number": null, "summary": null, "game_version": "1.21.1",
            "loader": "fabric", "loader_version": "0.16.0", "server_file_count": 120,
            "server_download_size": 1000, "unsupported_reason": null, "upload_id": "abc"
        }))
        .unwrap();
        assert_eq!(info.upload_id.as_deref(), Some("abc"));
    }
}
