//! The Modrinth API (<https://docs.modrinth.com/api/>), v2, which needs no key.
//!
//! Requests carry [`USER_AGENT`], time out after [`REQUEST_TIMEOUT`] and read at most
//! [`MAX_BODY_BYTES`]. Answers are cached in memory ([`CACHE_TTL`], hash lookups
//! [`HASH_CACHE_TTL`]), up to [`CACHE_ENTRIES`]. Modrinth allows 300 requests a minute per
//! IP: once it says none are left (`X-Ratelimit-Remaining: 0`, or a 429), calls fail at
//! once until `X-Ratelimit-Reset` has passed, rather than retrying.
//!
//! Files are only downloaded from `https://cdn.modrinth.com` ([`Modrinth::cdn_url`]);
//! tests serve that host from a loopback mirror, like the Mojang URLs.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use futures::AsyncReadExt as _;
use http_client::{AsyncBody, HttpClient, HttpRequestExt as _, Method, RedirectPolicy, Request};
use lodestone_minecraft::modrinth::{
    CDN_HOST, ClientSearchParams, ModrinthProject, ModrinthSearchResponse, RawProject, RawVersion,
    links::{check_api_segment, parse_modrinth_url},
};
use serde_json::json;
use url::Url;

/// The User-Agent Modrinth asks API clients to send: the app and where to find it.
pub const USER_AGENT: &str = concat!(
    "Lodestone/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Lodestone-Team/lodestone)"
);
pub const API_BASE: &str = "https://api.modrinth.com/v2";
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
pub const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
/// Icons bigger than this are not shown.
pub const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
pub const CACHE_TTL: Duration = Duration::from_secs(5 * 60);
pub const HASH_CACHE_TTL: Duration = Duration::from_secs(60 * 60);
pub const CACHE_ENTRIES: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ModrinthError {
    #[error("{0} isn't on Modrinth")]
    NotFound(String),
    #[error("Modrinth asks to slow down; try again in {seconds} seconds")]
    RateLimited { seconds: u64 },
    #[error("Modrinth isn't reachable: {0}")]
    Unreachable(String),
    #[error("{0}")]
    Invalid(String),
}

impl From<lodestone_minecraft::Error> for ModrinthError {
    fn from(err: lodestone_minecraft::Error) -> Self {
        ModrinthError::Invalid(err.to_string())
    }
}

/// What a modrinth.com link points to.
#[derive(Clone, Debug)]
pub struct LinkTarget {
    pub project: ModrinthProject,
    pub version: Option<RawVersion>,
}

#[derive(Default)]
struct Cache {
    entries: HashMap<String, (Instant, Duration, Arc<Vec<u8>>)>,
    order: VecDeque<String>,
}

impl Cache {
    fn get(&mut self, key: &str) -> Option<Arc<Vec<u8>>> {
        let (stored, ttl, body) = self.entries.get(key)?;
        if stored.elapsed() < *ttl {
            return Some(body.clone());
        }
        self.entries.remove(key);
        self.order.retain(|k| k != key);
        None
    }

    fn put(&mut self, key: String, ttl: Duration, body: Arc<Vec<u8>>) {
        if self
            .entries
            .insert(key.clone(), (Instant::now(), ttl, body))
            .is_none()
        {
            self.order.push_back(key);
        }
        while self.order.len() > CACHE_ENTRIES {
            if let Some(old) = self.order.pop_front() {
                self.entries.remove(&old);
            }
        }
    }
}

/// The Modrinth client. Share it with an `Arc`.
pub struct Modrinth {
    http: Arc<dyn HttpClient>,
    api: Url,
    /// Where `https://cdn.modrinth.com/<path>` is fetched from in tests.
    cdn_mirror: Option<Url>,
    cache: Mutex<Cache>,
    /// Calls fail at once until then.
    blocked_until: Mutex<Option<Instant>>,
    requests: std::sync::atomic::AtomicU64,
}

impl Modrinth {
    pub fn production(http: Arc<dyn HttpClient>) -> Self {
        Self::with_urls(http, Url::parse(API_BASE).expect("a valid URL"), None)
    }

