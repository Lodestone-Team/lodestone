//! The HTTP client for the Modrinth API, with a shared cache and rate limit handling.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use axum::body::Bytes;
use color_eyre::eyre::eyre;
use reqwest::header::HeaderMap;
use reqwest::{RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use sha1::Digest;
use tokio::io::AsyncWriteExt;
use tracing::warn;

use super::cache::TtlCache;
use super::search::SearchParams;
use super::types::{
    ModrinthProject, ModrinthSearchResponse, ModrinthVersion, RawProject, RawVersion,
};
use super::{FileHashes, ModrinthApi, API_BASE, USER_AGENT};
use crate::error::{Error, ErrorKind};

/// Modrinth API requests time out after this long.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// A Modrinth API response may be at most this large.
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
/// Search, project and version responses are cached this long.
const RESPONSE_TTL: Duration = Duration::from_secs(5 * 60);
/// Which version a file hash belongs to is cached this long.
const HASH_TTL: Duration = Duration::from_secs(60 * 60);
/// The newest version for a file hash is cached as long as other version responses.
const UPDATE_TTL: Duration = RESPONSE_TTL;
/// When the rate limit is used up for at most this long, requests wait; otherwise they fail.
const MAX_RATE_LIMIT_WAIT: Duration = Duration::from_secs(5);
/// Modrinth takes at most this many hashes or ids in one request.
const BATCH_SIZE: usize = 100;

fn unreachable(message: impl std::fmt::Display) -> Error {
    Error {
        kind: ErrorKind::External,
        source: eyre!("Modrinth is not reachable: {message}"),
    }
}

/// The Modrinth API client. One is shared by all users, so the cache and the rate limit
/// apply to the whole core.
pub struct ModrinthClient {
    api: reqwest::Client,
    downloads: reqwest::Client,
    base_url: url::Url,
    responses: Mutex<TtlCache>,
    hashes: Mutex<TtlCache>,
    /// Set when Modrinth says the rate limit is used up.
    blocked_until: Mutex<Option<Instant>>,
}

impl Default for ModrinthClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ModrinthClient {
    pub fn new() -> Self {
        let api = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        // no overall timeout for downloads, which can be large, but a stalled one ends
        let downloads = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            api,
            downloads,
            base_url: url::Url::parse(API_BASE).expect("API_BASE is a valid URL"),
            responses: Mutex::new(TtlCache::new(500, 64 * 1024 * 1024)),
            hashes: Mutex::new(TtlCache::new(5000, 16 * 1024 * 1024)),
            blocked_until: Mutex::new(None),
        }
    }

    fn url(&self, segments: &[&str], query: &[(&str, String)]) -> url::Url {
        let mut url = self.base_url.clone();
        if let Ok(mut path) = url.path_segments_mut() {
            path.pop_if_empty().extend(segments);
        }
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|(k, v)| (*k, v.as_str())));
        }
        url
    }

    fn cache_get(cache: &Mutex<TtlCache>, key: &str) -> Option<Bytes> {
        cache.lock().unwrap_or_else(|e| e.into_inner()).get(key)
    }

    fn cache_put(cache: &Mutex<TtlCache>, key: String, value: Bytes, ttl: Duration) {
        cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key, value, ttl);
    }

    /// Waits a little, or fails, while Modrinth's rate limit is used up.
    async fn wait_for_rate_limit(&self) -> Result<(), Error> {
        let wait = {
            let mut blocked = self.blocked_until.lock().unwrap_or_else(|e| e.into_inner());
            match *blocked {
                Some(until) if until > Instant::now() => until - Instant::now(),
                Some(_) => {
                    *blocked = None;
                    return Ok(());
                }
                None => return Ok(()),
            }
        };
        if wait > MAX_RATE_LIMIT_WAIT {
            return Err(unreachable(format!(
                "the rate limit was reached, try again in {} seconds",
                wait.as_secs().max(1)
            )));
        }
        tokio::time::sleep(wait).await;
        Ok(())
    }

    /// Reads the `X-Ratelimit-*` headers. When no requests are left, later requests wait
    /// until the limit resets.
    fn note_rate_limit(&self, headers: &HeaderMap, status: StatusCode) {
        let header = |name: &str| {
            headers
                .get(name)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
        };
        let exhausted =
            status == StatusCode::TOO_MANY_REQUESTS || header("x-ratelimit-remaining") == Some(0);
        if !exhausted {
            return;
        }
        let reset = header("x-ratelimit-reset")
            .or_else(|| header("retry-after"))
            .unwrap_or(60)
            .clamp(1, 120);
        warn!("The Modrinth rate limit was reached; requests are paused for {reset} seconds");
        *self.blocked_until.lock().unwrap_or_else(|e| e.into_inner()) =
            Some(Instant::now() + Duration::from_secs(reset));
    }

    /// Sends a request and reads the body, mapping Modrinth's errors.
    async fn send(&self, request: RequestBuilder) -> Result<Bytes, Error> {
        self.wait_for_rate_limit().await?;
        let mut response = request.send().await.map_err(|e| {
            if e.is_timeout() {
                unreachable("the request timed out")
            } else {
                unreachable(e.without_url())
            }
        })?;
        let status = response.status();
        self.note_rate_limit(response.headers(), status);
        if status == StatusCode::NOT_FOUND {
            return Err(Error {
                kind: ErrorKind::NotFound,
                source: eyre!("Not found on Modrinth"),
            });
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(unreachable("too many requests, try again in a minute"));
        }
        if status.is_server_error() {
            return Err(unreachable(format!("it answered {status}")));
        }
        if !status.is_success() {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Modrinth rejected the request ({status})"),
            });
        }
        if response
            .content_length()
            .is_some_and(|len| len > MAX_RESPONSE_BYTES as u64)
        {
            return Err(unreachable("the response is too large"));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| unreachable(e.without_url()))?
        {
            if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
                return Err(unreachable("the response is too large"));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(Bytes::from(body))
    }

    /// A GET request whose response is cached for `ttl`.
    async fn get_cached(
        &self,
        segments: &[&str],
        query: &[(&str, String)],
        ttl: Duration,
    ) -> Result<Bytes, Error> {
        let url = self.url(segments, query);
        let key = url.to_string();
        if let Some(bytes) = Self::cache_get(&self.responses, &key) {
            return Ok(bytes);
        }
        let bytes = self.send(self.api.get(url)).await?;
        Self::cache_put(&self.responses, key, bytes.clone(), ttl);
        Ok(bytes)
    }

    async fn post(&self, segments: &[&str], body: &serde_json::Value) -> Result<Bytes, Error> {
        let url = self.url(segments, &[]);
        self.send(self.api.post(url).json(body)).await
    }

    /// The cache key for a project fetched by id. `projects` fills the same entries.
    fn project_key(&self, id: &str) -> String {
        self.url(&["project", id], &[]).to_string()
    }

    /// Looks up hashes in the hash cache under `prefix`, asks Modrinth for the rest with
    /// `fetch`, and caches every answer, including "not found". `hashes` must be lowercase
    /// and without duplicates (see [`normalized_hashes`]).
    async fn by_hash<F, Fut>(
        &self,
        prefix: &str,
        hashes: &[String],
        ttl: Duration,
        fetch: F,
    ) -> Result<HashMap<String, ModrinthVersion>, Error>
    where
        F: Fn(Vec<String>) -> Fut,
        Fut: std::future::Future<Output = Result<HashMap<String, RawVersion>, Error>>,
    {
        let mut found = HashMap::new();
        let mut missing: Vec<String> = Vec::new();
        for hash in hashes {
            match Self::cache_get(&self.hashes, &format!("{prefix}{hash}"))
                .and_then(|b| serde_json::from_slice::<Option<ModrinthVersion>>(&b).ok())
            {
                Some(Some(version)) => {
                    found.insert(hash.clone(), version);
                }
                Some(None) => {}
                None => missing.push(hash.clone()),
            }
        }
        for chunk in missing.chunks(BATCH_SIZE) {
            let answer: HashMap<String, ModrinthVersion> = fetch(chunk.to_vec())
                .await?
                .into_iter()
                .map(|(hash, raw)| (hash.to_lowercase(), ModrinthVersion::from(raw)))
                .collect();
            for hash in chunk {
                let version = answer.get(hash).cloned();
                if let Ok(bytes) = serde_json::to_vec(&version) {
                    Self::cache_put(
                        &self.hashes,
                        format!("{prefix}{hash}"),
                        Bytes::from(bytes),
                        ttl,
                    );
                }
                if let Some(version) = version {
                    found.insert(hash.clone(), version);
                }
            }
        }
        Ok(found)
    }
}

