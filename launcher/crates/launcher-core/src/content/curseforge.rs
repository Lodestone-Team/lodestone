//! The CurseForge API (<https://docs.curseforge.com/rest-api/>), v1, which needs a key.
//!
//! The key Lodestone was issued is compiled in from `LODESTONE_CURSEFORGE_API_KEY`
//! when the launcher is built; the same environment variable at run time overrides it
//! (for development). Without a key there is no client ([`api_key`] is `None`) and the
//! app doesn't offer CurseForge. The key is sent only to the API host, never to the
//! CDN, and never logged ([`crate::trace`] logs hosts and paths only).
//!
//! CurseForge's terms for third-party apps don't allow saving or caching what the API
//! returns, so unlike [`super::Modrinth`] nothing here is cached. A 429 makes calls
//! fail at once for [`RATE_LIMIT_PAUSE`]; a 403 means the key was refused.
//!
//! Some authors don't allow their files to be downloaded outside CurseForge: such a
//! file has no `downloadUrl` ([`CfFile::download_url`] is `None`). The launcher never
//! builds a CDN URL for one; it sends the user to the file's page on curseforge.com
//! ([`CfFile::page_url`]) to download it in their browser.
//!
//! CurseForge identifies files by a MurmurHash2 [`fingerprint`] of their bytes with
//! whitespace removed.

use std::{
    io::{self, Read},
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpClient, HttpRequestExt as _, Method, RedirectPolicy, Request};
use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;

use super::{ContentKind, modrinth::USER_AGENT};
use crate::instances::Loader;

pub const API_BASE: &str = "https://api.curseforge.com/v1";
/// Overrides [`API_BASE`], to run the launcher against a local mock (loopback only).
pub const API_URL_ENV: &str = "LODESTONE_CURSEFORGE_API_URL";
pub const API_KEY_ENV: &str = "LODESTONE_CURSEFORGE_API_KEY";
/// CurseForge's id for Minecraft.
pub const GAME_ID: u32 = 432;
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
pub const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
/// How long calls fail at once after CurseForge answers 429.
pub const RATE_LIMIT_PAUSE: Duration = Duration::from_secs(30);
/// The most results a page may hold, and the most a search can reach (`index +
/// pageSize`), per the API documentation.
pub const PAGE_SIZE: usize = 50;
pub const MAX_SEARCH_RESULTS: usize = 10_000;
/// The most files read for one project, in pages of [`PAGE_SIZE`].
pub const MAX_FILES: usize = 2_000;
/// Hosts CurseForge serves files from. Anything else is refused.
pub const DOWNLOAD_HOSTS: [&str; 2] = ["edge.forgecdn.net", "mediafilez.forgecdn.net"];
/// The host of project logos and screenshots.
pub const MEDIA_HOST: &str = "media.forgecdn.net";

/// The API key: `LODESTONE_CURSEFORGE_API_KEY` at run time, else the one compiled in.
pub fn api_key() -> Option<String> {
    key_from(
        std::env::var(API_KEY_ENV).ok(),
        option_env!("LODESTONE_CURSEFORGE_API_KEY"),
    )
}