    /// A client for `api` (instead of [`API_BASE`]) serving the CDN from
    /// `<cdn_mirror>/cdn.modrinth.com/<path>`, for tests.
    pub fn with_urls(http: Arc<dyn HttpClient>, api: Url, cdn_mirror: Option<Url>) -> Self {
        Self {
            http,
            api,
            cdn_mirror,
            cache: Mutex::default(),
            blocked_until: Mutex::new(None),
            requests: Default::default(),
        }
    }

    /// The number of requests sent, for tests of the cache and rate limit.
    pub fn requests_sent(&self) -> u64 {
        self.requests.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn url(&self, path: &str) -> String {
        crate::http::join(&self.api, path)
    }

    fn check_blocked(&self) -> Result<(), ModrinthError> {
        let blocked = self.blocked_until.lock().unwrap_or_else(|p| p.into_inner());
        match *blocked {
            Some(until) if until > Instant::now() => Err(ModrinthError::RateLimited {
                seconds: until
                    .saturating_duration_since(Instant::now())
                    .as_secs()
                    .max(1),
            }),
            _ => Ok(()),
        }
    }

    fn block_for(&self, seconds: u64) {
        *self.blocked_until.lock().unwrap_or_else(|p| p.into_inner()) =
            Some(Instant::now() + Duration::from_secs(seconds));
    }

    async fn send(
        &self,
        method: Method,
        url: &str,
        body: Option<Vec<u8>>,
        limit: usize,
    ) -> Result<Vec<u8>, ModrinthError> {
        self.check_blocked()?;
        let mut builder = Request::builder()
            .method(method)
            .uri(url)
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
            .follow_redirects(RedirectPolicy::NoFollow)
            .timeout(REQUEST_TIMEOUT);
        if body.is_some() {
            builder = builder.header("Content-Type", "application/json");
        }
        let request = builder
            .body(body.map_or_else(AsyncBody::empty, AsyncBody::from))
            .map_err(|err| ModrinthError::Invalid(err.to_string()))?;
        self.requests
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let http = self.http.clone();
        crate::http::in_http_runtime(async move {
            let response = http
                .send(request)
                .await
                .map_err(|err| ModrinthError::Unreachable(format!("{err:#}")))?;
            let status = response.status().as_u16();
            let header = |name: &str| {
                response
                    .headers()
                    .get(name)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.trim().parse::<u64>().ok())
            };
            let reset = header("x-ratelimit-reset").unwrap_or(60).clamp(1, 3600);
            if status == 429 {
                self.block_for(reset);
                return Err(ModrinthError::RateLimited { seconds: reset });
            }
            if header("x-ratelimit-remaining") == Some(0) {
                self.block_for(reset);
            }
            let mut body = Vec::new();
            response
                .into_body()
                .take(limit as u64 + 1)
                .read_to_end(&mut body)
                .await
                .map_err(|err| ModrinthError::Unreachable(err.to_string()))?;
            match status {
                200..=299 if body.len() > limit => Err(ModrinthError::Invalid(format!(
                    "Modrinth's answer was larger than {} MB",
                    limit / (1024 * 1024)
                ))),
                200..=299 => Ok(body),
                404 | 410 => Err(ModrinthError::NotFound("That".into())),
                400..=499 => Err(ModrinthError::Invalid(format!(
                    "Modrinth refused the request (HTTP {status})"
                ))),
                _ => Err(ModrinthError::Unreachable(format!("HTTP {status}"))),
            }
        })
        .await
    }

    async fn cached(
        &self,
        method: Method,
        url: String,
        body: Option<serde_json::Value>,
        ttl: Duration,
    ) -> Result<Arc<Vec<u8>>, ModrinthError> {
        let body = body.map(|b| serde_json::to_vec(&b).unwrap_or_default());
        let key = format!(
            "{method} {url} {}",
            body.as_deref()
                .map(String::from_utf8_lossy)
                .unwrap_or_default()
        );
        if let Some(hit) = self
            .cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&key)
        {
            return Ok(hit);
        }
        let bytes = Arc::new(self.send(method, &url, body, MAX_BODY_BYTES).await?);
        self.cache
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .put(key, ttl, bytes.clone());
        Ok(bytes)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(
        &self,
        url: String,
        what: &str,
    ) -> Result<T, ModrinthError> {
        let bytes = self
            .cached(Method::GET, url, None, CACHE_TTL)
            .await
            .map_err(|err| named(err, what))?;
        parse(&bytes)
    }

