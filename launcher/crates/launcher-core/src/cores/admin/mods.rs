//! Mods, plugins, Modrinth and modpacks: work package A (modpacks, create flow) and C (the Mods tab) (see `launcher/docs/host-parity.md`).
//!
//! - Modrinth (A and C): search, projects, versions, links.
//! - Modpacks (A): inspect and upload.
//! - Installed mods and plugins (C): `GET /instance/{uuid}/mods[?check_updates=true]`,
//!   install, upload, enable, disable, delete, and the client modpack.

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

// ---- Installed mods and plugins: the Mods tab (C) ----

/// Mods or plugins (`InstalledModKind`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstalledModKind {
    #[default]
    Mod,
    Plugin,
}

/// An instance's loader, as the Mods tab knows it (`InstalledModLoader`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstalledModLoader {
    #[default]
    Fabric,
    Forge,
    Paper,
    Spigot,
    #[serde(other)]
    Unknown,
}

impl InstalledModLoader {
    pub fn as_str(self) -> &'static str {
        match self {
            InstalledModLoader::Fabric => "fabric",
            InstalledModLoader::Forge => "forge",
            InstalledModLoader::Paper => "paper",
            InstalledModLoader::Spigot => "spigot",
            InstalledModLoader::Unknown => "",
        }
    }

    /// The Modrinth loaders whose versions the core installs without `force`
    /// (`compatibleLoaders`): Paper also takes Bukkit and Spigot plugins.
    pub fn compatible(self) -> &'static [&'static str] {
        match self {
            InstalledModLoader::Paper => &["paper", "bukkit", "spigot"],
            InstalledModLoader::Fabric => &["fabric"],
            InstalledModLoader::Forge => &["forge"],
            InstalledModLoader::Spigot => &["spigot"],
            InstalledModLoader::Unknown => &[],
        }
    }
}

/// Whether the core installs `version` on an instance with `loader` and
/// `game_version` without `force` (`isVersionCompatible`).
pub fn is_version_compatible(
    version: &ModrinthVersion,
    loader: InstalledModLoader,
    game_version: &str,
) -> bool {
    version.game_versions.iter().any(|v| v == game_version)
        && version
            .loaders
            .iter()
            .any(|l| loader.compatible().contains(&l.as_str()))
}

/// What a jar says about itself (`ModFileMetadata`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ModFileMetadata {
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
}

/// A newer version on Modrinth (`ModrinthUpdate`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ModrinthUpdate {
    pub version_id: String,
    pub version_number: String,
}

/// The Modrinth project and version an installed file belongs to (`ModrinthMatch`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ModrinthMatch {
    pub project_id: String,
    pub version_id: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub version_number: String,
    /// Only when updates were checked.
    #[serde(default)]
    pub update: Option<ModrinthUpdate>,
    #[serde(default)]
    pub client_side: String,
    #[serde(default)]
    pub server_side: String,
}

/// Whether players must install a mod that is on the server (`ClientRequirement`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientRequirement {
    Required,
    Optional,
    Unsupported,
    #[default]
    #[serde(other)]
    Unknown,
}

/// A mod or plugin file of an instance (`InstalledMod`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct InstalledMod {
    /// The name on disk, with `.disabled` for a disabled one.
    pub file_name: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub metadata: Option<ModFileMetadata>,
    #[serde(default)]
    pub modrinth: Option<ModrinthMatch>,
    #[serde(default)]
    pub client_requirement: ClientRequirement,
    /// The installed mods that make players need this one.
    #[serde(default)]
    pub required_by: Vec<String>,
}

impl InstalledMod {
    /// The name to show (`installedModName`).
    pub fn name(&self) -> &str {
        self.modrinth
            .as_ref()
            .map(|m| m.title.as_str())
            .filter(|t| !t.is_empty())
            .or_else(|| {
                self.metadata
                    .as_ref()
                    .and_then(|m| m.name.as_deref())
                    .filter(|n| !n.is_empty())
            })
            .unwrap_or(&self.file_name)
    }

    /// The version to show, if any (`installedModVersion`).
    pub fn version(&self) -> Option<&str> {
        self.modrinth
            .as_ref()
            .map(|m| m.version_number.as_str())
            .filter(|v| !v.is_empty())
            .or_else(|| {
                self.metadata
                    .as_ref()
                    .and_then(|m| m.version.as_deref())
                    .filter(|v| !v.is_empty())
            })
    }

    /// Whether players need it after it was installed (`playersNeed`).
    pub fn players_need(&self) -> bool {
        self.client_requirement == ClientRequirement::Required
            || (self.client_requirement != ClientRequirement::Unsupported
                && self
                    .modrinth
                    .as_ref()
                    .is_some_and(|m| m.client_side == "required"))
    }
}