fn parse<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    serde_json::from_slice(bytes).map_err(|e| unreachable(format!("unexpected response ({e})")))
}

/// Lowercases and de-duplicates hashes, keeping their order.
fn normalized_hashes(hashes: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(hashes.len());
    for hash in hashes {
        let hash = hash.to_lowercase();
        if !out.contains(&hash) {
            out.push(hash);
        }
    }
    out
}

#[async_trait]
impl ModrinthApi for ModrinthClient {
    async fn search(&self, params: &SearchParams) -> Result<ModrinthSearchResponse, Error> {
        let bytes = self
            .get_cached(&["search"], &params.query_pairs(), RESPONSE_TTL)
            .await?;
        Ok(parse::<ModrinthSearchResponse>(&bytes)?.normalize())
    }

    async fn project(&self, id_or_slug: &str) -> Result<ModrinthProject, Error> {
        let bytes = self
            .get_cached(&["project", id_or_slug], &[], RESPONSE_TTL)
            .await?;
        Ok(parse::<RawProject>(&bytes)?.into())
    }

    async fn projects(&self, ids: &[String]) -> Result<Vec<ModrinthProject>, Error> {
        let mut projects = Vec::new();
        let mut missing = Vec::new();
        for id in ids {
            match Self::cache_get(&self.responses, &self.project_key(id))
                .and_then(|b| serde_json::from_slice::<RawProject>(&b).ok())
            {
                Some(raw) => projects.push(raw.into()),
                None if !missing.contains(id) => missing.push(id.clone()),
                None => {}
            }
        }
        for chunk in missing.chunks(BATCH_SIZE) {
            let ids = serde_json::to_string(chunk).unwrap_or_default();
            let url = self.url(&["projects"], &[("ids", ids)]);
            let bytes = self.send(self.api.get(url)).await?;
            for value in parse::<Vec<serde_json::Value>>(&bytes)? {
                let Ok(raw) = serde_json::from_value::<RawProject>(value.clone()) else {
                    continue;
                };
                if let Ok(bytes) = serde_json::to_vec(&value) {
                    Self::cache_put(
                        &self.responses,
                        self.project_key(&raw.id),
                        Bytes::from(bytes),
                        RESPONSE_TTL,
                    );
                }
                projects.push(raw.into());
            }
        }
        Ok(projects)
    }