    pub async fn search(
        &self,
        params: &ClientSearchParams,
    ) -> Result<ModrinthSearchResponse, ModrinthError> {
        let mut url = Url::parse(&self.url("search"))
            .map_err(|err| ModrinthError::Invalid(err.to_string()))?;
        url.query_pairs_mut().extend_pairs(params.query_pairs());
        let response: ModrinthSearchResponse = self.get_json(url.into(), "The search").await?;
        Ok(response.normalize())
    }

    pub async fn project(&self, id_or_slug: &str) -> Result<ModrinthProject, ModrinthError> {
        let id = check_api_segment(id_or_slug, "project id")?;
        let raw: RawProject = self
            .get_json(self.url(&format!("project/{id}")), id)
            .await?;
        Ok(raw.into())
    }

    /// Several projects by id, in one request per 100.
    pub async fn projects(&self, ids: &[String]) -> Result<Vec<ModrinthProject>, ModrinthError> {
        let mut projects = Vec::new();
        for chunk in ids.chunks(100) {
            for id in chunk {
                check_api_segment(id, "project id")?;
            }
            let mut url = Url::parse(&self.url("projects"))
                .map_err(|err| ModrinthError::Invalid(err.to_string()))?;
            url.query_pairs_mut()
                .append_pair("ids", &serde_json::to_string(chunk).unwrap_or_default());
            let raw: Vec<RawProject> = self.get_json(url.into(), "The projects").await?;
            projects.extend(raw.into_iter().map(ModrinthProject::from));
        }
        Ok(projects)
    }

    /// The versions of a project for any of `loaders` and `game_versions` (no filter when
    /// empty), newest first.
    pub async fn versions(
        &self,
        project: &str,
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<Vec<RawVersion>, ModrinthError> {
        let id = check_api_segment(project, "project id")?;
        let mut url = Url::parse(&self.url(&format!("project/{id}/version")))
            .map_err(|err| ModrinthError::Invalid(err.to_string()))?;
        if !loaders.is_empty() {
            url.query_pairs_mut().append_pair(
                "loaders",
                &serde_json::to_string(loaders).unwrap_or_default(),
            );
        }
        if !game_versions.is_empty() {
            url.query_pairs_mut().append_pair(
                "game_versions",
                &serde_json::to_string(game_versions).unwrap_or_default(),
            );
        }
        let mut versions: Vec<RawVersion> = self.get_json(url.into(), id).await?;
        sort_newest_first(&mut versions);
        Ok(versions)
    }

    /// Several versions by id, in one request per 100. Unknown ids are left out.
    pub async fn versions_by_id(&self, ids: &[String]) -> Result<Vec<RawVersion>, ModrinthError> {
        let mut versions = Vec::new();
        for chunk in ids.chunks(100) {
            for id in chunk {
                check_api_segment(id, "version id")?;
            }
            let mut url = Url::parse(&self.url("versions"))
                .map_err(|err| ModrinthError::Invalid(err.to_string()))?;
            url.query_pairs_mut()
                .append_pair("ids", &serde_json::to_string(chunk).unwrap_or_default());
            let raw: Vec<RawVersion> = self.get_json(url.into(), "The versions").await?;
            versions.extend(raw);
        }
        Ok(versions)
    }

    pub async fn version(&self, id: &str) -> Result<RawVersion, ModrinthError> {
        let id = check_api_segment(id, "version id")?;
        self.get_json(self.url(&format!("version/{id}")), "That version")
            .await
    }

    /// A version of `project` by id or version number.
    pub async fn project_version(
        &self,
        project: &str,
        version: &str,
    ) -> Result<RawVersion, ModrinthError> {
        let project = check_api_segment(project, "project id")?;
        let version = check_api_segment(version, "version")?;
        self.get_json(
            self.url(&format!("project/{project}/version/{version}")),
            "That version",
        )
        .await
    }

    async fn hash_post(
        &self,
        path: &str,
        hashes: &[String],
        extra: serde_json::Value,
    ) -> Result<HashMap<String, RawVersion>, ModrinthError> {
        let mut found = HashMap::new();
        let hashes: Vec<String> = hashes
            .iter()
            .map(|h| h.to_ascii_lowercase())
            .filter(|h| h.len() == 40 && h.chars().all(|c| c.is_ascii_hexdigit()))
            .collect();
        for chunk in hashes.chunks(200) {
            let mut body = json!({ "hashes": chunk, "algorithm": "sha1" });
            if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
                body.extend(extra.clone());
            }
            let bytes = self
                .cached(Method::POST, self.url(path), Some(body), HASH_CACHE_TTL)
                .await?;
            let map: HashMap<String, RawVersion> = parse(&bytes)?;
            found.extend(map.into_iter().map(|(k, v)| (k.to_ascii_lowercase(), v)));
        }
        Ok(found)
    }