fn key_from(runtime: Option<String>, built: Option<&str>) -> Option<String> {
    runtime
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
        .or_else(|| {
            built
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .map(String::from)
        })
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CurseForgeError {
    #[error("{0} isn't on CurseForge")]
    NotFound(String),
    #[error("CurseForge asks to slow down; try again in {seconds} seconds")]
    RateLimited { seconds: u64 },
    #[error("CurseForge refused Lodestone's API key")]
    KeyRefused,
    #[error("CurseForge isn't reachable: {0}")]
    Unreachable(String),
    #[error("{0}")]
    Invalid(String),
}

// ---------------------------------------------------------------------------
// What the API returns. Only the fields the launcher uses; the rest are ignored.

#[derive(Deserialize)]
struct Data<T> {
    data: T,
}

/// A project ("mod" in the API, whatever its class).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfMod {
    pub id: u64,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub links: CfLinks,
    #[serde(default)]
    pub logo: Option<CfImage>,
    #[serde(default)]
    pub authors: Vec<CfAuthor>,
    #[serde(default)]
    pub class_id: Option<u32>,
    #[serde(default)]
    pub download_count: f64,
    #[serde(default)]
    pub date_modified: String,
    /// `false` when the author doesn't allow downloads outside CurseForge.
    #[serde(default)]
    pub allow_mod_distribution: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfLinks {
    #[serde(default)]
    pub website_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfImage {
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CfAuthor {
    pub name: String,
}

impl CfMod {
    /// The project's page on curseforge.com.
    pub fn page_url(&self) -> String {
        self.links
            .website_url
            .clone()
            .filter(|url| url.starts_with("https://www.curseforge.com/"))
            .unwrap_or_else(|| format!("https://www.curseforge.com/projects/{}", self.id))
    }

    pub fn icon_url(&self) -> Option<&str> {
        let logo = self.logo.as_ref()?;
        logo.thumbnail_url.as_deref().or(logo.url.as_deref())
    }

    /// The kind of content, from the project's class.
    pub fn kind(&self) -> Option<ContentKind> {
        self.class_id.and_then(kind_of_class)
    }
}

/// One file of a project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfFile {
    pub id: u64,
    pub mod_id: u64,
    #[serde(default)]
    pub display_name: String,
    pub file_name: String,
    /// 1 release, 2 beta, 3 alpha.
    #[serde(default = "release")]
    pub release_type: u8,
    #[serde(default)]
    pub file_date: String,
    /// `None` when the author doesn't allow downloads outside CurseForge.
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub file_length: u64,
    #[serde(default)]
    pub hashes: Vec<CfHash>,
    /// Minecraft versions, loader names and "Client"/"Server", mixed.
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<CfDependency>,
    #[serde(default)]
    pub file_fingerprint: u64,
    #[serde(default)]
    pub is_available: Option<bool>,
    #[serde(default)]
    pub is_server_pack: Option<bool>,
}

fn release() -> u8 {
    1
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfHash {
    pub value: String,
    /// 1 SHA-1, 2 MD5.
    pub algo: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfDependency {
    pub mod_id: u64,
    /// 1 embedded library, 2 optional, 3 required, 4 tool, 5 incompatible, 6 include.
    pub relation_type: u8,
}

impl CfDependency {
    pub fn required(&self) -> bool {
        self.relation_type == 3
    }

    pub fn incompatible(&self) -> bool {
        self.relation_type == 5
    }
}

impl CfFile {
    /// The file's SHA-1, lowercase, if CurseForge gave one.
    pub fn sha1(&self) -> Option<String> {
        self.hashes
            .iter()
            .find(|hash| hash.algo == 1)
            .map(|hash| hash.value.to_ascii_lowercase())
            .filter(|hash| hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
    }

    /// Whether the launcher may download it; otherwise the user has to, from
    /// [`CfFile::page_url`].
    pub fn downloadable(&self) -> bool {
        self.download_url
            .as_deref()
            .is_some_and(|url| !url.trim().is_empty())
    }

    /// The page where this file is downloaded by hand, given its project's page.
    pub fn page_url(&self, project_page: &str) -> String {
        format!(
            "{}/download/{}",
            project_page.trim_end_matches('/'),
            self.id
        )
    }

    pub fn version_type(&self) -> &'static str {
        match self.release_type {
            2 => "beta",
            3 => "alpha",
            _ => "release",
        }
    }

    pub fn tags(&self) -> FileTags {
        FileTags::parse(&self.game_versions)
    }

    /// The name to show for the file: its display name, else its file name.
    pub fn display_name_or_file(&self) -> &str {
        if self.display_name.trim().is_empty() {
            &self.file_name
        } else {
            self.display_name.trim()
        }
    }
}

/// What a file's `gameVersions` say: Minecraft versions, loaders and sides.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileTags {
    pub game_versions: Vec<String>,
    pub loaders: Vec<Loader>,
    pub client: bool,
    pub server: bool,
}

impl FileTags {
    pub fn parse(tags: &[String]) -> Self {
        let mut parsed = FileTags::default();
        for tag in tags {
            let lower = tag.trim().to_ascii_lowercase();
            match lower.as_str() {
                "forge" => parsed.loaders.push(Loader::Forge),
                "neoforge" => parsed.loaders.push(Loader::NeoForge),
                "fabric" => parsed.loaders.push(Loader::Fabric),
                "quilt" => parsed.loaders.push(Loader::Quilt),
                "client" => parsed.client = true,
                "server" => parsed.server = true,
                _ if is_game_version(&lower) => parsed.game_versions.push(tag.trim().to_string()),
                _ => {} // "Java 17", "Cauldron", "LiteLoader", ...
            }
        }
        parsed
    }

    /// Whether an instance with `loader` can use the file. A file tagged with no
    /// loader fits any. Quilt loads Fabric mods, and NeoForge on 1.20.1 loads Forge
    /// mods (NeoForge forked from Forge in that version).
    pub fn fits_loader(&self, loader: Loader, game_version: &str) -> bool {
        if self.loaders.is_empty() {
            return true;
        }
        accepted_loaders(loader, game_version)
            .iter()
            .any(|accepted| self.loaders.contains(accepted))
    }
}

/// The loaders whose files an instance with `loader` can use, as above.
pub fn accepted_loaders(loader: Loader, game_version: &str) -> Vec<Loader> {
    match loader {
        Loader::None => Vec::new(),
        Loader::Quilt => vec![Loader::Quilt, Loader::Fabric],
        Loader::NeoForge if game_version == "1.20.1" => vec![Loader::NeoForge, Loader::Forge],
        other => vec![other],
    }
}

fn is_game_version(tag: &str) -> bool {
    tag.contains('.')
        && tag.starts_with(|c: char| c.is_ascii_digit())
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// CurseForge's number for a loader (`modLoaderType`).
pub fn loader_type(loader: Loader) -> Option<u8> {
    match loader {
        Loader::None => None,
        Loader::Forge => Some(1),
        Loader::Fabric => Some(4),
        Loader::Quilt => Some(5),
        Loader::NeoForge => Some(6),
    }
}

/// CurseForge's class ids for Minecraft content.
pub const CLASS_MODS: u32 = 6;
pub const CLASS_RESOURCE_PACKS: u32 = 12;
pub const CLASS_WORLDS: u32 = 17;
pub const CLASS_MODPACKS: u32 = 4471;
pub const CLASS_SHADERS: u32 = 6552;
pub const CLASS_DATA_PACKS: u32 = 6945;

pub fn class_of(kind: ContentKind) -> u32 {
    match kind {
        ContentKind::Mod => CLASS_MODS,
        ContentKind::ResourcePack => CLASS_RESOURCE_PACKS,
        ContentKind::ShaderPack => CLASS_SHADERS,
    }
}

pub fn kind_of_class(class: u32) -> Option<ContentKind> {
    match class {
        CLASS_MODS => Some(ContentKind::Mod),
        CLASS_RESOURCE_PACKS => Some(ContentKind::ResourcePack),
        CLASS_SHADERS => Some(ContentKind::ShaderPack),
        _ => None,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfPagination {
    #[serde(default)]
    pub index: usize,
    #[serde(default)]
    pub page_size: usize,
    #[serde(default)]
    pub result_count: usize,
    #[serde(default)]
    pub total_count: usize,
}

#[derive(Deserialize)]
struct Page<T> {
    data: Vec<T>,
    #[serde(default)]
    pagination: CfPagination,
}

/// A page of search results.
#[derive(Clone, Debug, PartialEq)]
pub struct CfSearchResults {
    pub hits: Vec<CfMod>,
    pub offset: usize,
    pub total: usize,
}

/// How search results are ordered (`sortField`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CfSort {
    #[default]
    Popularity,
    LastUpdated,
    Name,
    TotalDownloads,
}

impl CfSort {
    fn field(self) -> u8 {
        match self {
            CfSort::Popularity => 2,
            CfSort::LastUpdated => 3,
            CfSort::Name => 4,
            CfSort::TotalDownloads => 6,
        }
    }
}

/// What to search for.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CfSearch {
    pub query: String,
    pub class_id: u32,
    pub game_version: Option<String>,
    /// Any of these loaders; none for no filter.
    pub loaders: Vec<Loader>,
    pub sort: CfSort,
    pub offset: usize,
    pub limit: usize,
}

impl CfSearch {
    pub fn new(kind_class: u32, query: &str) -> Self {
        Self {
            query: query.trim().to_string(),
            class_id: kind_class,
            limit: 20,
            ..Self::default()
        }
    }

    fn query_pairs(&self) -> Vec<(&'static str, String)> {
        let limit = self.limit.clamp(1, PAGE_SIZE);
        let offset = self.offset.min(MAX_SEARCH_RESULTS.saturating_sub(limit));
        let mut pairs = vec![
            ("gameId", GAME_ID.to_string()),
            ("classId", self.class_id.to_string()),
            ("index", offset.to_string()),
            ("pageSize", limit.to_string()),
            ("sortField", self.sort.field().to_string()),
            (
                "sortOrder",
                if self.sort == CfSort::Name {
                    "asc"
                } else {
                    "desc"
                }
                .to_string(),
            ),
        ];
        if !self.query.is_empty() {
            pairs.push(("searchFilter", self.query.clone()));
        }
        if let Some(version) = self.game_version.as_ref().filter(|v| !v.is_empty()) {
            pairs.push(("gameVersion", version.clone()));
        }
        let types: Vec<u8> = self
            .loaders
            .iter()
            .filter_map(|l| loader_type(*l))
            .collect();
        if !types.is_empty() {
            pairs.push((
                "modLoaderTypes",
                serde_json::to_string(&types).unwrap_or_default(),
            ));
        }
        pairs
    }
}

/// A file CurseForge matched by fingerprint, with its project id.
#[derive(Clone, Debug, PartialEq)]
pub struct CfMatch {
    pub mod_id: u64,
    pub file: CfFile,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintAnswer {
    #[serde(default)]
    exact_matches: Vec<ExactMatch>,
}

#[derive(Deserialize)]
struct ExactMatch {
    id: u64,
    file: CfFile,
}

// ---------------------------------------------------------------------------

/// The CurseForge client. Share it with an `Arc`.
pub struct CurseForge {
    http: Arc<dyn HttpClient>,
    api: Url,
    key: String,
    /// Where `https://<cdn host>/<path>` is fetched from in tests.
    cdn_mirror: Option<Url>,
    blocked_until: Mutex<Option<Instant>>,
    requests: std::sync::atomic::AtomicU64,
}

impl CurseForge {
    /// The client for CurseForge's API with Lodestone's key, or `None` without a key.
    /// `LODESTONE_CURSEFORGE_API_URL` points it at a local mock instead, which also
    /// serves the CDN.
    pub fn production(http: Arc<dyn HttpClient>) -> Option<Self> {
        let key = api_key()?;
        match std::env::var(API_URL_ENV)
            .ok()
            .and_then(|url| loopback_url(&url))
        {
            Some(mock) => Some(Self::with_urls(
                http,
                mock.join("v1").ok()?,
                key,
                Some(mock),
            )),
            None => Some(Self::with_urls(
                http,
                Url::parse(API_BASE).expect("a valid URL"),
                key,
                None,
            )),
        }
    }

    /// A client for `api` (instead of [`API_BASE`]) serving the CDN from
    /// `<cdn_mirror>/<host>/<path>`, for tests.
    pub fn with_urls(
        http: Arc<dyn HttpClient>,
        api: Url,
        key: String,
        cdn_mirror: Option<Url>,
    ) -> Self {
        Self {
            http,
            api,
            key,
            cdn_mirror,
            blocked_until: Mutex::new(None),
            requests: Default::default(),
        }
    }

    pub fn requests_sent(&self) -> u64 {
        self.requests.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn url(&self, path: &str) -> Result<Url, CurseForgeError> {
        Url::parse(&crate::http::join(&self.api, path))
            .map_err(|err| CurseForgeError::Invalid(err.to_string()))
    }

    fn check_blocked(&self) -> Result<(), CurseForgeError> {
        let blocked = self.blocked_until.lock().unwrap_or_else(|p| p.into_inner());
        match *blocked {
            Some(until) if until > Instant::now() => Err(CurseForgeError::RateLimited {
                seconds: until
                    .saturating_duration_since(Instant::now())
                    .as_secs()
                    .max(1),
            }),
            _ => Ok(()),
        }
    }

    async fn send(
        &self,
        method: Method,
        url: Url,
        body: Option<serde_json::Value>,
        limit: usize,
        with_key: bool,
    ) -> Result<Vec<u8>, CurseForgeError> {
        self.check_blocked()?;
        let mut builder = Request::builder()
            .method(method)
            .uri(url.as_str())
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
            .follow_redirects(RedirectPolicy::NoFollow)
            .timeout(REQUEST_TIMEOUT);
        if with_key {
            builder = builder.header("x-api-key", self.key.as_str());
        }
        let body = body.map(|b| serde_json::to_vec(&b).unwrap_or_default());
        if body.is_some() {
            builder = builder.header("Content-Type", "application/json");
        }
        let request = builder
            .body(body.map_or_else(AsyncBody::empty, AsyncBody::from))
            .map_err(|err| CurseForgeError::Invalid(err.to_string()))?;
        self.requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let http = self.http.clone();
        crate::http::in_http_runtime(async move {
            let response = http
                .send(request)
                .await
                .map_err(|err| CurseForgeError::Unreachable(format!("{err:#}")))?;
            let status = response.status().as_u16();
            if status == 429 {
                *self.blocked_until.lock().unwrap_or_else(|p| p.into_inner()) =
                    Some(Instant::now() + RATE_LIMIT_PAUSE);
                return Err(CurseForgeError::RateLimited {
                    seconds: RATE_LIMIT_PAUSE.as_secs(),
                });
            }
            let mut body = Vec::new();
            response
                .into_body()
                .take(limit as u64 + 1)
                .read_to_end(&mut body)
                .await
                .map_err(|err| CurseForgeError::Unreachable(err.to_string()))?;
            match status {
                200..=299 if body.len() > limit => Err(CurseForgeError::Invalid(format!(
                    "CurseForge's answer was larger than {} MB",
                    limit / (1024 * 1024)
                ))),
                200..=299 => Ok(body),
                401 | 403 => Err(CurseForgeError::KeyRefused),
                404 | 410 => Err(CurseForgeError::NotFound("That".into())),
                400..=499 => Err(CurseForgeError::Invalid(format!(
                    "CurseForge refused the request (HTTP {status})"
                ))),
                _ => Err(CurseForgeError::Unreachable(format!("HTTP {status}"))),
            }
        })
        .await
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        url: Url,
        what: &str,
    ) -> Result<T, CurseForgeError> {
        let bytes = self
            .send(Method::GET, url, None, MAX_BODY_BYTES, true)
            .await
            .map_err(|err| named(err, what))?;
        parse(&bytes)
    }

    async fn post<T: serde::de::DeserializeOwned>(
        &self,
        url: Url,
        body: serde_json::Value,
    ) -> Result<T, CurseForgeError> {
        let bytes = self
            .send(Method::POST, url, Some(body), MAX_BODY_BYTES, true)
            .await?;
        parse(&bytes)
    }

    pub async fn search(&self, search: &CfSearch) -> Result<CfSearchResults, CurseForgeError> {
        let mut url = self.url("mods/search")?;
        url.query_pairs_mut().extend_pairs(search.query_pairs());
        let page: Page<CfMod> = self.get(url, "The search").await?;
        Ok(CfSearchResults {
            hits: page.data,
            offset: page.pagination.index,
            total: page.pagination.total_count.min(MAX_SEARCH_RESULTS),
        })
    }

    pub async fn project(&self, id: u64) -> Result<CfMod, CurseForgeError> {
        let data: Data<CfMod> = self
            .get(self.url(&format!("mods/{id}"))?, "That project")
            .await?;
        Ok(data.data)
    }

    /// Several projects by id, in one request per 100. Unknown ids are left out.
    pub async fn projects(&self, ids: &[u64]) -> Result<Vec<CfMod>, CurseForgeError> {
        let mut projects = Vec::new();
        for chunk in ids.chunks(100) {
            let data: Data<Vec<CfMod>> = self
                .post(self.url("mods")?, json!({ "modIds": chunk }))
                .await?;
            projects.extend(data.data);
        }
        Ok(projects)
    }

    /// The project's description, as HTML.
    pub async fn description(&self, id: u64) -> Result<String, CurseForgeError> {
        let data: Data<String> = self
            .get(self.url(&format!("mods/{id}/description"))?, "That project")
            .await?;
        Ok(data.data)
    }

    /// A project's files for `game_version` and `loader` (no filter for `None`), newest
    /// first, read in pages of [`PAGE_SIZE`] up to [`MAX_FILES`]. Server packs are left
    /// out.
    pub async fn files(
        &self,
        project: u64,
        game_version: Option<&str>,
        loader: Option<Loader>,
    ) -> Result<Vec<CfFile>, CurseForgeError> {
        let mut files = Vec::new();
        loop {
            let mut url = self.url(&format!("mods/{project}/files"))?;
            {
                let mut query = url.query_pairs_mut();
                query.append_pair("index", &files.len().to_string());
                query.append_pair("pageSize", &PAGE_SIZE.to_string());
                if let Some(version) = game_version.filter(|v| !v.is_empty()) {
                    query.append_pair("gameVersion", version);
                }
                if let Some(kind) = loader.and_then(loader_type) {
                    query.append_pair("modLoaderType", &kind.to_string());
                }
            }
            let page: Page<CfFile> = self.get(url, "That project").await?;
            let got = page.data.len();
            files.extend(page.data);
            let total = page.pagination.total_count;
            if got == 0 || files.len() >= total || files.len() >= MAX_FILES {
                break;
            }
        }
        files.retain(|file| file.is_server_pack != Some(true));
        sort_newest_first(&mut files);
        Ok(files)
    }

    pub async fn file(&self, project: u64, file: u64) -> Result<CfFile, CurseForgeError> {
        let data: Data<CfFile> = self
            .get(
                self.url(&format!("mods/{project}/files/{file}"))?,
                "That file",
            )
            .await?;
        Ok(data.data)
    }

    /// Several files by id, in one request per 500. Files CurseForge no longer has are
    /// left out.
    pub async fn files_by_id(&self, ids: &[u64]) -> Result<Vec<CfFile>, CurseForgeError> {
        let mut files = Vec::new();
        for chunk in ids.chunks(500) {
            // With one id CurseForge may answer with an object rather than a list.
            let value: Data<serde_json::Value> = self
                .post(self.url("mods/files")?, json!({ "fileIds": chunk }))
                .await?;
            match value.data {
                serde_json::Value::Array(list) => {
                    for item in list {
                        files.push(serde_json::from_value(item).map_err(invalid)?);
                    }
                }
                object @ serde_json::Value::Object(_) => {
                    files.push(serde_json::from_value(object).map_err(invalid)?)
                }
                _ => {}
            }
        }
        Ok(files)
    }

    /// The files with these [`fingerprint`]s, by fingerprint. Unknown ones are left out.
    pub async fn files_by_fingerprint(
        &self,
        fingerprints: &[u32],
    ) -> Result<std::collections::HashMap<u32, CfMatch>, CurseForgeError> {
        let mut found = std::collections::HashMap::new();
        for chunk in fingerprints.chunks(500) {
            let answer: Data<FingerprintAnswer> = self
                .post(
                    self.url(&format!("fingerprints/{GAME_ID}"))?,
                    json!({ "fingerprints": chunk }),
                )
                .await?;
            for hit in answer.data.exact_matches {
                let print = hit.file.file_fingerprint as u32;
                found.insert(
                    print,
                    CfMatch {
                        mod_id: hit.id,
                        file: hit.file,
                    },
                );
            }
        }
        Ok(found)
    }

    /// The URL to download a file from, if CurseForge gave one on its CDN.
    pub fn download_url(&self, file: &CfFile) -> Result<String, CurseForgeError> {
        let url = file
            .download_url
            .as_deref()
            .filter(|u| !u.trim().is_empty())
            .ok_or_else(|| {
                CurseForgeError::Invalid(format!(
                    "{} can only be downloaded from CurseForge's website",
                    file.file_name
                ))
            })?;
        self.allowed_url(url, &DOWNLOAD_HOSTS)
    }

    fn allowed_url(&self, url: &str, hosts: &[&str]) -> Result<String, CurseForgeError> {
        let parsed =
            Url::parse(url).map_err(|_| CurseForgeError::Invalid(format!("{url} isn't a URL")))?;
        let host = parsed.host_str().unwrap_or_default().to_string();
        if parsed.scheme() != "https"
            || !hosts.contains(&host.as_str())
            || parsed.port().is_some()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(CurseForgeError::Invalid(format!(
                "Refusing to download from {}: CurseForge files only come from {}",
                if host.is_empty() {
                    "an unknown host"
                } else {
                    &host
                },
                hosts.join(", ")
            )));
        }
        // Percent-encoded: CurseForge's file names may contain spaces.
        let path = match parsed.query() {
            Some(query) => format!("{}?{query}", parsed.path()),
            None => parsed.path().to_string(),
        };
        Ok(match &self.cdn_mirror {
            None => format!("https://{host}{path}"),
            Some(base) => format!("{}/{host}{path}", base.as_str().trim_end_matches('/')),
        })
    }

    /// The bytes of a project logo, at most [`MAX_IMAGE_BYTES`]. No key is sent.
    pub async fn image(&self, url: &str) -> Result<Vec<u8>, CurseForgeError> {
        let url = self.allowed_url(url, &[MEDIA_HOST])?;
        let url = Url::parse(&url).map_err(invalid)?;
        self.send(Method::GET, url, None, MAX_IMAGE_BYTES, false)
            .await
    }
}

/// `url` if it is plain HTTP or HTTPS on this machine.
fn loopback_url(url: &str) -> Option<Url> {
    let parsed = Url::parse(url).ok()?;
    let loopback = match parsed.host()? {
        url::Host::Domain(name) => name == "localhost",
        url::Host::Ipv4(ip) => ip.is_loopback(),
        url::Host::Ipv6(ip) => ip.is_loopback(),
    };
    (loopback && matches!(parsed.scheme(), "http" | "https")).then_some(parsed)
}

fn invalid(err: impl std::fmt::Display) -> CurseForgeError {
    CurseForgeError::Invalid(format!(
        "CurseForge sent an answer that can't be read: {err}"
    ))
}

fn named(err: CurseForgeError, what: &str) -> CurseForgeError {
    match err {
        CurseForgeError::NotFound(_) => CurseForgeError::NotFound(what.to_string()),
        other => other,
    }
}

fn parse<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, CurseForgeError> {
    serde_json::from_slice(bytes).map_err(invalid)
}

/// Sorts files newest first, by their date.
pub fn sort_newest_first(files: &mut [CfFile]) {
    files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
}

// ---------------------------------------------------------------------------
// Fingerprints

/// CurseForge's fingerprint of a file's bytes: 32-bit MurmurHash2 with seed 1 over the
/// bytes left after removing tabs, line feeds, carriage returns and spaces.
pub fn fingerprint(bytes: &[u8]) -> u32 {
    let mut hasher = Murmur2::seeded(
        1,
        bytes.iter().filter(|b| !is_whitespace(**b)).count() as u32,
    );
    hasher.update(bytes);
    hasher.finish()
}

/// [`fingerprint`] of a file, read twice in chunks (the length of the filtered bytes is
/// part of the hash's seed) rather than into memory at once.
pub fn fingerprint_file(path: &Path) -> io::Result<u32> {
    let mut length = 0u32;
    read_chunks(path, |chunk| {
        length = length.wrapping_add(chunk.iter().filter(|b| !is_whitespace(**b)).count() as u32)
    })?;
    let mut hasher = Murmur2::seeded(1, length);
    read_chunks(path, |chunk| hasher.update(chunk))?;
    Ok(hasher.finish())
}

fn read_chunks(path: &Path, mut each: impl FnMut(&[u8])) -> io::Result<()> {
    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(());
        }
        each(&buffer[..read]);
    }
}

fn is_whitespace(byte: u8) -> bool {
    matches!(byte, 0x09 | 0x0a | 0x0d | 0x20)
}

const M: u32 = 0x5bd1_e995;

/// MurmurHash2 over whitespace-filtered bytes fed in any number of pieces.
struct Murmur2 {
    h: u32,
    pending: [u8; 4],
    filled: usize,
}

impl Murmur2 {
    /// CurseForge's seed is 1; other seeds are for checking against known answers.
    fn seeded(seed: u32, length: u32) -> Self {
        Self {
            h: seed ^ length,
            pending: [0; 4],
            filled: 0,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if is_whitespace(byte) {
                continue;
            }
            self.pending[self.filled] = byte;
            self.filled += 1;
            if self.filled == 4 {
                let mut k = u32::from_le_bytes(self.pending);
                k = k.wrapping_mul(M);
                k ^= k >> 24;
                k = k.wrapping_mul(M);
                self.h = self.h.wrapping_mul(M) ^ k;
                self.filled = 0;
            }
        }
    }

    fn finish(mut self) -> u32 {
        let tail = &self.pending[..self.filled];
        if tail.len() >= 3 {
            self.h ^= u32::from(tail[2]) << 16;
        }
        if tail.len() >= 2 {
            self.h ^= u32::from(tail[1]) << 8;
        }
        if !tail.is_empty() {
            self.h ^= u32::from(tail[0]);
            self.h = self.h.wrapping_mul(M);
        }
        self.h ^= self.h >> 13;
        self.h = self.h.wrapping_mul(M);
        self.h ^= self.h >> 15;
        self.h
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A straightforward MurmurHash2 (seed 1) over already-filtered bytes, to check the
    /// streaming one against.
    fn reference(data: &[u8]) -> u32 {
        let mut h = 1u32 ^ data.len() as u32;
        let (chunks, tail) = data.as_chunks::<4>();
        for chunk in chunks {
            let mut k = u32::from_le_bytes(*chunk);
            k = k.wrapping_mul(M);
            k ^= k >> 24;
            k = k.wrapping_mul(M);
            h = h.wrapping_mul(M) ^ k;
        }
        match tail.len() {
            3 => {
                h ^= u32::from(tail[2]) << 16;
                h ^= u32::from(tail[1]) << 8;
                h ^= u32::from(tail[0]);
                h = h.wrapping_mul(M);
            }
            2 => {
                h ^= u32::from(tail[1]) << 8;
                h ^= u32::from(tail[0]);
                h = h.wrapping_mul(M);
            }
            1 => {
                h ^= u32::from(tail[0]);
                h = h.wrapping_mul(M);
            }
            _ => {}
        }
        h ^= h >> 13;
        h = h.wrapping_mul(M);
        h ^= h >> 15;
        h
    }

    /// Known answers for MurmurHash2 from github.com/aviddiviner/go-murmur's tests,
    /// the implementation packwiz uses for CurseForge fingerprints.
    #[test]
    fn murmur2_matches_published_values() {
        let hash = |data: &[u8], seed: u32| {
            let mut hasher = Murmur2::seeded(seed, data.len() as u32);
            hasher.update(data);
            hasher.finish()
        };
        assert_eq!(hash(b"foo", 123), 1_412_061_192);
        assert_eq!(hash(b"zztop", 123), 1_878_194_508);
        assert_eq!(hash(b"foobarbaz", 234), 1_777_016_281);
        assert_eq!(hash(b"blam", 777), 1_668_928_339);
    }

    #[test]
    fn fingerprints_ignore_whitespace() {
        assert_eq!(fingerprint(b"a b\tc\r\nd"), fingerprint(b"abcd"));
        assert_ne!(fingerprint(b"abcd"), fingerprint(b"abce"));
        // Only those four bytes count as whitespace.
        assert_ne!(fingerprint(b"ab\x0bcd"), fingerprint(b"abcd"));
    }

    #[test]
    fn fingerprints_match_murmur2_with_seed_1() {
        // The empty input: h = 1 ^ 0, then the final mix.
        let mut h = 1u32;
        h ^= h >> 13;
        h = h.wrapping_mul(M);
        h ^= h >> 15;
        assert_eq!(fingerprint(b""), h);
        for data in [
            &b"x"[..],
            b"xy",
            b"xyz",
            b"wxyz",
            b"hello, world!",
            &[0xffu8; 37],
        ] {
            let filtered: Vec<u8> = data
                .iter()
                .copied()
                .filter(|b| !is_whitespace(*b))
                .collect();
            assert_eq!(fingerprint(data), reference(&filtered), "{data:?}");
        }
    }

    #[test]
    fn a_files_fingerprint_is_read_in_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.jar");
        let bytes: Vec<u8> = (0..3_000_001u32).map(|i| (i * 7 % 251) as u8).collect();
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(fingerprint_file(&path).unwrap(), fingerprint(&bytes));
    }

    #[test]
    fn the_key_comes_from_the_environment_or_the_build() {
        assert_eq!(key_from(None, None), None);
        assert_eq!(key_from(Some("  ".into()), None), None);
        assert_eq!(key_from(None, Some("built")).as_deref(), Some("built"));
        assert_eq!(
            key_from(Some(" mine ".into()), Some("built")).as_deref(),
            Some("mine")
        );
    }

    #[test]
    fn file_tags_say_versions_loaders_and_sides() {
        let tags = FileTags::parse(
            &["1.20.1", "Forge", "NeoForge", "Client", "Java 17", "1.20"].map(String::from),
        );
        assert_eq!(tags.game_versions, ["1.20.1", "1.20"]);
        assert_eq!(tags.loaders, [Loader::Forge, Loader::NeoForge]);
        assert!(tags.client && !tags.server);
    }

    #[test]
    fn quilt_takes_fabric_files_and_neoforge_on_1_20_1_takes_forge_files() {
        let fabric = FileTags::parse(&["1.20.1".into(), "Fabric".into()]);
        let forge = FileTags::parse(&["1.20.1".into(), "Forge".into()]);
        let untagged = FileTags::parse(&["1.20.1".into()]);
        assert!(fabric.fits_loader(Loader::Quilt, "1.20.1"));
        assert!(!fabric.fits_loader(Loader::Forge, "1.20.1"));
        assert!(forge.fits_loader(Loader::NeoForge, "1.20.1"));
        assert!(!forge.fits_loader(Loader::NeoForge, "1.21.1"));
        assert!(untagged.fits_loader(Loader::Fabric, "1.20.1"));
    }

    #[test]
    fn search_parameters_stay_within_the_api_limits() {
        let mut search = CfSearch::new(CLASS_MODS, " sodium ");
        search.game_version = Some("1.20.1".into());
        search.loaders = vec![Loader::Quilt, Loader::Fabric];
        search.limit = 500;
        search.offset = 20_000;
        let pairs: std::collections::HashMap<_, _> = search.query_pairs().into_iter().collect();
        assert_eq!(pairs["gameId"], "432");
        assert_eq!(pairs["classId"], "6");
        assert_eq!(pairs["searchFilter"], "sodium");
        assert_eq!(pairs["pageSize"], "50");
        assert_eq!(pairs["index"], (MAX_SEARCH_RESULTS - 50).to_string());
        assert_eq!(pairs["modLoaderTypes"], "[5,4]");
        assert_eq!(pairs["gameVersion"], "1.20.1");
    }

    #[test]
    fn files_only_come_from_the_cdn() {
        let http = crate::http::default_client().unwrap();
        let cf = CurseForge::with_urls(
            http.clone(),
            Url::parse(API_BASE).unwrap(),
            "k".into(),
            None,
        );
        let file = |url: Option<&str>| CfFile {
            id: 4_567_890,
            mod_id: 1,
            display_name: String::new(),
            file_name: "mod.jar".into(),
            release_type: 1,
            file_date: String::new(),
            download_url: url.map(String::from),
            file_length: 1,
            hashes: vec![],
            game_versions: vec![],
            dependencies: vec![],
            file_fingerprint: 0,
            is_available: None,
            is_server_pack: None,
        };
        assert_eq!(
            cf.download_url(&file(Some(
                "https://edge.forgecdn.net/files/4567/890/My Mod+1.jar"
            )))
            .unwrap(),
            "https://edge.forgecdn.net/files/4567/890/My%20Mod+1.jar"
        );
        for bad in [
            None,
            Some(""),
            Some("http://edge.forgecdn.net/files/1/2/a.jar"),
            Some("https://edge.forgecdn.net.evil.com/a.jar"),
            Some("https://user@edge.forgecdn.net/a.jar"),
            Some("https://edge.forgecdn.net:444/a.jar"),
            Some("https://media.forgecdn.net/a.jar"),
        ] {
            assert!(cf.download_url(&file(bad)).is_err(), "{bad:?}");
        }
        assert!(!file(None).downloadable());
        assert_eq!(
            file(None).page_url("https://www.curseforge.com/minecraft/mc-mods/jei/"),
            "https://www.curseforge.com/minecraft/mc-mods/jei/download/4567890"
        );
        let mirror = CurseForge::with_urls(
            http,
            Url::parse("http://127.0.0.1:1/v1").unwrap(),
            "k".into(),
            Some(Url::parse("http://127.0.0.1:1/").unwrap()),
        );
        assert_eq!(
            mirror
                .download_url(&file(Some("https://edge.forgecdn.net/files/1/2/a.jar")))
                .unwrap(),
            "http://127.0.0.1:1/edge.forgecdn.net/files/1/2/a.jar"
        );
    }

    #[test]
    fn only_loopback_mocks_replace_the_api() {
        assert!(loopback_url("http://127.0.0.1:8080/").is_some());
        assert!(loopback_url("http://localhost:1/").is_some());
        assert!(loopback_url("http://example.com/").is_none());
        assert!(loopback_url("file:///x").is_none());
    }
}