/// An instance's mods or plugins (`InstalledModList`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct InstalledModList {
    pub kind: InstalledModKind,
    pub loader: InstalledModLoader,
    pub game_version: String,
    /// `mods` or `plugins`, in the instance.
    pub directory: String,
    pub restart_required: bool,
    /// Fabric's loader version, which players need too.
    pub loader_version: Option<String>,
    /// How many enabled mods players need.
    pub client_required_count: u32,
    /// By name, ignoring case.
    pub mods: Vec<InstalledMod>,
}

/// `POST /instance/{uuid}/mods/install` (`ModInstallRequest`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ModInstallRequest {
    pub version_id: String,
    pub install_dependencies: bool,
    /// Install even if the version isn't for this loader or game version.
    pub force: bool,
}

/// A project an install skipped, and why (`ModInstallSkipped`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ModInstallSkipped {
    pub project_id: String,
    #[serde(default)]
    pub reason: String,
}

/// What an install did (`ModInstallResult`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ModInstallResult {
    pub installed: Vec<InstalledMod>,
    /// File names.
    pub removed: Vec<String>,
    pub skipped: Vec<ModInstallSkipped>,
}

impl ModInstallResult {
    /// Adds another install's result, for "Update all".
    pub fn extend(&mut self, other: ModInstallResult) {
        self.installed.extend(other.installed);
        self.removed.extend(other.removed);
        self.skipped.extend(other.skipped);
    }
}

/// `GET /instance/{uuid}/mods/client_pack` (`ClientPackDownload`): a download key for
/// `GET /file/{key}`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ClientPackDownload {
    pub key: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default)]
    pub mod_count: u32,
    #[serde(default)]
    pub included_jars: u32,
}

/// The page of a project on modrinth.com (`modrinthProjectUrl`).
pub fn modrinth_project_url(project_type: &str, slug: &str) -> String {
    format!(
        "https://modrinth.com/{}/{}",
        segment(project_type),
        segment(slug)
    )
}