    /// The versions files with these SHA-1 hashes belong to, by hash. Unknown hashes are
    /// left out.
    pub async fn versions_by_sha1(
        &self,
        hashes: &[String],
    ) -> Result<HashMap<String, RawVersion>, ModrinthError> {
        self.hash_post("version_files", hashes, json!({})).await
    }

    /// The newest version for `loaders` and `game_versions` of the projects these files
    /// belong to, by hash.
    /// The newest version of the files with `hashes` that fits `loaders` and
    /// `game_versions`, by hash. `version_types` ("release", "beta", "alpha") limits
    /// it to those kinds; `None` takes any.
    pub async fn latest_by_sha1(
        &self,
        hashes: &[String],
        loaders: &[String],
        game_versions: &[String],
        version_types: Option<&[&str]>,
    ) -> Result<HashMap<String, RawVersion>, ModrinthError> {
        let mut filters = json!({ "loaders": loaders, "game_versions": game_versions });
        if let Some(types) = version_types {
            filters["version_types"] = json!(types);
        }
        self.hash_post("version_files/update", hashes, filters)
            .await
    }

    /// The project (and version) a pasted modrinth.com link points to.
    pub async fn resolve_link(&self, link: &str) -> Result<LinkTarget, ModrinthError> {
        let parsed = parse_modrinth_url(link)?;
        let project = self.project(&parsed.project).await?;
        let version = match &parsed.version {
            Some(version) => Some(self.project_version(&project.id, version).await?),
            None => None,
        };
        Ok(LinkTarget { project, version })
    }