    async fn project_versions(
        &self,
        id_or_slug: &str,
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<Vec<ModrinthVersion>, Error> {
        let mut query = Vec::new();
        if !loaders.is_empty() {
            query.push((
                "loaders",
                serde_json::to_string(loaders).unwrap_or_default(),
            ));
        }
        if !game_versions.is_empty() {
            query.push((
                "game_versions",
                serde_json::to_string(game_versions).unwrap_or_default(),
            ));
        }
        let bytes = self
            .get_cached(&["project", id_or_slug, "version"], &query, RESPONSE_TTL)
            .await?;
        let mut versions: Vec<ModrinthVersion> = parse::<Vec<RawVersion>>(&bytes)?
            .into_iter()
            .map(Into::into)
            .collect();
        // RFC 3339 dates in the same format sort as text
        versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
        Ok(versions)
    }

    async fn version(&self, version_id: &str) -> Result<ModrinthVersion, Error> {
        let bytes = self
            .get_cached(&["version", version_id], &[], RESPONSE_TTL)
            .await?;
        Ok(parse::<RawVersion>(&bytes)?.into())
    }

    async fn project_version(
        &self,
        project: &str,
        id_or_number: &str,
    ) -> Result<ModrinthVersion, Error> {
        let bytes = self
            .get_cached(
                &["project", project, "version", id_or_number],
                &[],
                RESPONSE_TTL,
            )
            .await?;
        Ok(parse::<RawVersion>(&bytes)?.into())
    }

    async fn versions_by_hash(
        &self,
        sha512: &[String],
    ) -> Result<HashMap<String, ModrinthVersion>, Error> {
        let hashes = normalized_hashes(sha512);
        self.by_hash("version:", &hashes, HASH_TTL, |chunk| async move {
            let body = serde_json::json!({ "hashes": chunk, "algorithm": "sha512" });
            parse(&self.post(&["version_files"], &body).await?)
        })
        .await
    }

    async fn latest_versions_by_hash(
        &self,
        sha512: &[String],
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<HashMap<String, ModrinthVersion>, Error> {
        let hashes = normalized_hashes(sha512);
        let prefix = format!("update:{}:{}:", loaders.join(","), game_versions.join(","));
        self.by_hash(&prefix, &hashes, UPDATE_TTL, |chunk| async move {
            let body = serde_json::json!({
                "hashes": chunk,
                "algorithm": "sha512",
                "loaders": loaders,
                "game_versions": game_versions,
            });
            parse(&self.post(&["version_files", "update"], &body).await?)
        })
        .await
    }

    async fn download(
        &self,
        url: &url::Url,
        file: &mut tokio::fs::File,
        max_bytes: u64,
    ) -> Result<FileHashes, Error> {
        let host = url.host_str().unwrap_or_default().to_string();
        let failed = |message: String| Error {
            kind: ErrorKind::External,
            source: eyre!("Download from {host} failed: {message}"),
        };
        let mut response = self
            .downloads
            .get(url.clone())
            .send()
            .await
            .map_err(|e| failed(e.without_url().to_string()))?;
        let status = response.status();
        if !status.is_success() {
            return Err(failed(format!("the server answered {status}")));
        }
        if response.content_length().is_some_and(|len| len > max_bytes) {
            return Err(too_large(max_bytes));
        }
        let mut sha1 = sha1::Sha1::new();
        let mut sha512 = sha2::Sha512::new();
        let mut size = 0_u64;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| failed(e.without_url().to_string()))?
        {
            size += chunk.len() as u64;
            if size > max_bytes {
                return Err(too_large(max_bytes));
            }
            sha1.update(&chunk);
            sha512.update(&chunk);
            file.write_all(&chunk)
                .await
                .map_err(|e| eyre!("Failed to write the download: {e}"))?;
        }
        file.flush()
            .await
            .map_err(|e| eyre!("Failed to write the download: {e}"))?;
        Ok(FileHashes {
            size,
            sha1: hex::encode(sha1.finalize()),
            sha512: hex::encode(sha512.finalize()),
        })
    }
}

/// The error for a file that is larger than allowed.
pub fn too_large(max_bytes: u64) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(
            "The file is larger than {}",
            crate::util::format_byte(max_bytes)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_encoded_urls() {
        let client = ModrinthClient::new();
        assert_eq!(
            client
                .url(&["project", "fabric-api", "version"], &[])
                .as_str(),
            "https://api.modrinth.com/v2/project/fabric-api/version"
        );
        // a segment can never add path parts
        assert_eq!(
            client.url(&["project", "a/b?c#d"], &[]).as_str(),
            "https://api.modrinth.com/v2/project/a%2Fb%3Fc%23d"
        );
        let url = client.url(
            &["project", "x", "version"],
            &[("loaders", r#"["fabric"]"#.to_string())],
        );
        assert_eq!(
            url.query_pairs().next().unwrap().1,
            r#"["fabric"]"#,
            "{url}"
        );
    }

    #[test]
    fn rate_limit_headers_pause_requests() {
        let client = ModrinthClient::new();
        let mut headers = HeaderMap::new();
        headers.insert("x-ratelimit-remaining", "12".parse().unwrap());
        client.note_rate_limit(&headers, StatusCode::OK);
        assert!(client.blocked_until.lock().unwrap().is_none());

        headers.insert("x-ratelimit-remaining", "0".parse().unwrap());
        headers.insert("x-ratelimit-reset", "30".parse().unwrap());
        client.note_rate_limit(&headers, StatusCode::OK);
        let until = client.blocked_until.lock().unwrap().unwrap();
        assert!(until > Instant::now() + Duration::from_secs(25));
    }

    // These tests call the real Modrinth API, with Lodestone's user agent and a handful of
    // requests in total.

    #[tokio::test]
    async fn network_search_finds_fabric_api() {
        let client = ModrinthClient::new();
        let params = SearchParams::new(
            Some("fabric-api".into()),
            crate::modrinth::ModrinthProjectType::Mod,
            Some("1.20.1".into()),
            Some("fabric".into()),
            None,
            None,
            Some(10),
        )
        .unwrap();
        let response = client.search(&params).await.unwrap();
        assert!(response.total_hits > 0);
        let hit = response
            .hits
            .iter()
            .find(|h| h.slug == "fabric-api")
            .expect("fabric-api is found");
        assert_eq!(hit.project_id, "P7dR8mSH");
        assert!(hit.categories.contains(&"fabric".to_string()));
        for hit in &response.hits {
            assert!(
                hit.server_side == "required" || hit.server_side == "optional",
                "{} is client only",
                hit.slug
            );
        }
    }

    #[tokio::test]
    async fn network_plugin_search_returns_plugins() {
        let client = ModrinthClient::new();
        let params = SearchParams::new(
            None,
            crate::modrinth::ModrinthProjectType::Plugin,
            None,
            None,
            Some(crate::modrinth::ModrinthSearchIndex::Downloads),
            None,
            Some(20),
        )
        .unwrap();
        let response = client.search(&params).await.unwrap();
        assert!(!response.hits.is_empty());
        for hit in &response.hits {
            assert!(
                hit.categories
                    .iter()
                    .any(|c| crate::modrinth::search::PLUGIN_LOADERS.contains(&c.as_str())),
                "{} has no plugin loader: {:?}",
                hit.slug,
                hit.categories
            );
        }
    }

    #[tokio::test]
    async fn network_resolves_a_link() {
        let client = ModrinthClient::new();
        let target =
            crate::modrinth::links::resolve_url(&client, "https://modrinth.com/mod/fabric-api")
                .await
                .unwrap();
        assert_eq!(target.project_id, "P7dR8mSH");
        assert_eq!(target.slug, "fabric-api");
        assert_eq!(target.project_type, "mod");
        assert_eq!(target.version_id, None);
    }

    #[tokio::test]
    async fn network_installs_a_small_mod() {
        use crate::mods::{InstalledModLoader, ModFolder};
        let client = ModrinthClient::new();
        let dir = tempfile::tempdir().unwrap();
        let folder = ModFolder {
            loader: InstalledModLoader::Fabric,
            game_version: "1.20.1".into(),
            dir: dir.path().join("mods"),
        };
        // LazyDFU 0.1.3, about 15 KB, without dependencies
        let version = client.version("4SHylIO9").await.unwrap();
        let project = client.project(&version.project_id).await.unwrap();
        let result = crate::mods::install::install(
            &client,
            &folder,
            version.clone(),
            &project,
            crate::mods::install::InstallOptions {
                install_dependencies: true,
                force: false,
            },
        )
        .await
        .unwrap();
        let file = version.primary_file().unwrap();
        assert_eq!(result.installed.len(), 1);
        let installed = &result.installed[0];
        assert_eq!(installed.file_name, file.filename);
        assert_eq!(installed.sha512, file.sha512);
        let on_disk = crate::mods::scan::sha512_of(&folder.dir.join(&file.filename)).unwrap();
        assert_eq!(on_disk, file.sha512);
        assert_eq!(installed.modrinth.as_ref().unwrap().slug, "lazydfu");
    }

    #[tokio::test]
    async fn fails_fast_while_rate_limited() {
        let client = ModrinthClient::new();
        *client.blocked_until.lock().unwrap() = Some(Instant::now() + Duration::from_secs(60));
        let err = client.version("abc").await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::External));
        assert!(err
            .source
            .to_string()
            .starts_with("Modrinth is not reachable: the rate limit was reached"));
    }
}