/// A multipart body with each file as a `file` field, for uploads of jars.
fn multipart_files(files: &[(String, Vec<u8>)]) -> (Vec<u8>, String) {
    let boundary = format!(
        "lodestone-host-{:016x}",
        u64::from_le_bytes(crate::random_bytes())
    );
    let mut body = Vec::new();
    for (name, bytes) in files {
        // quotes and line breaks would end the header
        let safe_name: String = name
            .chars()
            .map(|c| if c == '"' || c.is_control() { '_' } else { c })
            .collect();
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{safe_name}\"\r\nContent-Type: application/java-archive\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (body, format!("multipart/form-data; boundary={boundary}"))
}

/// Installs and uploads take as long as their downloads; the dashboard sets no limit.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(30 * 60);

impl AdminClient {
    /// `GET /instance/{uuid}/mods[?check_updates=true]`. Checking for updates asks
    /// Modrinth too, so it is slower.
    pub async fn installed_mods(
        &self,
        uuid: &str,
        check_updates: bool,
    ) -> Result<InstalledModList, CoreError> {
        let mut path = format!("instance/{}/mods", segment(uuid));
        if check_updates {
            path.push_str("?check_updates=true");
        }
        let bytes = self
            .send_slow(
                Method::GET,
                &path,
                None,
                4 * MAX_JSON_BYTES,
                MODRINTH_TIMEOUT,
            )
            .await?;
        serde_json::from_slice(&bytes).map_err(|err| CoreError::Invalid(err.to_string()))
    }

    /// `POST /instance/{uuid}/mods/install`: a Modrinth version (and its required
    /// dependencies), replacing other versions of the same project.
    pub async fn install_mod(
        &self,
        uuid: &str,
        request: &ModInstallRequest,
    ) -> Result<ModInstallResult, CoreError> {
        let body =
            serde_json::to_vec(request).map_err(|err| CoreError::Invalid(err.to_string()))?;
        let bytes = self
            .send_slow(
                Method::POST,
                &format!("instance/{}/mods/install", segment(uuid)),
                Some((body, "application/json".into())),
                4 * MAX_JSON_BYTES,
                INSTALL_TIMEOUT,
            )
            .await?;
        serde_json::from_slice(&bytes).map_err(|err| CoreError::Invalid(err.to_string()))
    }

    /// `POST /instance/{uuid}/mods/upload[?overwrite=true]`: jars into the mods or
    /// plugins folder. A file that exists already is a 409 (`CoreError::Server`) unless
    /// `overwrite`.
    pub async fn upload_mods(
        &self,
        uuid: &str,
        files: &[(String, Vec<u8>)],
        overwrite: bool,
    ) -> Result<Vec<InstalledMod>, CoreError> {
        let mut path = format!("instance/{}/mods/upload", segment(uuid));
        if overwrite {
            path.push_str("?overwrite=true");
        }
        let bytes = self
            .send_slow(
                Method::POST,
                &path,
                Some(multipart_files(files)),
                MAX_JSON_BYTES,
                INSTALL_TIMEOUT,
            )
            .await?;
        serde_json::from_slice(&bytes).map_err(|err| CoreError::Invalid(err.to_string()))
    }

    /// `PUT /instance/{uuid}/mods/{file}/enable` or `…/disable`.
    pub async fn set_mod_enabled(
        &self,
        uuid: &str,
        file_name: &str,
        enabled: bool,
    ) -> Result<InstalledMod, CoreError> {
        self.json(
            Method::PUT,
            &format!(
                "instance/{}/mods/{}/{}",
                segment(uuid),
                segment(file_name),
                if enabled { "enable" } else { "disable" }
            ),
            None::<()>,
        )
        .await
    }

    /// `DELETE /instance/{uuid}/mods/{file}`.
    pub async fn delete_mod(&self, uuid: &str, file_name: &str) -> Result<(), CoreError> {
        self.call(
            Method::DELETE,
            &format!("instance/{}/mods/{}", segment(uuid), segment(file_name)),
            None::<()>,
        )
        .await
    }

    /// `GET /instance/{uuid}/mods/client_pack`: builds the client modpack (hashing
    /// every jar, which takes a while) and gives a download key for it.
    pub async fn client_pack(&self, uuid: &str) -> Result<ClientPackDownload, CoreError> {
        let bytes = self
            .send_slow(
                Method::GET,
                &format!("instance/{}/mods/client_pack", segment(uuid)),
                None,
                MAX_JSON_BYTES,
                INSTALL_TIMEOUT,
            )
            .await?;
        serde_json::from_slice(&bytes).map_err(|err| CoreError::Invalid(err.to_string()))
    }

    /// `GET /file/{key}`: a file a download key names (the client pack). Needs no
    /// sign-in: the key is the permission. Up to `limit` bytes.
    pub async fn download_file(&self, key: &str, limit: u64) -> Result<Vec<u8>, CoreError> {
        self.send_slow(
            Method::GET,
            &format!("file/{}", segment(key)),
            None,
            limit,
            INSTALL_TIMEOUT,
        )
        .await
    }
}

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

    #[test]
    fn reads_an_installed_list() {
        let list: InstalledModList = serde_json::from_str(
            r#"{"kind":"mod","loader":"fabric","game_version":"1.21.1","directory":"mods",
                "restart_required":true,"loader_version":"0.16.14","client_required_count":1,
                "mods":[{"file_name":"sodium.jar","enabled":true,"size":10,"sha512":"x",
                  "metadata":{"id":"sodium","name":"Sodium","version":"0.6","description":null,"authors":[]},
                  "modrinth":{"project_id":"p","version_id":"v","slug":"sodium","title":"Sodium",
                    "icon_url":null,"version_number":"mc1.21-0.6","update":{"version_id":"v2","version_number":"0.7"},
                    "client_side":"required","server_side":"optional"},
                  "client_requirement":"required","required_by":[]},
                 {"file_name":"x.jar.disabled","enabled":false,"size":1,"sha512":"y","metadata":null,
                  "modrinth":null,"client_requirement":"something","required_by":[]}]}"#,
        )
        .unwrap();
        assert_eq!(list.loader, InstalledModLoader::Fabric);
        assert_eq!(list.mods[0].name(), "Sodium");
        assert_eq!(list.mods[0].version(), Some("mc1.21-0.6"));
        assert!(list.mods[0].players_need());
        assert_eq!(list.mods[1].name(), "x.jar.disabled");
        assert_eq!(list.mods[1].client_requirement, ClientRequirement::Unknown);
    }

    #[test]
    fn paper_takes_bukkit_plugins() {
        let version: ModrinthVersion = serde_json::from_str(
            r#"{"id":"v","project_id":"p","name":"","version_number":"1","version_type":"release",
                "game_versions":["1.21.1"],"loaders":["bukkit"],"date_published":"","downloads":0,
                "files":[],"dependencies":[]}"#,
        )
        .unwrap();
        assert!(is_version_compatible(
            &version,
            InstalledModLoader::Paper,
            "1.21.1"
        ));
        assert!(!is_version_compatible(
            &version,
            InstalledModLoader::Paper,
            "1.20"
        ));
        assert!(!is_version_compatible(
            &version,
            InstalledModLoader::Fabric,
            "1.21.1"
        ));
    }

    #[test]
    fn uploads_are_one_field_per_file() {
        let (body, content_type) = multipart_files(&[
            ("a.jar".into(), b"AAA".to_vec()),
            ("b\"c.jar".into(), b"BBB".to_vec()),
        ]);
        let text = String::from_utf8(body).unwrap();
        assert!(content_type.starts_with("multipart/form-data; boundary="));
        assert_eq!(text.matches("name=\"file\"").count(), 2);
        assert!(text.contains("filename=\"b_c.jar\""));
        assert!(text.trim_end().ends_with("--"));
    }
}