    /// The URL to download a Modrinth file from, if it is an HTTPS URL on
    /// `cdn.modrinth.com`.
    pub fn cdn_url(&self, url: &str) -> Result<String, ModrinthError> {
        let parsed =
            Url::parse(url).map_err(|_| ModrinthError::Invalid(format!("{url} isn't a URL")))?;
        if parsed.scheme() != "https"
            || parsed.host_str() != Some(CDN_HOST)
            || parsed.port().is_some()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(ModrinthError::Invalid(format!(
                "Refusing to download from {}: files only come from {CDN_HOST}",
                parsed.host_str().unwrap_or("an unknown host")
            )));
        }
        let path = match parsed.query() {
            Some(query) => format!("{}?{query}", parsed.path()),
            None => parsed.path().to_string(),
        };
        Ok(match &self.cdn_mirror {
            None => format!("https://{CDN_HOST}{path}"),
            Some(base) => format!("{}/{CDN_HOST}{path}", base.as_str().trim_end_matches('/')),
        })
    }

    /// The URL to download a modpack's file from, if it is an HTTPS URL on one of the
    /// hosts the `.mrpack` format allows ([`DOWNLOAD_HOSTS`]). Tests fetch
    /// `<cdn_mirror>/<host>/<path>`.
    ///
    /// [`DOWNLOAD_HOSTS`]: lodestone_minecraft::mrpack::DOWNLOAD_HOSTS
    pub fn pack_file_url(&self, url: &Url) -> Result<String, ModrinthError> {
        let host = url.host_str().unwrap_or_default();
        if url.scheme() != "https"
            || !lodestone_minecraft::mrpack::DOWNLOAD_HOSTS.contains(&host)
            || url.port().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(ModrinthError::Invalid(format!(
                "Refusing to download from {}: modpack files only come from {}",
                if host.is_empty() {
                    "an unknown host"
                } else {
                    host
                },
                lodestone_minecraft::mrpack::DOWNLOAD_HOSTS.join(", ")
            )));
        }
        let path = match url.query() {
            Some(query) => format!("{}?{query}", url.path()),
            None => url.path().to_string(),
        };
        Ok(match &self.cdn_mirror {
            None => format!("https://{host}{path}"),
            Some(base) => format!("{}/{host}{path}", base.as_str().trim_end_matches('/')),
        })
    }

    /// The bytes of an image on Modrinth's CDN (project icons), at most
    /// [`MAX_IMAGE_BYTES`].
    pub async fn image(&self, url: &str) -> Result<Vec<u8>, ModrinthError> {
        let url = self.cdn_url(url)?;
        self.send(Method::GET, &url, None, MAX_IMAGE_BYTES).await
    }
}

fn named(err: ModrinthError, what: &str) -> ModrinthError {
    match err {
        ModrinthError::NotFound(_) => ModrinthError::NotFound(what.to_string()),
        other => other,
    }
}

fn parse<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, ModrinthError> {
    serde_json::from_slice(bytes).map_err(|err| {
        ModrinthError::Invalid(format!("Modrinth sent an answer that can't be read: {err}"))
    })
}

/// Sorts versions newest first, by their publication time.
pub fn sort_newest_first(versions: &mut [RawVersion]) {
    versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_names_lodestone() {
        assert_eq!(
            USER_AGENT,
            format!(
                "Lodestone/{} (https://github.com/Lodestone-Team/lodestone)",
                env!("CARGO_PKG_VERSION")
            )
        );
    }

    #[test]
    fn only_the_cdn_is_downloaded_from() {
        let http = crate::http::default_client().unwrap();
        let modrinth = Modrinth::production(http.clone());
        assert_eq!(
            modrinth
                .cdn_url("https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar")
                .unwrap(),
            "https://cdn.modrinth.com/data/AANobbMI/versions/x/sodium.jar"
        );
        for bad in [
            "http://cdn.modrinth.com/data/x.jar",
            "https://cdn.modrinth.com.evil.com/x.jar",
            "https://evil.com/cdn.modrinth.com/x.jar",
            "https://user@cdn.modrinth.com/x.jar",
            "https://cdn.modrinth.com:444/x.jar",
            "https://github.com/x.jar",
            "file:///etc/passwd",
            "not a url",
        ] {
            assert!(modrinth.cdn_url(bad).is_err(), "{bad}");
        }
        let mirror = Modrinth::with_urls(
            http,
            Url::parse("http://127.0.0.1:1/v2").unwrap(),
            Some(Url::parse("http://127.0.0.1:1/").unwrap()),
        );
        assert_eq!(
            mirror
                .cdn_url("https://cdn.modrinth.com/data/x.jar")
                .unwrap(),
            "http://127.0.0.1:1/cdn.modrinth.com/data/x.jar"
        );
    }

    #[test]
    fn the_cache_is_bounded() {
        let mut cache = Cache::default();
        for i in 0..CACHE_ENTRIES + 10 {
            cache.put(i.to_string(), CACHE_TTL, Arc::new(vec![]));
        }
        assert_eq!(cache.entries.len(), CACHE_ENTRIES);
        assert!(cache.get("0").is_none());
        assert!(cache.get(&(CACHE_ENTRIES + 9).to_string()).is_some());
        cache.put("old".into(), Duration::ZERO, Arc::new(vec![]));
        assert!(cache.get("old").is_none());
    }
}
